//! Main cache operations for persistent cold tier
//!
//! This module implements the core cache operations including get, put, remove,
//! and statistics retrieval for the persistent cold tier cache.

use super::types::timestamp_nanos;
use crate::cache::tier::cold::PersistentColdTier;
use crate::cache::tier::cold::compression_engine::CompressedData;
use crate::cache::tier::cold::data_structures::*;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::types::performance::timer::PrecisionTimer;
use crate::cache::types::statistics::multi_tier::ErrorType;
use crate::cache::types::statistics::tier_stats::TierStatistics;

use bincode;

impl<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()>,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Decode<()>
        + bincode::Encode,
> PersistentColdTier<K, V>
{
    /// Get value from persistent storage
    pub fn get(&self, key: &K) -> Option<V> {
        let timer = PrecisionTimer::start();
        let cold_key = ColdCacheKey::from_cache_key(key);

        // Access metadata index directly (worker owns the data, no locks needed)
        let index = &self.metadata_index;

        // Check bloom filter first (fast negative lookup)
        if !index.bloom_filter.might_contain(&cold_key) {
            let elapsed_ns = timer.elapsed_ns();
            self.stats.record_miss(elapsed_ns);
            return None;
        }

        // Lookup in metadata index
        if let Some(index_entry) = index.get_entry(&cold_key) {
            // Clone the entry before I/O operations
            let index_entry_clone = index_entry.clone();

            // Read compressed data from memory-mapped file
            match self.read_compressed_data(&index_entry_clone) {
                Ok(compressed_data) => {
                    // Decompress data
                    // Create CompressedData struct from raw data and metadata
                    let compressed_data_struct = CompressedData {
                        data: compressed_data,
                        original_size: index_entry_clone.uncompressed_size as usize,
                        checksum: index_entry_clone.checksum,
                        algorithm: index_entry_clone.compression_algo,
                    };

                    match self.compression_engine.decompress(&compressed_data_struct) {
                        Ok(decompressed_data) => {
                            // Deserialize cache value using bincode
                            match bincode::decode_from_slice::<V, _>(
                                &decompressed_data,
                                bincode::config::standard(),
                            )
                            .map(|(v, _)| v)
                            {
                                Ok(cache_value) => {
                                    // Update access metadata
                                    self.update_access_metadata(
                                        &cold_key,
                                        &mut index_entry.clone(),
                                    );

                                    let elapsed_ns = timer.elapsed_ns();
                                    self.stats.record_hit(elapsed_ns);

                                    Some(cache_value)
                                }
                                Err(_) => {
                                    // Track deserialization error
                                    self.error_stats.record_error(ErrorType::CorruptedData);
                                    let elapsed_ns = timer.elapsed_ns();
                                    self.stats.record_miss(elapsed_ns);
                                    None
                                }
                            }
                        }
                        Err(_) => {
                            // Track decompression error
                            self.error_stats.record_error(ErrorType::CorruptedData);
                            let elapsed_ns = timer.elapsed_ns();
                            self.stats.record_miss(elapsed_ns);
                            None
                        }
                    }
                }
                Err(_) => {
                    // Track IO error
                    self.error_stats.record_error(ErrorType::DiskIOError);
                    let elapsed_ns = timer.elapsed_ns();
                    self.stats.record_miss(elapsed_ns);
                    None
                }
            }
        } else {
            let elapsed_ns = timer.elapsed_ns();
            self.stats.record_miss(elapsed_ns);
            None
        }
    }

    /// Put value in persistent storage
    pub fn put(&mut self, key: K, value: V) -> Result<(), CacheOperationError> {
        let timer = PrecisionTimer::start();

        // Use serialized approach for round-trip key recovery capability - bincode always available
        let cold_key = ColdCacheKey::from_cache_key_serialized(&key).map_err(|_| {
            CacheOperationError::SerializationError("Failed to serialize key".to_string())
        })?;

        // Serialize value using CacheValue trait
        let serialized_data = match bincode::encode_to_vec(&value, bincode::config::standard()) {
            Ok(data) => data,
            Err(_) => {
                // Track serialization error
                self.error_stats.record_error(ErrorType::CorruptedData);
                let elapsed_ns = timer.elapsed_ns();
                self.stats.record_miss(elapsed_ns);
                return Err(CacheOperationError::serialization_failed(
                    "Failed to serialize cache value",
                ));
            }
        };

        // Track uncompressed size for compression ratio calculation
        let uncompressed_size = serialized_data.len() as u64;

        // Select optimal compression algorithm
        let compression_algo = self.compression_engine.select_algorithm(&serialized_data);

        // Compress data
        let compressed_data = match self
            .compression_engine
            .compress(&serialized_data, compression_algo)
        {
            Ok(data) => data,
            Err(_) => {
                // Track compression error
                self.error_stats.record_error(ErrorType::CorruptedData);
                let elapsed_ns = timer.elapsed_ns();
                self.stats.record_miss(elapsed_ns);
                return Err(CacheOperationError::serialization_failed(
                    "Compression failed",
                ));
            }
        };

        // Write to memory-mapped file
        let file_offset = match self.write_compressed_data(&compressed_data.data) {
            Ok(offset) => offset,
            Err(_) => {
                // Track storage write error
                self.error_stats.record_error(ErrorType::DiskIOError);
                let elapsed_ns = timer.elapsed_ns();
                self.stats.record_miss(elapsed_ns);
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

        // Update metadata index (worker owns the data, no locks needed)
        self.metadata_index.insert_entry(cold_key.clone(), index_entry);
        self.metadata_index.bloom_filter.insert(&cold_key);

        // Schedule sync if needed
        self.sync_state.schedule_sync();

        // Update statistics
        self.stats.update_entry_count(1);
        self.stats
            .update_memory_usage(compressed_data.data.len() as i64);
        // Track uncompressed size
        self.stats
            .update_uncompressed_size(uncompressed_size as i64);

        let elapsed_ns = timer.elapsed_ns();
        self.stats.record_hit(elapsed_ns);

        Ok(())
    }

    /// Remove entry from persistent storage
    pub fn remove(&mut self, key: &K) -> bool {
        let cold_key = ColdCacheKey::from_cache_key(key);

        // Access metadata index directly (worker owns the data, no locks needed)
        if let Some(index_entry) = self.metadata_index.remove_entry(&cold_key) {
            // Mark space as free (actual cleanup happens during compaction)
            self.mark_space_free(index_entry.file_offset, index_entry.compressed_size);

            // Track uncompressed size reduction
            self.stats
                .update_uncompressed_size(-(index_entry.uncompressed_size as i64));

            // Update statistics
            self.stats.update_entry_count(-1);
            self.stats
                .update_memory_usage(-(index_entry.compressed_size as i64));

            true
        } else {
            false
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> TierStatistics {
        // Use canonical snapshot method from AtomicTierStats
        self.stats.snapshot()
    }
}
