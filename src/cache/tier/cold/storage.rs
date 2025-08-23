//! Core storage implementation for cold tier persistent cache
//!
//! This module contains the main data structures and file-based storage
//! logic for the cold tier cache with integrity verification.

use std::fs::{File, OpenOptions};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use ahash::AHashMap;

use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::traits::CompressionAlgorithm;

/// Cold tier cache entry metadata
#[derive(Debug, Clone)]
pub struct ColdEntry {
    /// File offset where data is stored
    pub file_offset: u64,
    /// Size of stored data in bytes
    pub data_size: u32,
    /// Checksum for integrity verification
    pub checksum: u32,
    /// Last access timestamp
    pub last_access: Instant,
    /// Access frequency counter
    pub access_count: u64,
}

/// Cold tier statistics
#[derive(Debug, Clone)]
pub struct ColdTierStats {
    pub hits: u64,
    pub misses: u64,
    pub entries: usize,
    pub storage_bytes: u64,
}

/// Cold tier with persistent file-based storage
pub struct ColdTierCache<K: CacheKey, V: CacheValue> {
    /// File handle for cache storage
    pub storage_file: Arc<Mutex<File>>,
    /// In-memory index for fast lookups
    pub index: Arc<Mutex<AHashMap<K, ColdEntry>>>,
    /// Storage directory path
    pub storage_path: PathBuf,
    /// Maximum file size before rotation
    pub max_file_size: u64,
    /// Current file offset for new entries
    pub write_offset: AtomicU64,
    /// Compression algorithm for stored data
    pub compression_algorithm: CompressionAlgorithm,
    /// Phantom data to maintain type parameter
    _phantom: PhantomData<V>,
}

/// Global cold tier statistics
static COLD_HITS: AtomicU64 = AtomicU64::new(0);
static COLD_MISSES: AtomicU64 = AtomicU64::new(0);
static COLD_WRITES: AtomicU64 = AtomicU64::new(0);
static COLD_READS: AtomicU64 = AtomicU64::new(0);
static COLD_ENTRIES: AtomicUsize = AtomicUsize::new(0);
static COLD_STORAGE_BYTES: AtomicU64 = AtomicU64::new(0);

impl<K: CacheKey, V: CacheValue> ColdTierCache<K, V> {
    /// Create new cold tier cache
    pub fn new<P: AsRef<Path>>(
        storage_path: P,
        compression_algorithm: CompressionAlgorithm,
    ) -> Result<Self, CacheOperationError> {
        let storage_path = storage_path.as_ref().to_path_buf();

        // Ensure storage directory exists
        if let Some(parent) = storage_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;
        }

        // Open or create storage file
        let storage_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&storage_path)
            .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;

        let cache = Self {
            storage_file: Arc::new(Mutex::new(storage_file)),
            index: Arc::new(Mutex::new(AHashMap::new())),
            storage_path,
            max_file_size: 128 * 1024 * 1024, // 128MB default
            write_offset: AtomicU64::new(0),
            compression_algorithm,
            _phantom: PhantomData,
        };

        // Initialize write offset from file size
        let file_size = cache.get_file_size()?;
        cache.write_offset.store(file_size, Ordering::Relaxed);

        Ok(cache)
    }

    /// Get entry from cache - crossbeam zero-copy reference
    pub fn get(&self, key: &K) -> Result<Option<V>, CacheOperationError> {
        let index = self
            .index
            .lock()
            .map_err(|_| CacheOperationError::invalid_state("Index lock poisoned"))?;

        // Connect directly to sophisticated PersistentColdTier that already exists
        drop(index); // Release lock before calling advanced system
        
        use crate::cache::tier::cold::core::operations::PersistentColdTier;
        
        match PersistentColdTier::get(key) {
            Ok(Some(value)) => {
                Self::record_hit();
                Ok(Some(value))
            },
            Ok(None) => {
                Self::record_miss();
                Ok(None)
            },
            Err(e) => {
                Self::record_miss();
                Err(e)
            }
        }
    }

    /// Put entry into cache - crossbeam owns values directly
    pub fn put(&self, key: K, value: V) -> Result<(), CacheOperationError> {
        // Connect directly to sophisticated PersistentColdTier
        use crate::cache::tier::cold::core::operations::PersistentColdTier;
        
        let result = PersistentColdTier::put(key, value);
        if result.is_ok() {
            Self::record_write(std::mem::size_of::<V>() as u32);
        }
        result
    }

    /// Sync all data to disk
    pub fn sync_to_disk(&self) -> Result<(), CacheOperationError> {
        let file = self
            .storage_file
            .lock()
            .map_err(|_| CacheOperationError::resource_exhausted("Storage file lock poisoned"))?;
        file.sync_all()
            .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;
        Ok(())
    }

    /// Clean up expired entries
    pub fn cleanup_expired(&self) -> Result<usize, CacheOperationError> {
        use crate::cache::tier::cold::optimization::{StorageOptimizer, OptimizationConfig};
        
        // Connect to existing sophisticated optimization system
        let optimizer_config = OptimizationConfig::default();
        let mut optimizer = StorageOptimizer::new(optimizer_config);
        
        // Use existing sophisticated remove_expired_entries() method
        // This method already implements:
        // - TTL logic using entry.last_access.elapsed() > max_idle_duration
        // - Proper index iteration with thread-safe locking
        // - Actual entry removal using cache.remove(&key)
        // - Statistics tracking and error handling
        let removed_count = optimizer.remove_expired_entries(self)?;
        
        println!("Cold tier cleanup completed: {} expired entries removed", removed_count);
        
        Ok(removed_count)
    }

    /// Validate storage integrity
    pub fn validate_integrity(&self) -> Result<bool, CacheOperationError> {
        use crate::cache::tier::cold::serialization::utilities::calculate_checksum;
        use std::io::{Read, Seek, SeekFrom};
        
        let index = self.index.lock()
            .map_err(|_| CacheOperationError::resource_exhausted("Index lock poisoned"))?;
        
        let mut validation_errors = 0;
        let total_entries = index.len();
        
        // Validate each entry using existing sophisticated systems
        for (key, entry) in index.iter() {
            // Read data from storage file at the stored offset
            let mut storage_file = self.storage_file.lock()
                .map_err(|_| CacheOperationError::resource_exhausted("Storage file lock poisoned"))?;
            
            // Seek to entry position
            if let Err(_) = storage_file.seek(SeekFrom::Start(entry.file_offset)) {
                validation_errors += 1;
                eprintln!("Failed to seek to offset {} for key validation", entry.file_offset);
                continue;
            }
            
            // Read the stored data
            let mut data = vec![0u8; entry.data_size as usize];
            if let Err(_) = storage_file.read_exact(&mut data) {
                validation_errors += 1;
                eprintln!("Failed to read data for key validation");
                continue;
            }
            
            // Validate checksum using existing sophisticated calculation
            let calculated_checksum = calculate_checksum(&data);
            if calculated_checksum != entry.checksum {
                validation_errors += 1;
                eprintln!("Checksum mismatch detected: expected {}, got {}", 
                         entry.checksum, calculated_checksum);
            }
        }
        
        let integrity_valid = validation_errors == 0;
        if !integrity_valid {
            eprintln!("Storage integrity validation failed: {}/{} entries corrupted", 
                     validation_errors, total_entries);
        } else {
            println!("Storage integrity validation passed: {}/{} entries verified", 
                    total_entries, total_entries);
        }
        
        Ok(integrity_valid)
    }

    /// Get current file size
    pub fn get_file_size(&self) -> Result<u64, CacheOperationError> {
        let file = self
            .storage_file
            .lock()
            .map_err(|_| CacheOperationError::resource_exhausted("Storage file lock poisoned"))?;
        let metadata = file
            .metadata()
            .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;
        Ok(metadata.len())
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> Result<ColdTierStats, CacheOperationError> {
        let index = self
            .index
            .lock()
            .map_err(|_| CacheOperationError::resource_exhausted("Index lock poisoned"))?;
        Ok(ColdTierStats {
            hits: COLD_HITS.load(Ordering::Relaxed),
            misses: COLD_MISSES.load(Ordering::Relaxed),
            entries: index.len(),
            storage_bytes: COLD_STORAGE_BYTES.load(Ordering::Relaxed),
        })
    }

    /// Check if key exists in cache
    pub fn contains_key(&self, key: &K) -> Result<bool, CacheOperationError> {
        let index = self
            .index
            .lock()
            .map_err(|_| CacheOperationError::resource_exhausted("Index lock poisoned"))?;
        Ok(index.contains_key(key))
    }

    /// Get entry count
    pub fn len(&self) -> Result<usize, CacheOperationError> {
        let index = self
            .index
            .lock()
            .map_err(|_| CacheOperationError::resource_exhausted("Index lock poisoned"))?;
        Ok(index.len())
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> Result<bool, CacheOperationError> {
        let index = self
            .index
            .lock()
            .map_err(|_| CacheOperationError::resource_exhausted("Index lock poisoned"))?;
        Ok(index.is_empty())
    }

    /// Clear all entries from cache
    pub fn clear(&self) -> Result<(), CacheOperationError> {
        let mut index = self
            .index
            .lock()
            .map_err(|_| CacheOperationError::resource_exhausted("Index lock poisoned"))?;
        index.clear();

        // Reset file to empty
        let file = self
            .storage_file
            .lock()
            .map_err(|_| CacheOperationError::resource_exhausted("Storage file lock poisoned"))?;
        file.set_len(0)
            .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;

        self.write_offset.store(0, Ordering::Relaxed);
        COLD_ENTRIES.store(0, Ordering::Relaxed);
        COLD_STORAGE_BYTES.store(0, Ordering::Relaxed);

        Ok(())
    }

    /// Remove entry from cache
    pub fn remove(&self, key: &K) -> Result<bool, CacheOperationError> {
        let mut index = self
            .index
            .lock()
            .map_err(|_| CacheOperationError::resource_exhausted("Index lock poisoned"))?;
        if let Some(entry) = index.remove(key) {
            COLD_ENTRIES.fetch_sub(1, Ordering::Relaxed);
            COLD_STORAGE_BYTES.fetch_sub(entry.data_size as u64, Ordering::Relaxed);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get storage path
    pub fn storage_path(&self) -> &Path {
        &self.storage_path
    }

    /// Get current write offset
    pub fn write_offset(&self) -> u64 {
        self.write_offset.load(Ordering::Relaxed)
    }

    /// Check if file rotation is needed
    pub fn needs_rotation(&self) -> bool {
        self.write_offset() >= self.max_file_size
    }

    /// Set maximum file size before rotation
    pub fn set_max_file_size(&mut self, max_size: u64) {
        self.max_file_size = max_size;
    }

    /// Get entry metadata if it exists
    pub fn get_entry_metadata(&self, key: &K) -> Result<Option<ColdEntry>, CacheOperationError> {
        let index = self
            .index
            .lock()
            .map_err(|_| CacheOperationError::resource_exhausted("Index lock poisoned"))?;
        Ok(index.get(key).cloned())
    }

    /// Update entry access information
    pub fn update_access(&self, key: &K) -> Result<(), CacheOperationError> {
        let mut index = self
            .index
            .lock()
            .map_err(|_| CacheOperationError::resource_exhausted("Index lock poisoned"))?;
        if let Some(entry) = index.get_mut(key) {
            entry.last_access = Instant::now();
            entry.access_count += 1;
        }
        Ok(())
    }

    /// Get entries sorted by access frequency
    pub fn get_entries_by_frequency(&self) -> Result<Vec<(K, ColdEntry)>, CacheOperationError> {
        let index = self
            .index
            .lock()
            .map_err(|_| CacheOperationError::resource_exhausted("Index lock poisoned"))?;
        let mut entries: Vec<_> = index.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        entries.sort_by(|a, b| b.1.access_count.cmp(&a.1.access_count));
        Ok(entries)
    }

    /// Get least recently used entries
    pub fn get_lru_entries(
        &self,
        count: usize,
    ) -> Result<Vec<(K, ColdEntry)>, CacheOperationError> {
        let index = self
            .index
            .lock()
            .map_err(|_| CacheOperationError::resource_exhausted("Index lock poisoned"))?;
        let mut entries: Vec<_> = index.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        entries.sort_by(|a, b| a.1.last_access.cmp(&b.1.last_access));
        Ok(entries.into_iter().take(count).collect())
    }

    /// Record cache hit
    pub fn record_hit() {
        COLD_HITS.fetch_add(1, Ordering::Relaxed);
        COLD_READS.fetch_add(1, Ordering::Relaxed);
    }

    /// Record cache miss
    pub fn record_miss() {
        COLD_MISSES.fetch_add(1, Ordering::Relaxed);
    }

    /// Record cache write
    pub fn record_write(size: u32) {
        COLD_WRITES.fetch_add(1, Ordering::Relaxed);
        COLD_ENTRIES.fetch_add(1, Ordering::Relaxed);
        COLD_STORAGE_BYTES.fetch_add(size as u64, Ordering::Relaxed);
    }

    /// Compact storage to reduce fragmentation
    pub fn compact(&self) -> Result<(), CacheOperationError> {
        use crate::cache::tier::cold::optimization::{StorageOptimizer, OptimizationConfig};
        
        // Connect to existing sophisticated optimization system
        let optimizer_config = OptimizationConfig::default();
        let mut optimizer = StorageOptimizer::new(optimizer_config);
        
        // Use existing optimization system that includes real compaction
        let result = optimizer.optimize_storage(self)?;
        
        // Log compaction results using existing metrics
        println!("Compaction completed: {} entries removed, efficiency improved from {:.2} to {:.2}",
                 result.entries_removed,
                 result.efficiency_before,
                 result.efficiency_after);
        
        Ok(())
    }

    /// Create new cold tier cache with configuration
    pub fn new_with_config<P: AsRef<Path>>(
        storage_path: P,
        max_file_size: u64,
        compression_algorithm: CompressionAlgorithm,
    ) -> Result<Self, CacheOperationError> {
        let storage_path = storage_path.as_ref().to_path_buf();

        // Ensure storage directory exists
        if let Some(parent) = storage_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;
        }

        // Open or create storage file
        let storage_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&storage_path)
            .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;

        let cache = Self {
            storage_file: Arc::new(Mutex::new(storage_file)),
            index: Arc::new(Mutex::new(AHashMap::new())),
            storage_path,
            max_file_size,
            write_offset: AtomicU64::new(0),
            compression_algorithm,
            _phantom: PhantomData,
        };

        Ok(cache)
    }

    /// Get cache statistics
    pub fn stats(&self) -> ColdTierStats {
        ColdTierStats {
            hits: COLD_HITS.load(Ordering::Relaxed),
            misses: COLD_MISSES.load(Ordering::Relaxed),
            entries: COLD_ENTRIES.load(Ordering::Relaxed),
            storage_bytes: COLD_STORAGE_BYTES.load(Ordering::Relaxed),
        }
    }
}

impl ColdEntry {
    /// Create new cold entry
    pub fn new(file_offset: u64, data_size: u32, checksum: u32) -> Self {
        Self {
            file_offset,
            data_size,
            checksum,
            last_access: Instant::now(),
            access_count: 1,
        }
    }

    /// Check if entry is recently accessed
    pub fn is_recently_accessed(&self, threshold: std::time::Duration) -> bool {
        self.last_access.elapsed() < threshold
    }

    /// Check if entry is frequently accessed
    pub fn is_frequently_accessed(&self, threshold: u64) -> bool {
        self.access_count >= threshold
    }

    /// Get age of entry
    pub fn age(&self) -> std::time::Duration {
        self.last_access.elapsed()
    }
}

impl Default for ColdTierStats {
    fn default() -> Self {
        Self {
            hits: 0,
            misses: 0,
            entries: 0,
            storage_bytes: 0,
        }
    }
}
