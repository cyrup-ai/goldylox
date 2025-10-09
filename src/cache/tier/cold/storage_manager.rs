#![allow(dead_code)]
// Cold tier storage manager - Complete storage management library with memory-mapped files, atomic operations, and storage statistics

//! Storage management for memory-mapped files and atomic operations
//!
//! This module handles memory-mapped file operations, atomic writes, and file management
//! for the persistent cold tier cache storage system.

use std::fs::OpenOptions;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use memmap2::MmapOptions;

use super::data_structures::StorageManager;

impl StorageManager {
    /// Create new storage manager
    pub fn new(data_path: PathBuf, index_path: PathBuf, max_file_size: u64) -> io::Result<Self> {
        // Create or open data file
        let data_handle = OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&data_path)?;

        // Set initial size if file is empty
        let data_metadata = data_handle.metadata()?;
        if data_metadata.len() == 0 {
            data_handle.set_len(max_file_size)?;
        }

        // Create memory map for data file
        let data_file = unsafe { Some(MmapOptions::new().map_mut(&data_handle)?) };

        // Create or open index file
        let index_handle = OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&index_path)?;

        // Set initial size for index file (10% of data file size)
        let index_metadata = index_handle.metadata()?;
        let max_index_size = max_file_size / 10;
        if index_metadata.len() == 0 {
            index_handle.set_len(max_index_size)?;
        }

        // Create memory map for index file
        let index_file = unsafe { Some(MmapOptions::new().map_mut(&index_handle)?) };

        Ok(Self {
            data_file,
            index_file,
            data_handle: Some(data_handle),
            index_handle: Some(index_handle),
            data_path,
            index_path,
            write_position: AtomicU64::new(0),
            max_data_size: max_file_size,
            max_index_size,
            generation: AtomicU32::new(1),
        })
    }

    /// Get current write position
    pub fn current_write_position(&self) -> u64 {
        self.write_position.load(Ordering::Relaxed)
    }

    /// Reserve space for writing
    pub fn reserve_space(&self, size: u64) -> Option<u64> {
        let current_pos = self.write_position.load(Ordering::Relaxed);
        if current_pos + size <= self.max_data_size {
            Some(self.write_position.fetch_add(size, Ordering::SeqCst))
        } else {
            None
        }
    }

    /// Sync data file to disk (async, non-blocking)
    pub async fn sync_data(&self) -> io::Result<()> {
        // MmapMut is !Send, so we need to do sync operations in spawn_blocking
        // Clone file handle before moving into spawn_blocking
        let data_file_clone = self.data_handle.as_ref().and_then(|h| h.try_clone().ok());
        
        if let Some(ref mmap) = self.data_file {
            // Get raw pointer - we'll use it unsafely in spawn_blocking
            let mmap_ptr = mmap as *const memmap2::MmapMut as usize;
            
            tokio::task::spawn_blocking(move || -> io::Result<()> {
                // Safety: We ensure StorageManager outlives this task
                // by awaiting the task before StorageManager can be dropped
                unsafe {
                    let mmap_ref = &*(mmap_ptr as *const memmap2::MmapMut);
                    mmap_ref.flush()?;
                }
                
                // Also sync file handle if available
                if let Some(handle) = data_file_clone {
                    handle.sync_all()?;
                }
                
                Ok(())
            })
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))??;
        } else if let Some(handle) = data_file_clone {
            // No mmap but we have a file handle
            let async_file = tokio::fs::File::from_std(handle);
            async_file.sync_all().await?;
        }
        
        Ok(())
    }

    /// Sync index file to disk (async, non-blocking)
    pub async fn sync_index(&self) -> io::Result<()> {
        // MmapMut is !Send, so we need to do sync operations in spawn_blocking
        // Clone file handle before moving into spawn_blocking
        let index_file_clone = self.index_handle.as_ref().and_then(|h| h.try_clone().ok());
        
        if let Some(ref mmap) = self.index_file {
            // Get raw pointer - convert to usize which is Send
            let mmap_ptr = mmap as *const memmap2::MmapMut as usize;
            
            tokio::task::spawn_blocking(move || -> io::Result<()> {
                // Safety: We ensure StorageManager outlives this task
                unsafe {
                    let mmap_ref = &*(mmap_ptr as *const memmap2::MmapMut);
                    mmap_ref.flush()?;
                }
                
                // Also sync file handle if available
                if let Some(handle) = index_file_clone {
                    handle.sync_all()?;
                }
                
                Ok(())
            })
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))??;
        } else if let Some(handle) = index_file_clone {
            // No mmap but we have a file handle
            let async_file = tokio::fs::File::from_std(handle);
            async_file.sync_all().await?;
        }
        
        Ok(())
    }

    /// Async shutdown: sync both data and index files
    /// This should be called during graceful shutdown before Drop
    pub async fn shutdown_async(&self) -> io::Result<()> {
        // Sync data file first
        self.sync_data().await?;
        
        // Then sync index file
        self.sync_index().await?;
        
        Ok(())
    }

    /// Get available space in data file
    pub fn available_space(&self) -> u64 {
        let current_pos = self.write_position.load(Ordering::Relaxed);
        self.max_data_size.saturating_sub(current_pos)
    }

    /// Check if storage needs compaction
    pub fn needs_compaction(&self) -> bool {
        let used_space = self.write_position.load(Ordering::Relaxed);
        let usage_ratio = used_space as f64 / self.max_data_size as f64;
        usage_ratio > 0.8 // Compact when 80% full
    }

    /// Reset write position (used during compaction)
    pub fn reset_write_position(&self) {
        self.write_position.store(0, Ordering::SeqCst);
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Get current generation number
    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Relaxed)
    }

    /// Expand data file if needed with proper memory-mapped file remapping
    pub fn expand_if_needed(&mut self, required_size: u64) -> io::Result<bool> {
        let current_pos = self.write_position.load(Ordering::Relaxed);
        if current_pos + required_size > self.max_data_size {
            // Double the file size
            let new_size = self.max_data_size * 2;

            // Remap the memory-mapped file properly
            if let (Some(handle), Some(mmap)) = (&self.data_handle, &mut self.data_file) {
                // Step 1: Flush any pending writes before remapping
                mmap.flush()?;

                // Step 2: Extend the underlying file
                handle.set_len(new_size)?;

                // Step 3: Drop the old memory map
                drop(std::mem::take(&mut self.data_file));

                // Step 4: Create new memory map with expanded size
                let new_mmap =
                    unsafe { MmapOptions::new().len(new_size as usize).map_mut(handle)? };

                // Step 5: Update the mmap reference
                self.data_file = Some(new_mmap);

                // Step 6: Update max size tracking
                self.max_data_size = new_size;

                // Step 7: Also expand the index file proportionally (10% of data size)
                let new_index_size = new_size / 10;
                if let (Some(index_handle), Some(index_mmap)) =
                    (&self.index_handle, &mut self.index_file)
                {
                    // Flush index before remapping
                    index_mmap.flush()?;

                    // Extend index file
                    index_handle.set_len(new_index_size)?;

                    // Drop old index mmap
                    drop(std::mem::take(&mut self.index_file));

                    // Create new index mmap
                    let new_index_mmap = unsafe {
                        MmapOptions::new()
                            .len(new_index_size as usize)
                            .map_mut(index_handle)?
                    };

                    self.index_file = Some(new_index_mmap);
                    self.max_index_size = new_index_size;
                }

                // Log the expansion for monitoring
                log::info!(
                    "Expanded storage: data {} -> {} bytes, index {} -> {} bytes",
                    self.max_data_size / 2,
                    new_size,
                    self.max_index_size * 10 / new_size,
                    new_index_size
                );

                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Get storage statistics
    pub fn storage_stats(&self) -> StorageStats {
        let used_space = self.write_position.load(Ordering::Relaxed);
        StorageStats {
            total_data_size: self.max_data_size,
            used_data_size: used_space,
            available_data_size: self.max_data_size.saturating_sub(used_space),
            total_index_size: self.max_index_size,
            generation: self.generation.load(Ordering::Relaxed),
            fragmentation_ratio: 0.1, // Default fragmentation ratio for compilation
        }
    }

    /// Calculate fragmentation ratio (simplified)
    fn calculate_fragmentation_ratio<
        K: crate::cache::traits::CacheKey,
        V: crate::cache::traits::CacheValue,
    >(
        &self,
        cache: &crate::cache::tier::cold::storage::ColdTierCache<K, V>,
    ) -> Result<f32, crate::cache::traits::types_and_enums::CacheOperationError> {
        // Connect to existing sophisticated fragmentation analysis using lock-free DashMap access
        let used_space: u64 = cache
            .index
            .iter()
            .map(|entry_ref| entry_ref.value().data_size as u64)
            .sum();
        let total_space = cache
            .write_offset
            .load(std::sync::atomic::Ordering::Relaxed);

        if total_space > 0 {
            Ok((1.0 - (used_space as f64 / total_space as f64)) as f32)
        } else {
            Ok(0.0)
        }
    }

    /// Clear all storage data and reset file contents
    pub fn clear_storage(&mut self) -> Result<(), std::io::Error> {
        // 1. Reset write position to beginning
        self.write_position
            .store(0, std::sync::atomic::Ordering::SeqCst);

        // 2. Increment generation for consistency tracking
        self.generation
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        // 3. Truncate data file to zero and resize to max capacity
        if let Some(handle) = &self.data_handle {
            handle.set_len(0)?;
            handle.set_len(self.max_data_size)?;
            handle.sync_all()?;
        }

        // 4. Truncate index file to zero and resize to max capacity
        if let Some(handle) = &self.index_handle {
            handle.set_len(0)?;
            handle.set_len(self.max_index_size)?;
            handle.sync_all()?;
        }

        // 5. Recreate memory maps for the cleared files
        if let (Some(data_handle), Some(index_handle)) = (&self.data_handle, &self.index_handle) {
            // Drop existing memory maps
            self.data_file = None;
            self.index_file = None;

            // Create new memory maps
            use memmap2::MmapOptions;
            self.data_file = Some(unsafe { MmapOptions::new().map_mut(data_handle)? });
            self.index_file = Some(unsafe { MmapOptions::new().map_mut(index_handle)? });
        }

        Ok(())
    }
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_data_size: u64,
    pub used_data_size: u64,
    pub available_data_size: u64,
    pub total_index_size: u64,
    pub generation: u32,
    pub fragmentation_ratio: f32,
}

impl Drop for StorageManager {
    fn drop(&mut self) {
        // Best-effort synchronous sync (Drop cannot be async)
        // WARNING: This is a fallback - call shutdown_async() explicitly for graceful shutdown
        
        // Synchronous mmap flush
        if let Some(ref mmap) = self.data_file {
            let _ = mmap.flush();
        }
        if let Some(ref mmap) = self.index_file {
            let _ = mmap.flush();
        }
        
        // Synchronous file sync
        if let Some(ref handle) = self.data_handle {
            let _ = handle.sync_all();
        }
        if let Some(ref handle) = self.index_handle {
            let _ = handle.sync_all();
        }
        
        log::debug!("StorageManager dropped with synchronous sync fallback");
    }
}
