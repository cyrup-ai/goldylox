//! Compaction and maintenance operations for persistent cold tier
//!
//! This module implements lock-free compaction algorithms that run in the
//! ColdTierService worker thread with exclusive access to tier data.

use std::io::Write;
use std::sync::atomic::Ordering;

use crate::cache::tier::cold::data_structures::{CompressionAlgorithm, PersistentColdTier};
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

impl<
    K: CacheKey + Default,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>,
> PersistentColdTier<K, V>
{
    /// Compact data file to remove fragmentation
    /// 
    /// This method:
    /// 1. Collects all valid entries from the current data file
    /// 2. Writes them sequentially to a new temporary file (removes gaps)
    /// 3. Atomically replaces the old file with the compacted one
    /// 4. Updates all metadata indexes with new offsets
    /// 5. Returns the number of bytes reclaimed
    pub fn compact_data_file(&mut self) -> Result<u64, CacheOperationError> {
        let state = &self.compaction_system.compaction_state;
        state.progress.store(0.1);
        state.is_compacting.store(true, Ordering::SeqCst);

        let start_time = std::time::Instant::now();

        // Step 1: Access storage and index directly (worker owns the data)
        let storage = &mut self.storage_manager;
        let index = &mut self.metadata_index;

        state.progress.store(0.2);

        // Step 2: Collect all valid entries with their data
        let mut valid_entries = Vec::new();

        if let Some(ref mmap) = storage.data_file {
            for (key, entry) in index.key_index.iter() {
                let start = entry.file_offset as usize;
                let end = start + entry.compressed_size as usize;

                if end <= mmap.len() {
                    let data = mmap[start..end].to_vec();
                    valid_entries.push((key.clone(), data, entry.clone()));
                }
            }
        } else {
            log::warn!("No data file available for compaction");
            state.progress.store(1.0);
            state.is_compacting.store(false, Ordering::SeqCst);
            return Err(CacheOperationError::io_failed("No data file mapped"));
        }

        state.progress.store(0.5);

        // Step 3: Write compacted data to temp file
        let temp_path = storage.data_path.with_extension("tmp");
        let mut temp_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&temp_path)
            .map_err(|e| {
                log::error!("Failed to create temp file: {}", e);
                state.progress.store(1.0);
                state.is_compacting.store(false, Ordering::SeqCst);
                CacheOperationError::io_failed(format!("Failed to create temp file: {}", e))
            })?;

        let mut new_offset = 0u64;
        let mut new_index_entries = Vec::new();

        for (key, data, mut entry) in valid_entries {
            temp_file.write_all(&data).map_err(|e| {
                log::error!("Failed to write compacted data: {}", e);
                state.progress.store(1.0);
                state.is_compacting.store(false, Ordering::SeqCst);
                CacheOperationError::io_failed(format!("Failed to write compacted data: {}", e))
            })?;

            // Update entry with new offset
            entry.file_offset = new_offset;
            new_offset += data.len() as u64;

            new_index_entries.push((key, entry));
        }

        temp_file.sync_all().map_err(|e| {
            log::error!("Failed to sync temp file: {}", e);
            state.progress.store(1.0);
            state.is_compacting.store(false, Ordering::SeqCst);
            CacheOperationError::io_failed(format!("Failed to sync temp file: {}", e))
        })?;
        drop(temp_file);

        state.progress.store(0.8);

        // Step 4: Calculate bytes reclaimed
        let old_size = storage.write_position.load(Ordering::Relaxed);
        let new_size = new_offset;
        let bytes_reclaimed = old_size.saturating_sub(new_size);

        // Step 5: Atomically replace old file with compacted file
        std::fs::rename(&temp_path, &storage.data_path).map_err(|e| {
            // Clean up temp file on failure to prevent resource leak
            if let Err(cleanup_err) = std::fs::remove_file(&temp_path) {
                log::warn!("Failed to clean up temp file after rename error: {}", cleanup_err);
            }
            log::error!("Failed to replace data file: {}", e);
            state.progress.store(1.0);
            state.is_compacting.store(false, Ordering::SeqCst);
            CacheOperationError::io_failed(format!("Failed to replace data file: {}", e))
        })?;

        // Step 5b: Remap the file after replacement
        // The old mmap still points to the deleted file, we need to remap to the new compacted file
        storage.data_file = None; // Drop old mmap
        
        // Reopen the data file handle to point to the new compacted file
        let new_data_handle = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&storage.data_path)
            .map_err(|e| {
                log::error!("Failed to reopen data file after compaction: {}", e);
                state.progress.store(1.0);
                state.is_compacting.store(false, Ordering::SeqCst);
                CacheOperationError::io_failed(format!("Failed to reopen data file: {}", e))
            })?;

        // Ensure the file has the correct size
        new_data_handle.set_len(storage.max_data_size).map_err(|e| {
            log::error!("Failed to resize compacted file: {}", e);
            state.progress.store(1.0);
            state.is_compacting.store(false, Ordering::SeqCst);
            CacheOperationError::io_failed(format!("Failed to resize file: {}", e))
        })?;

        // Create new mmap for the compacted file
        use memmap2::MmapOptions;
        storage.data_file = Some(unsafe {
            MmapOptions::new().map_mut(&new_data_handle).map_err(|e| {
                log::error!("Failed to remap data file: {}", e);
                state.progress.store(1.0);
                state.is_compacting.store(false, Ordering::SeqCst);
                CacheOperationError::io_failed(format!("Failed to remap data file: {}", e))
            })?
        });

        // Update the data handle to the new one
        storage.data_handle = Some(new_data_handle);

        // Step 6: Update metadata index with new offsets
        for (key, entry) in new_index_entries {
            index.key_index.insert(key, entry);
        }

        // Step 7: Update write position
        storage.write_position.store(new_size, Ordering::Relaxed);

        // Update compaction statistics
        let duration = start_time.elapsed();
        state.bytes_reclaimed.store(bytes_reclaimed, Ordering::Relaxed);
        state
            .last_duration_ns
            .store(duration.as_nanos() as u64, Ordering::Relaxed);
        state.progress.store(1.0);
        state.is_compacting.store(false, Ordering::SeqCst);

        log::info!("Compaction completed: {} bytes reclaimed", bytes_reclaimed);

        Ok(bytes_reclaimed)
    }

    /// Cleanup expired entries - remove entries that haven't been accessed in 7 days
    ///
    /// This method:
    /// 1. Scans the metadata index for entries with old last_access_ns timestamps
    /// 2. Removes expired entries from the index
    /// 3. Marks their storage space as reclaimable (compaction will reclaim it)
    /// 4. Returns the count of removed entries
    pub fn cleanup_expired_entries(&mut self) -> Result<usize, CacheOperationError> {
        let state = &self.compaction_system.compaction_state;
        state.progress.store(0.2);

        let current_time_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| {
                log::error!("Failed to get current time: {}", e);
                state.progress.store(1.0);
                CacheOperationError::internal_error(format!("Failed to get current time: {}", e))
            })?
            .as_nanos() as u64;

        // Expiration threshold: 7 days of no access
        const MAX_IDLE_NS: u64 = 7 * 24 * 60 * 60 * 1_000_000_000;

        state.progress.store(0.3);

        // Step 1: Access index directly (worker owns the data)
        let index = &mut self.metadata_index;

        let expired_keys: Vec<_> = index
            .key_index
            .iter()
            .filter_map(|(key, entry)| {
                let idle_time = current_time_ns.saturating_sub(entry.last_access_ns);
                if idle_time > MAX_IDLE_NS {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();

        state.progress.store(0.6);

        // Step 2: Remove from metadata index and update statistics
        let mut total_bytes_freed = 0u64;
        for key in &expired_keys {
            if let Some(entry) = index.key_index.remove(key) {
                total_bytes_freed += entry.compressed_size as u64;
                
                // Update statistics for each removed entry
                self.stats.update_entry_count(-1);
                self.stats.update_memory_usage(-(entry.compressed_size as i64));
            }
        }

        state.progress.store(0.9);

        // Step 3: Mark space as reclaimable (actual file cleanup happens during compaction)
        // The removed entries leave gaps in the data file that compaction will reclaim

        state.progress.store(1.0);

        log::info!(
            "Expired entry cleanup: removed {} entries, {} bytes marked for reclaim",
            expired_keys.len(),
            total_bytes_freed
        );

        Ok(expired_keys.len())
    }

    /// Optimize compression parameters - test algorithms and select the best one
    ///
    /// This method:
    /// 1. Samples entries from the cold tier
    /// 2. Tests different compression algorithms (LZ4, Zstd, Snappy, Brotli)
    /// 3. Scores each based on compression ratio and speed
    /// 4. Sets the compression engine to use the best algorithm
    /// 5. Returns the selected algorithm
    pub fn optimize_compression_parameters(
        &mut self,
    ) -> Result<CompressionAlgorithm, CacheOperationError> {
        let state = &self.compaction_system.compaction_state;
        state.progress.store(0.1);

        // Step 1: Access storage and index directly (worker owns the data)
        let index = &self.metadata_index;
        let storage = &self.storage_manager;

        let sample_size = 100.min(index.key_index.len());
        let sample_entries: Vec<_> = index.key_index.values().take(sample_size).collect();

        state.progress.store(0.3);

        // Step 2: Read sample data
        let mut sample_data = Vec::new();
        if let Some(ref mmap) = storage.data_file {
            for entry in sample_entries {
                let start = entry.file_offset as usize;
                let end = start + entry.compressed_size as usize;

                if end <= mmap.len() {
                    sample_data.push(mmap[start..end].to_vec());
                }
            }
        } else {
            log::warn!("No data file available for compression optimization");
            state.progress.store(1.0);
            return Err(CacheOperationError::io_failed(
                "No data file available for compression optimization",
            ));
        }

        state.progress.store(0.5);

        // Step 3: Test compression algorithms
        let algorithms = vec![
            CompressionAlgorithm::Lz4,
            CompressionAlgorithm::Zstd,
            CompressionAlgorithm::Snappy,
            CompressionAlgorithm::Brotli,
        ];

        let mut best_algorithm = CompressionAlgorithm::Lz4;
        let mut best_score = 0.0f64;

        for algo in algorithms {
            self.compression_engine.set_algorithm(algo);

            let mut total_ratio = 0.0;
            let mut test_count = 0;

            // Test on samples
            for data in &sample_data {
                let original_size = data.len();

                // Compress
                let start = std::time::Instant::now();
                if let Ok(compressed) = self.compression_engine.compress(data, algo) {
                    let compress_time = start.elapsed();

                    let ratio = compressed.data.len() as f64 / original_size as f64;
                    let speed = original_size as f64 / compress_time.as_secs_f64();

                    // Score = compression ratio + speed factor
                    let score = (1.0 - ratio) + (speed / 1_000_000.0); // Normalize speed
                    total_ratio += score;
                    test_count += 1;
                }
            }

            let avg_score = if test_count > 0 {
                total_ratio / test_count as f64
            } else {
                0.0
            };

            if avg_score > best_score {
                best_score = avg_score;
                best_algorithm = algo;
            }
        }

        state.progress.store(0.9);

        // Step 4: Apply best algorithm
        self.compression_engine.set_algorithm(best_algorithm);

        state.progress.store(1.0);

        log::info!(
            "Compression optimization: selected {:?} (score: {:.4})",
            best_algorithm,
            best_score
        );

        Ok(best_algorithm)
    }
}
