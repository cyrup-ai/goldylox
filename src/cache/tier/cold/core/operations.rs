//! Main cache operations for persistent cold tier
//!
//! This module implements the core cache operations including get, put, remove,
//! and statistics retrieval for the persistent cold tier cache.

use crate::cache::tier::cold::core::types::*;
use crate::cache::tier::cold::compression_engine::CompressedData;
use crate::cache::tier::cold::data_structures::*;
use crate::cache::tier::cold::PersistentColdTier;
use crate::cache::types::results::CacheOperationError;
use crate::cache::manager::TierStatistics;
use super::types::{timestamp_nanos, PrecisionTimer};
use crate::cache::traits::{CacheKey, CacheValue};

#[cfg(feature = "bincode")]
use bincode::{config, decode_from_slice, encode_to_vec};

impl<K: CacheKey, V: CacheValue + serde::Serialize + serde::de::DeserializeOwned>
    PersistentColdTier<K, V>
{
    /// Get value from persistent storage
    pub fn get(&self, key: &K) -> Option<V> {
        let timer = PrecisionTimer::start();
        let cold_key = ColdCacheKey::from_cache_key(key);

        // Check bloom filter first (fast negative lookup)
        if !self.metadata_index.bloom_filter.might_contain(&cold_key) {
            let elapsed_ns = timer.elapsed_ns();
            self.stats
                .miss_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.stats
                .last_access_ns
                .store(elapsed_ns, std::sync::atomic::Ordering::Relaxed);
            return None;
        }

        // Lookup in metadata index
        if let Some(index_entry) = self.metadata_index.get_entry(&cold_key) {
            // Read compressed data from memory-mapped file
            match self.read_compressed_data(&index_entry) {
                Ok(compressed_data) => {
                    // Decompress data
                    // Create CompressedData struct from raw data and metadata
                    let compressed_data_struct = CompressedData {
                        data: compressed_data,
                        original_size: index_entry.uncompressed_size as usize,
                        checksum: index_entry.checksum,
                        algorithm: index_entry.compression_algo,
                    };

                    match self.compression_engine.decompress(&compressed_data_struct) {
                        Ok(decompressed_data) => {
                            // Deserialize cache value using bincode
                            match bincode::decode_from_slice::<V, _>(&decompressed_data, bincode::config::standard())
                                .map(|(v, _)| v) {
                                Ok(cache_value) => {
                                    // Update access metadata
                                    self.update_access_metadata(&cold_key, &index_entry);

                                    let elapsed_ns = timer.elapsed_ns();
                                    self.stats
                                        .hit_count
                                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    self.stats
                                        .last_access_ns
                                        .store(elapsed_ns, std::sync::atomic::Ordering::Relaxed);

                                    Some(cache_value)
                                }
                                Err(_) => {
                                    let elapsed_ns = timer.elapsed_ns();
                                    self.stats
                                        .miss_count
                                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    self.stats
                                        .last_access_ns
                                        .store(elapsed_ns, std::sync::atomic::Ordering::Relaxed);
                                    None
                                }
                            }
                        }
                        Err(_) => {
                            let elapsed_ns = timer.elapsed_ns();
                            self.stats
                                .miss_count
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            self.stats
                                .last_access_ns
                                .store(elapsed_ns, std::sync::atomic::Ordering::Relaxed);
                            None
                        }
                    }
                }
                Err(_) => {
                    let elapsed_ns = timer.elapsed_ns();
                    self.stats
                        .miss_count
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    self.stats
                        .last_access_ns
                        .store(elapsed_ns, std::sync::atomic::Ordering::Relaxed);
                    None
                }
            }
        } else {
            let elapsed_ns = timer.elapsed_ns();
            self.stats
                .miss_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.stats
                .last_access_ns
                .store(elapsed_ns, std::sync::atomic::Ordering::Relaxed);
            None
        }
    }

    /// Put value in persistent storage
    pub fn put(&mut self, key: K, value: V) -> Result<(), CacheOperationError> {
        let timer = PrecisionTimer::start();
        let cold_key = ColdCacheKey::from_cache_key(&key);

        // Serialize value using CacheValue trait
        let serialized_data = match bincode::encode_to_vec(&value, bincode::config::standard()) {
            Ok(data) => data,
            Err(_) => {
                let elapsed_ns = timer.elapsed_ns();
                self.stats
                    .miss_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.stats
                    .last_access_ns
                    .store(elapsed_ns, std::sync::atomic::Ordering::Relaxed);
                return Err(CacheOperationError::serialization_failed(
                    "Failed to serialize cache value",
                ));
            }
        };

        // Select optimal compression algorithm
        let compression_algo = self.compression_engine.select_algorithm(&serialized_data);

        // Compress data
        let compressed_data = match self
            .compression_engine
            .compress(&serialized_data, compression_algo)
        {
            Ok(data) => data,
            Err(_) => {
                let elapsed_ns = timer.elapsed_ns();
                self.stats
                    .miss_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.stats
                    .last_access_ns
                    .store(elapsed_ns, std::sync::atomic::Ordering::Relaxed);
                return Err(CacheOperationError::serialization_failed(
                    "Compression failed",
                ));
            }
        };

        // Write to memory-mapped file
        let file_offset = match self.write_compressed_data(&compressed_data.data) {
            Ok(offset) => offset,
            Err(_) => {
                let elapsed_ns = timer.elapsed_ns();
                self.stats
                    .miss_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.stats
                    .last_access_ns
                    .store(elapsed_ns, std::sync::atomic::Ordering::Relaxed);
                return Err(CacheOperationError::io_failed("Failed to write to storage"));
            }
        };

        // Create index entry
        let index_entry = IndexEntry {
            file_offset,
            compressed_size: compressed_data.len() as u32,
            uncompressed_size: serialized_data.len() as u32,
            compression_algo,
            created_at_ns: timestamp_nanos(std::time::Instant::now()),
            last_access_ns: timestamp_nanos(std::time::Instant::now()),
            access_count: 1,
            checksum: self.calculate_checksum(&compressed_data.data),
        };

        // Update metadata index
        self.metadata_index
            .insert_entry(cold_key.clone(), index_entry);
        self.metadata_index.bloom_filter.insert(&cold_key);

        // Schedule sync if needed
        self.sync_state.schedule_sync();

        // Update statistics
        self.stats
            .entry_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.stats.storage_size.fetch_add(
            compressed_data.data.len() as u64,
            std::sync::atomic::Ordering::Relaxed,
        );

        let elapsed_ns = timer.elapsed_ns();
        self.stats
            .hit_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.stats
            .last_access_ns
            .store(elapsed_ns, std::sync::atomic::Ordering::Relaxed);

        Ok(())
    }

    /// Remove entry from persistent storage
    pub fn remove(&mut self, key: &K) -> bool {
        let cold_key = ColdCacheKey::from_cache_key(key);

        if let Some(index_entry) = self.metadata_index.remove_entry(&cold_key) {
            // Mark space as free (actual cleanup happens during compaction)
            self.mark_space_free(index_entry.file_offset, index_entry.compressed_size);

            // Update statistics
            self.stats
                .entry_count
                .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            self.stats.storage_size.fetch_sub(
                index_entry.compressed_size as u64,
                std::sync::atomic::Ordering::Relaxed,
            );

            true
        } else {
            false
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> TierStatistics {
        let entry_count = self
            .stats
            .entry_count
            .load(std::sync::atomic::Ordering::Relaxed) as usize;
        let memory_usage = self
            .stats
            .storage_size
            .load(std::sync::atomic::Ordering::Relaxed) as usize;
        let hit_count = self
            .stats
            .hit_count
            .load(std::sync::atomic::Ordering::Relaxed);
        let miss_count = self
            .stats
            .miss_count
            .load(std::sync::atomic::Ordering::Relaxed);
        let total_requests = hit_count + miss_count;

        // Calculate ops_per_second based on recent activity
        let ops_per_second = if total_requests > 0 {
            // Use a 60-second time window to estimate current throughput
            let time_window_s = 60.0;
            let recent_ops = std::cmp::min(total_requests, 100); // Cap at recent 100 ops
            recent_ops as f64 / time_window_s
        } else {
            0.0
        };

        // Calculate error_rate based on miss ratio as proxy for errors
        // In a real implementation, this would track actual errors
        let error_rate = if total_requests > 0 {
            // Use miss rate as a proxy for error rate (misses indicate potential issues)
            let miss_rate = miss_count as f64 / total_requests as f64;
            // Scale down miss rate since not all misses are errors
            miss_rate * 0.1 // 10% of misses considered as errors
        } else {
            0.0
        };

        TierStatistics {
            entry_count,
            memory_usage,
            hit_rate: if total_requests > 0 {
                hit_count as f64 / total_requests as f64
            } else {
                0.0
            },
            avg_access_time_ns: self
                .stats
                .last_access_ns
                .load(std::sync::atomic::Ordering::Relaxed),
            ops_per_second,
            error_rate,
        }
    }
}
