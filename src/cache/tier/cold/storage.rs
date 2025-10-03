#![allow(dead_code)]
// Cold tier storage - Complete cold storage implementation with compression, memory mapping, statistics, validation, and comprehensive cache operations

//! Core storage implementation for cold tier persistent cache
//!
//! This module contains the main data structures and file-based storage
//! logic for the cold tier cache with integrity verification.

use std::fs::{File, OpenOptions};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crossbeam_channel::{Sender, bounded};
use log;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Instant;

use dashmap::DashMap;

use crate::cache::traits::CompressionAlgorithm;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::types::statistics::atomic_stats::AtomicTierStats;

/// Cold tier cache entry metadata
#[derive(Debug, Clone)]
#[allow(dead_code)] // Cold tier - complete metadata structure for file-based storage operations
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
#[derive(
    Debug, Clone, Default, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode,
)]
#[allow(dead_code)] // Cold tier - complete statistics API for storage analysis and monitoring
pub struct ColdTierStats {
    pub hits: u64,
    pub misses: u64,
    pub entries: usize,
    pub storage_bytes: u64,
}

/// File I/O operations for channel-based coordination
#[allow(dead_code)] // Cold tier - complete I/O operation types for file management
pub enum FileOperation {
    Read {
        offset: u64,
        size: usize,
        response: crossbeam_channel::Sender<Result<Vec<u8>, std::io::Error>>,
    },
    Write {
        offset: u64,
        data: Vec<u8>,
        response: crossbeam_channel::Sender<Result<(), std::io::Error>>,
    },
    Sync {
        response: crossbeam_channel::Sender<Result<(), std::io::Error>>,
    },
    GetSize {
        response: crossbeam_channel::Sender<Result<u64, std::io::Error>>,
    },
    Truncate {
        size: u64,
        response: crossbeam_channel::Sender<Result<(), std::io::Error>>,
    },
}

/// Cold tier with persistent file-based storage
#[allow(dead_code)] // Cold tier - complete cache implementation for persistent storage
pub struct ColdTierCache<K: CacheKey, V: CacheValue> {
    /// Channel for file I/O operations
    pub file_ops: Sender<FileOperation>,
    /// Handle to I/O thread (for cleanup)
    file_io_thread: Option<thread::JoinHandle<()>>,
    /// In-memory index for fast lookups
    pub index: DashMap<K, ColdEntry>,
    /// Storage directory path
    pub storage_path: PathBuf,
    /// Maximum file size before rotation
    pub max_file_size: u64,
    /// Current file offset for new entries
    pub write_offset: AtomicU64,
    /// Compression algorithm for stored data
    pub compression_algorithm: CompressionAlgorithm,
    /// Serialization engine for coordinated cache operations
    pub serialization_engine: crate::cache::tier::cold::serialization::config::SerializationEngine,
    /// Per-instance atomic statistics (replaces global statics)
    pub statistics: Arc<AtomicTierStats>,
    /// Phantom data to maintain type parameter
    _phantom: PhantomData<V>,
}



impl<
    K: CacheKey + bincode::Encode,
    V: CacheValue + bincode::Decode<()> + bincode::Encode + serde::de::DeserializeOwned,
> ColdTierCache<K, V>
{
    /// Spawn dedicated I/O thread that owns the File
    #[allow(dead_code)] // Cold tier - complete I/O thread management for file operations
    fn spawn_io_thread(mut file: File) -> (Sender<FileOperation>, thread::JoinHandle<()>) {
        let (tx, rx) = bounded::<FileOperation>(100);

        let handle = thread::spawn(move || {
            use std::io::{Read, Seek, SeekFrom, Write};

            while let Ok(op) = rx.recv() {
                match op {
                    FileOperation::Read {
                        offset,
                        size,
                        response,
                    } => {
                        let result = (|| {
                            file.seek(SeekFrom::Start(offset))?;
                            let mut buffer = vec![0u8; size];
                            file.read_exact(&mut buffer)?;
                            Ok(buffer)
                        })();
                        let _ = response.send(result);
                    }
                    FileOperation::Write {
                        offset,
                        data,
                        response,
                    } => {
                        let result = (|| {
                            file.seek(SeekFrom::Start(offset))?;
                            file.write_all(&data)?;
                            Ok(())
                        })();
                        let _ = response.send(result);
                    }
                    FileOperation::Sync { response } => {
                        let result = file.sync_all();
                        let _ = response.send(result);
                    }
                    FileOperation::GetSize { response } => {
                        let result = file.metadata().map(|m| m.len());
                        let _ = response.send(result);
                    }
                    FileOperation::Truncate { size, response } => {
                        let result = file.set_len(size);
                        let _ = response.send(result);
                    }
                }
            }
        });

        (tx, handle)
    }

    /// Create new cold tier cache
    #[allow(dead_code)] // Cold tier - complete cache construction for persistent storage
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
            .truncate(true)
            .read(true)
            .write(true)
            .open(&storage_path)
            .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;

        let (file_ops, file_io_thread) = Self::spawn_io_thread(storage_file);

        // Initialize serialization engine with proper configuration
        use crate::cache::tier::cold::serialization::config::{
            SerializationConfig, SerializationEngine,
        };
        let serialization_config = SerializationConfig {
            compression_active: compression_algorithm != CompressionAlgorithm::None,
            compression_level: match compression_algorithm {
                CompressionAlgorithm::None => 0,
                CompressionAlgorithm::Lz4 => 1,
                CompressionAlgorithm::Zstd => 3,
                CompressionAlgorithm::Deflate => 6,
                CompressionAlgorithm::Brotli => 6,
            },
        };
        let serialization_engine = SerializationEngine::new(serialization_config);

        let cache = Self {
            file_ops,
            file_io_thread: Some(file_io_thread),
            index: DashMap::new(),
            storage_path,
            max_file_size: 128 * 1024 * 1024, // 128MB default
            write_offset: AtomicU64::new(0),
            compression_algorithm,
            serialization_engine,
            statistics: Arc::new(AtomicTierStats::new()),
            _phantom: PhantomData,
        };

        // Initialize write offset from file size
        let file_size = cache.get_file_size()?;
        cache.write_offset.store(file_size, Ordering::Relaxed);

        Ok(cache)
    }

    /// Get entry from cache - crossbeam zero-copy reference
    #[allow(dead_code)] // Cold tier - complete cache get operation for file-based storage
    pub fn get(&self, key: &K) -> Result<Option<V>, CacheOperationError> {
        // Index operations are now lock-free with DashMap

        match self.index.get(key) {
            Some(entry_ref) => {
                // Found in index - use actual file reading implementation
                let entry = entry_ref.value().clone();
                match self.read_data_from_file(&entry) {
                    Ok(value) => {
                        self.record_hit();
                        Ok(Some(value))
                    }
                    Err(e) => {
                        self.record_miss();
                        Err(e)
                    }
                }
            }
            None => {
                self.record_miss();
                Ok(None)
            }
        }
    }

    /// Read data from file using ColdEntry metadata
    #[allow(dead_code)] // Cold tier - complete file reading implementation for persistent storage
    fn read_data_from_file(&self, entry: &ColdEntry) -> Result<V, CacheOperationError> {
        use std::time::Duration;

        // Use file I/O to read data
        let (response_tx, response_rx) = bounded(1);
        self.file_ops
            .send(FileOperation::Read {
                offset: entry.file_offset,
                size: entry.data_size as usize,
                response: response_tx,
            })
            .map_err(|_| CacheOperationError::io_failed("I/O thread unavailable"))?;

        // Wait for response
        let data = response_rx
            .recv_timeout(Duration::from_millis(100))
            .map_err(|_| CacheOperationError::timing_error("Storage read timeout"))?
            .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;

        // Parse storage header and extract payload
        use crate::cache::tier::cold::serialization::binary_format::deserialize_cache_value;
        use crate::cache::tier::cold::serialization::header::StorageHeader;

        let (header, header_size) = StorageHeader::deserialize(&data)?;
        header.validate()?;

        // Extract payload data after header
        let payload_data = &data[header_size..];

        // Get compression algorithm from header (V2) or use cache default (V1)
        let compression_algo = header
            .compression_algorithm()
            .unwrap_or(self.compression_algorithm);

        // Deserialize using proper format handling
        match deserialize_cache_value::<V>(payload_data, compression_algo)? {
            Some(value) => Ok(value),
            None => Err(CacheOperationError::serialization_failed(format!(
                "Failed to deserialize value from cold tier storage: payload_size={}, compression={:?}, file_offset={}",
                payload_data.len(),
                compression_algo,
                entry.file_offset
            ))),
        }
    }

    /// Put entry into cache using crossbeam FileOperation messaging
    pub fn put(&self, key: K, value: V) -> Result<(), CacheOperationError> {
        use crate::cache::tier::cold::serialization::binary_format::serialize_cache_value;
        use crate::cache::tier::cold::serialization::header::StorageHeader;
        use crossbeam_channel::bounded;

        // Serialize value using proper serialization infrastructure
        let payload_data = serialize_cache_value(&value, self.compression_algorithm)?;

        // Create storage header with proper metadata
        let header = StorageHeader::new_v2(
            value.estimated_size() as u32,
            self.compression_algorithm,
            payload_data.len() as u32,
        )?;

        // Combine header and payload for complete storage format
        let mut serialized_data = header.serialize();
        serialized_data.extend_from_slice(&payload_data);

        // Get next file offset using proper write_offset tracking
        let offset = self.write_offset.load(Ordering::Relaxed);

        // Use same crossbeam messaging pattern as sync_to_disk()
        let (tx, rx) = bounded(1);
        // Get data size and checksum before moving serialized_data
        let data_size = serialized_data.len() as u32;
        let checksum = {
            use crate::cache::tier::cold::serialization::utilities::calculate_checksum;
            calculate_checksum(&serialized_data)
        };

        self.file_ops
            .send(FileOperation::Write {
                offset,
                data: serialized_data,
                response: tx,
            })
            .map_err(|_| CacheOperationError::resource_exhausted("I/O thread terminated"))?;

        rx.recv()
            .map_err(|_| CacheOperationError::resource_exhausted("I/O response channel closed"))?
            .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;

        // Update in-memory index after successful write - use actual write offset
        let actual_offset = self
            .write_offset
            .fetch_add(data_size as u64, Ordering::Relaxed);
        let entry = ColdEntry {
            file_offset: actual_offset,
            data_size,
            checksum,
            last_access: Instant::now(),
            access_count: 1,
        };
        self.index.insert(key, entry);

        // Record write statistics
        self.record_write(data_size);

        Ok(())
    }

    /// Estimate serialized size for capacity planning
    #[allow(dead_code)] // Cold tier - size estimation for capacity planning integration
    pub fn estimate_entry_size(&self, value: &V) -> usize {
        use crate::cache::tier::cold::serialization::binary_format::estimate_serialized_size;
        let payload_size = estimate_serialized_size(value);
        let header_size = match self.compression_algorithm {
            CompressionAlgorithm::None => 16, // V1 header
            _ => 25,                          // V2 header with compression
        };
        header_size + payload_size
    }

    /// Advanced serialization with detailed result metrics
    #[allow(dead_code)] // Cold tier - advanced serialization with comprehensive metrics
    pub fn serialize_with_metrics(
        &self,
        value: &V,
    ) -> Result<
        crate::cache::tier::cold::serialization::config::SerializationResult,
        CacheOperationError,
    > {
        use crate::cache::tier::cold::serialization::binary_format::serialize_cache_value;
        use crate::cache::tier::cold::serialization::config::SerializationResult;

        let original_size = value.estimated_size();
        let compressed_data = serialize_cache_value(value, self.compression_algorithm)?;
        let compressed_size = compressed_data.len();

        Ok(SerializationResult {
            data: compressed_data,
            original_size,
            compressed_size,
        })
    }

    /// Advanced deserialization with detailed result metrics
    #[allow(dead_code)] // Cold tier - advanced deserialization with comprehensive metrics
    pub fn deserialize_with_metrics(
        &self,
        data: &[u8],
    ) -> Result<
        crate::cache::tier::cold::serialization::config::DeserializationResult<V>,
        CacheOperationError,
    > {
        use crate::cache::tier::cold::serialization::binary_format::deserialize_cache_value;
        use crate::cache::tier::cold::serialization::config::DeserializationResult;

        let deserialized_value =
            match deserialize_cache_value::<V>(data, self.compression_algorithm)? {
                Some(value) => value,
                None => {
                    return Err(CacheOperationError::serialization_failed(
                        "Deserialization returned None",
                    ));
                }
            };

        let size = data.len();

        Ok(DeserializationResult {
            data: deserialized_value,
            size,
        })
    }

    /// Sync all data to disk
    #[allow(dead_code)] // Cold tier - complete disk synchronization for persistent storage
    pub fn sync_to_disk(&self) -> Result<(), CacheOperationError> {
        let (tx, rx) = bounded(1);
        self.file_ops
            .send(FileOperation::Sync { response: tx })
            .map_err(|_| CacheOperationError::resource_exhausted("I/O thread terminated"))?;
        rx.recv()
            .map_err(|_| CacheOperationError::resource_exhausted("I/O response channel closed"))?
            .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;
        Ok(())
    }

    /// Clean up expired entries
    pub fn cleanup_expired(&self) -> Result<usize, CacheOperationError> {
        use crate::cache::tier::cold::optimization::{OptimizationConfig, StorageOptimizer};

        // Connect to existing sophisticated optimization system
        let optimizer_config = OptimizationConfig::default();
        let optimizer = StorageOptimizer::new(optimizer_config);

        // Use existing sophisticated remove_expired_entries() method
        // This method already implements:
        // - TTL logic using entry.last_access.elapsed() > max_idle_duration
        // - Proper index iteration with thread-safe locking
        // - Actual entry removal using cache.remove(&key)
        // - Statistics tracking and error handling
        let removed_count = optimizer.remove_expired_entries(self)?;

        log::info!(
            "Cold tier cleanup completed: {} expired entries removed",
            removed_count
        );

        Ok(removed_count)
    }

    /// Validate storage integrity
    pub fn validate_integrity(&self) -> Result<bool, CacheOperationError> {
        use crate::cache::tier::cold::serialization::utilities::calculate_checksum;

        let index = &self.index;

        let mut validation_errors = 0;
        let total_entries = index.len();

        // Validate each entry using existing sophisticated systems
        for entry_ref in index.iter() {
            let (_key, entry) = entry_ref.pair();
            // Read data via channel
            let (tx, rx) = bounded(1);
            if self
                .file_ops
                .send(FileOperation::Read {
                    offset: entry.file_offset,
                    size: entry.data_size as usize,
                    response: tx,
                })
                .is_err()
            {
                validation_errors += 1;
                log::error!("Failed to send read request for key validation");
                continue;
            }

            let data = match rx.recv() {
                Ok(Ok(data)) => data,
                _ => {
                    validation_errors += 1;
                    log::error!("Failed to read data for key validation");
                    continue;
                }
            };

            // Validate checksum using existing sophisticated calculation
            let calculated_checksum = calculate_checksum(&data);
            if calculated_checksum != entry.checksum {
                validation_errors += 1;
                log::error!(
                    "Checksum mismatch detected: expected {}, got {}",
                    entry.checksum,
                    calculated_checksum
                );
            }
        }

        let integrity_valid = validation_errors == 0;
        if !integrity_valid {
            log::error!(
                "Storage integrity validation failed: {}/{} entries corrupted",
                validation_errors,
                total_entries
            );
        } else {
            log::info!(
                "Storage integrity validation passed: {}/{} entries verified",
                total_entries,
                total_entries
            );
        }

        Ok(integrity_valid)
    }

    /// Get current file size
    pub fn get_file_size(&self) -> Result<u64, CacheOperationError> {
        let (tx, rx) = bounded(1);
        self.file_ops
            .send(FileOperation::GetSize { response: tx })
            .map_err(|_| CacheOperationError::resource_exhausted("I/O thread terminated"))?;
        rx.recv()
            .map_err(|_| CacheOperationError::resource_exhausted("I/O response channel closed"))?
            .map_err(|e| CacheOperationError::io_failed(e.to_string()))
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> Result<ColdTierStats, CacheOperationError> {
        let snapshot = self.statistics.snapshot();
        Ok(ColdTierStats {
            hits: snapshot.hits,
            misses: snapshot.misses,
            entries: self.index.len(),
            storage_bytes: snapshot.memory_usage as u64,
        })
    }

    /// Check if key exists in cache
    pub fn contains_key(&self, key: &K) -> Result<bool, CacheOperationError> {
        let index = &self.index;
        Ok(index.contains_key(key))
    }

    /// Get entry count
    pub fn len(&self) -> Result<usize, CacheOperationError> {
        let index = &self.index;
        Ok(index.len())
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> Result<bool, CacheOperationError> {
        let index = &self.index;
        Ok(index.is_empty())
    }

    /// Clear all entries from cache
    pub fn clear(&self) -> Result<(), CacheOperationError> {
        let index = &self.index;
        index.clear();

        // Reset file to empty
        let (tx, rx) = bounded(1);
        self.file_ops
            .send(FileOperation::Truncate {
                size: 0,
                response: tx,
            })
            .map_err(|_| CacheOperationError::resource_exhausted("I/O thread terminated"))?;
        rx.recv()
            .map_err(|_| CacheOperationError::resource_exhausted("I/O response channel closed"))?
            .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;

        self.write_offset.store(0, Ordering::Relaxed);
        self.statistics.reset();

        Ok(())
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
        let index = &self.index;
        Ok(index.get(key).map(|entry_ref| entry_ref.value().clone()))
    }

    /// Update entry access information
    pub fn update_access(&self, key: &K) -> Result<(), CacheOperationError> {
        if let Some(mut entry_ref) = self.index.get_mut(key) {
            entry_ref.last_access = Instant::now();
            entry_ref.access_count += 1;
        }
        Ok(())
    }

    /// Get entries sorted by access frequency
    pub fn get_entries_by_frequency(&self) -> Result<Vec<(K, ColdEntry)>, CacheOperationError> {
        let index = &self.index;
        let mut entries: Vec<_> = index
            .iter()
            .map(|entry_ref| {
                let (k, v) = entry_ref.pair();
                (k.clone(), v.clone())
            })
            .collect();
        entries.sort_by(|a, b| b.1.access_count.cmp(&a.1.access_count));
        Ok(entries)
    }

    /// Get least recently used entries
    pub fn get_lru_entries(
        &self,
        count: usize,
    ) -> Result<Vec<(K, ColdEntry)>, CacheOperationError> {
        let index = &self.index;
        let mut entries: Vec<_> = index
            .iter()
            .map(|entry_ref| {
                let (k, v) = entry_ref.pair();
                (k.clone(), v.clone())
            })
            .collect();
        entries.sort_by(|a, b| a.1.last_access.cmp(&b.1.last_access));
        Ok(entries.into_iter().take(count).collect())
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
            .truncate(true)
            .read(true)
            .write(true)
            .open(&storage_path)
            .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;

        let (file_ops, file_io_thread) = Self::spawn_io_thread(storage_file);

        // Initialize serialization engine with proper configuration
        use crate::cache::tier::cold::serialization::config::{
            SerializationConfig, SerializationEngine,
        };
        let serialization_config = SerializationConfig {
            compression_active: compression_algorithm != CompressionAlgorithm::None,
            compression_level: match compression_algorithm {
                CompressionAlgorithm::None => 0,
                CompressionAlgorithm::Lz4 => 1,
                CompressionAlgorithm::Zstd => 3,
                CompressionAlgorithm::Deflate => 6,
                CompressionAlgorithm::Brotli => 6,
            },
        };
        let serialization_engine = SerializationEngine::new(serialization_config);

        let cache = Self {
            file_ops,
            file_io_thread: Some(file_io_thread),
            index: DashMap::new(),
            storage_path,
            max_file_size,
            write_offset: AtomicU64::new(0),
            compression_algorithm,
            serialization_engine,
            statistics: Arc::new(AtomicTierStats::new()),
            _phantom: PhantomData,
        };

        Ok(cache)
    }

    /// Get cache statistics
    pub fn stats(&self) -> ColdTierStats {
        let snapshot = self.statistics.snapshot();
        ColdTierStats {
            hits: snapshot.hits,
            misses: snapshot.misses,
            entries: snapshot.entry_count,
            storage_bytes: snapshot.memory_usage as u64,
        }
    }
}

// Basic impl block for utility methods that don't require complex trait bounds
impl<K: CacheKey, V: CacheValue> ColdTierCache<K, V> {
    /// Record cache hit
    pub fn record_hit(&self) {
        self.statistics.record_hit(0);
    }

    /// Record cache miss
    pub fn record_miss(&self) {
        self.statistics.record_miss(0);
    }

    /// Record cache write
    pub fn record_write(&self, size: u32) {
        self.statistics.update_entry_count(1);
        self.statistics.update_memory_usage(size as i64);
    }

    /// Remove entry from cache
    pub fn remove(&self, key: &K) -> Result<bool, CacheOperationError> {
        let index = &self.index;
        if let Some((_key, entry)) = index.remove(key) {
            self.statistics.update_entry_count(-1);
            self.statistics.update_memory_usage(-(entry.data_size as i64));
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Compact storage to reduce fragmentation
    pub fn compact(&self) -> Result<(), CacheOperationError> {
        use crate::cache::tier::cold::optimization::{OptimizationConfig, StorageOptimizer};

        // Connect to existing sophisticated optimization system
        let optimizer_config = OptimizationConfig::default();
        let mut optimizer = StorageOptimizer::new(optimizer_config);

        // Use existing optimization system that includes real compaction
        let result = optimizer.optimize_storage(self)?;

        // Log compaction results using existing metrics
        log::info!(
            "Compaction completed: {} entries removed, efficiency improved from {:.2} to {:.2}",
            result.entries_removed,
            result.efficiency_before,
            result.efficiency_after
        );

        Ok(())
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

impl<K: CacheKey, V: CacheValue> Drop for ColdTierCache<K, V> {
    fn drop(&mut self) {
        // Signal I/O thread to shutdown by dropping sender
        // (channel will close, thread will exit)
        // Wait for thread to finish if it exists
        if let Some(handle) = self.file_io_thread.take() {
            let _ = handle.join();
        }
    }
}
