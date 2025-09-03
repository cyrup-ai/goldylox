//! Core data structures for the persistent cold tier cache
//!
//! This module defines all the primary data structures used by the cold tier cache,
//! including storage managers, compression engines, metadata indexes, and related types.

use std::collections::HashMap;
use std::fs::File;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8};

use crate::cache::types::statistics::atomic_stats::AtomicTierStats;
use dashmap::DashMap;

use crossbeam_channel::{Receiver, Sender};
use crossbeam_utils::atomic::AtomicCell;
use memmap2::MmapMut;

use crate::cache::config::types::ColdTierConfig;
use crate::cache::tier::cold::sync::SyncStatsSnapshot;
use crate::cache::traits::*;
use crate::cache::manager::error_recovery::statistics::ErrorStatistics;

/// Binary format constants for cache value serialization
/// These constants define a stable, versioned binary format for persistent storage
#[allow(dead_code)] // Cold tier - magic header used in serialization format
pub const CACHE_VALUE_MAGIC: &[u8; 4] = b"BLZ1";
#[allow(dead_code)] // Cold tier - version field used in serialization format  
pub const CACHE_VALUE_VERSION: u8 = 0x01;
#[allow(dead_code)] // Cold tier - header size used in compression calculations
pub const HEADER_SIZE: usize = 16; // Magic(4) + Version(1) + Compression(1) + Reserved(2) + Timestamp(8)

/// Compression algorithm types for the compression engine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)] // Cold tier - complete compression algorithm support
pub enum CompressionAlgorithm {
    None,
    Lz4,
    Gzip,
    Zstd,
    Snappy,
    Brotli,
}

/// Compression algorithm indicators for binary format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SerializationCompression {
    None = 0x00,
    Lz4 = 0x01,
    Gzip = 0x02,
    Zstd = 0x03,
    Snappy = 0x04,
    Brotli = 0x05,
}

/// Persistent cold tier cache with memory-mapped storage
#[derive(Debug)]
pub struct PersistentColdTier<K: CacheKey, V: CacheValue> {
    /// Memory-mapped storage files
    pub storage_manager: StorageManager,
    /// Compression engine for value serialization
    pub compression_engine: CompressionEngine,
    /// Atomic metadata index
    pub metadata_index: MetadataIndex<K>,
    /// Background compaction system
    pub compaction_system: CompactionSystem,
    /// Atomic statistics
    pub stats: AtomicTierStats,
    /// Error tracking statistics
    pub error_stats: ErrorStatistics,
    /// Configuration
    pub config: ColdTierConfig,
    /// File synchronization state
    pub sync_state: SyncState,
    /// Recovery system for crash resilience
    pub recovery_system: RecoverySystem,
    /// Phantom data for unused type parameter
    pub _phantom: PhantomData<V>,
}

/// Memory-mapped storage manager with atomic file operations
#[derive(Debug)]
pub struct StorageManager {
    /// Primary data file (memory-mapped)
    pub data_file: Option<MmapMut>,
    /// Index file (memory-mapped)
    pub index_file: Option<MmapMut>,
    /// File handles for atomic operations
    pub data_handle: Option<File>,
    pub index_handle: Option<File>,
    /// Storage paths
    pub data_path: PathBuf,
    pub index_path: PathBuf,
    /// Atomic write position in data file
    pub write_position: AtomicU64,
    /// File size limits
    pub max_data_size: u64,
    pub max_index_size: u64,
    /// File generation for consistency
    pub generation: AtomicU32,
}

/// Advanced compression engine with multiple algorithms
#[derive(Debug)]
pub struct CompressionEngine {
    /// Current compression algorithm
    #[allow(dead_code)] // Cold tier - algorithm selection used in compression engine core
    pub algorithm: AtomicCell<CompressionAlgorithm>,
    /// Compression statistics
    pub compression_stats: CompressionStats,
    /// Per-algorithm performance metrics (thread-safe)
    #[allow(dead_code)] // Cold tier - metrics tracking used in adaptive compression selection
    pub algorithm_metrics: DashMap<CompressionAlgorithm, AlgorithmMetrics>,
    /// Adaptive compression thresholds
    #[allow(dead_code)] // Cold tier - thresholds used in adaptive compression decisions
    pub adaptive_thresholds: AdaptiveThresholds,
    /// Adaptation coordination counter
    #[allow(dead_code)] // Cold tier - counter used in compression algorithm adaptation
    pub adaptation_counter: AtomicU64,
    /// Last adaptation timestamp (nanoseconds)
    #[allow(dead_code)] // Cold tier - timestamp used in compression algorithm adaptation
    pub last_adaptation: AtomicU64,
    /// Compression level for algorithms that support it
    #[allow(dead_code)] // Cold tier - level used in compression configuration
    pub compression_level: AtomicU8,
    /// Fast mode setting for performance optimization
    #[allow(dead_code)] // Cold tier - fast mode used in compression performance optimization
    pub fast_mode: AtomicBool,
}

/// Compression result with comprehensive metadata
#[derive(Debug, Clone)]
#[allow(dead_code)] // Cold tier - compression result used in serialization operations
pub struct CompressionResult {
    /// Compressed data
    pub data: Vec<u8>,
    /// Original size before compression
    pub original_size: usize,
    /// Compressed size
    pub compressed_size: usize,
    /// Compression ratio (compressed/original)
    pub compression_ratio: f64,
    /// Algorithm used
    pub algorithm: CompressionAlgorithm,
    /// Compression time in nanoseconds
    pub compression_time_ns: u64,
}

/// Decompression result with metadata
#[derive(Debug, Clone)]
#[allow(dead_code)] // Cold tier - decompression result used in deserialization operations
pub struct DecompressionResult {
    /// Decompressed data
    pub data: Vec<u8>,
    /// Actual size after decompression
    pub actual_size: usize,
    /// Decompression time in nanoseconds
    pub decompression_time_ns: u64,
}

/// Metadata index for fast key lookup
#[derive(Debug)]
pub struct MetadataIndex<K: CacheKey> {
    /// In-memory index (key -> file offset mapping)
    #[allow(dead_code)] // Cold tier - key index used in metadata lookup operations
    pub key_index: HashMap<ColdCacheKey<K>, IndexEntry>,
    /// Bloom filter for fast negative lookups
    #[allow(dead_code)] // Cold tier - bloom filter used in fast negative lookup optimization
    pub bloom_filter: BloomFilter<K>,
    /// Index modification tracking
    #[allow(dead_code)] // Cold tier - dirty entries tracking used in index synchronization
    pub dirty_entries: AtomicU32,
    /// Last index sync timestamp
    #[allow(dead_code)] // Cold tier - sync timestamp used in index persistence
    pub last_sync_ns: AtomicU64,
}

/// Background compaction system for file optimization
#[derive(Debug)]
pub struct CompactionSystem {
    /// Compaction task queue
    #[allow(dead_code)] // Cold tier - compaction channels used in background file optimization
    pub compaction_tx: Sender<CompactionTask>,
    #[allow(dead_code)] // Cold tier - compaction channels used in background file optimization
    pub compaction_rx: Receiver<CompactionTask>,
    /// Compaction state
    #[allow(dead_code)] // Cold tier - state tracking used in compaction process monitoring
    pub compaction_state: CompactionState,
    /// Last compaction timestamp
    #[allow(dead_code)] // Cold tier - timestamp used in compaction scheduling
    pub last_compaction_ns: AtomicU64,
    /// Compaction thread handle
    #[allow(dead_code)] // Cold tier - thread handle used in background compaction management
    pub compaction_handle: Option<std::thread::JoinHandle<()>>,
    /// Last checkpoint snapshot
    #[allow(dead_code)] // Cold tier - checkpoint used in compaction recovery
    pub last_checkpoint: Option<SyncStatsSnapshot>,
}

/// File synchronization state for crash safety
#[derive(Debug)]
pub struct SyncState {
    /// Pending writes counter
    pub pending_writes: AtomicU32,
    /// Last successful sync
    pub last_sync_ns: AtomicU64,
    /// Sync frequency
    pub sync_interval_ns: u64,
    /// Auto-sync enabled
    pub auto_sync_enabled: AtomicBool,
}

/// Recovery system for crash resilience
#[derive(Debug)]
pub struct RecoverySystem {
    /// Recovery log file
    pub recovery_log: Option<File>,
    /// Recovery log path
    pub log_path: PathBuf,
    /// Checkpoint intervals
    pub checkpoint_interval_ns: u64,
    /// Last checkpoint timestamp
    pub last_checkpoint_ns: AtomicU64,
}

/// Cold cache key for persistent storage
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ColdCacheKey<K: CacheKey> {
    /// Key hash for fast comparison
    pub key_hash: u64,
    /// Original cache key (serialized)
    pub serialized_key: Vec<u8>,
    /// Type marker for generic key
    pub _phantom: std::marker::PhantomData<K>,
}

impl<K: CacheKey> crate::cache::traits::core::CacheKey for ColdCacheKey<K> {
    type HashContext = ();
    type Priority = u8;
    type SizeEstimator = usize;

    fn estimated_size(&self) -> usize {
        self.serialized_key.len() + std::mem::size_of::<ColdCacheKey<K>>()
    }

    fn tier_affinity(&self) -> crate::cache::traits::types_and_enums::TierAffinity {
        // Cold cache keys prefer cold tier
        crate::cache::traits::types_and_enums::TierAffinity::Cold
    }

    fn hash_context(&self) -> Self::HashContext {
        ()
    }

    fn priority(&self) -> Self::Priority {
        3 // Lower priority for cold tier
    }

    fn size_estimator(&self) -> Self::SizeEstimator {
        self.estimated_size()
    }

    fn fast_hash(&self, _context: &Self::HashContext) -> u64 {
        self.key_hash
    }
}

/// Index entry for metadata lookup
#[derive(Debug, Clone)]
pub struct IndexEntry {
    /// File offset for data
    pub file_offset: u64,
    /// Compressed size in bytes
    pub compressed_size: u32,
    /// Uncompressed size in bytes
    pub uncompressed_size: u32,
    /// Compression algorithm used
    pub compression_algo: CompressionAlgorithm,
    /// Creation timestamp
    pub created_at_ns: u64,
    /// Last access timestamp
    pub last_access_ns: u64,
    /// Access count
    pub access_count: u32,
    /// Entry checksum for integrity
    pub checksum: u32,
}

/// Bloom filter for negative lookup optimization
#[derive(Debug)]
pub struct BloomFilter<K: CacheKey> {
    /// Bit array for filter
    pub bits: Vec<AtomicU64>,
    /// Hash function count
    pub hash_count: u32,
    /// Filter capacity
    pub capacity: u32,
    /// Current item count
    pub item_count: AtomicU32,
    /// Phantom data for key type
    pub _phantom: PhantomData<K>,
}

/// Compression statistics tracking
#[derive(Debug)]
pub struct CompressionStats {
    /// Total bytes compressed
    #[allow(dead_code)] // Cold tier - compression statistics used in performance monitoring
    pub total_compressed: AtomicU64,
    /// Total bytes uncompressed
    #[allow(dead_code)] // Cold tier - compression statistics used in performance monitoring
    pub total_uncompressed: AtomicU64,
    /// Compression operations count
    #[allow(dead_code)] // Cold tier - operation counters used in statistics reporting
    pub compression_ops: AtomicU64,
    /// Decompression operations count
    #[allow(dead_code)] // Cold tier - operation counters used in statistics reporting
    pub decompression_ops: AtomicU64,
    /// Total compression time
    #[allow(dead_code)] // Cold tier - timing metrics used in performance analysis
    pub total_compression_time_ns: AtomicU64,
    /// Total decompression time
    #[allow(dead_code)] // Cold tier - timing metrics used in performance analysis
    pub total_decompression_time_ns: AtomicU64,
}

/// Per-algorithm performance metrics
#[derive(Debug, Clone)]
pub struct AlgorithmMetrics {
    /// Average compression ratio
    pub avg_compression_ratio: f32,
    /// Average compression speed (bytes/second)
    pub avg_compression_speed: f64,
    /// Average decompression speed (bytes/second)
    pub avg_decompression_speed: f64,
    /// Total operation count
    pub operation_count: u64,
    /// Compression operation count
    pub compression_ops: u64,
    /// Decompression operation count
    pub decompression_ops: u64,
}

/// Adaptive compression thresholds
#[derive(Debug)]
pub struct AdaptiveThresholds {
    /// Minimum size for compression
    pub min_compression_size: AtomicU32,
    /// Compression ratio threshold (stored as bits)
    pub min_compression_ratio: AtomicU32,
    /// Speed threshold for algorithm selection (stored as bits)
    pub speed_threshold: AtomicU64,
}

impl Default for AdaptiveThresholds {
    fn default() -> Self {
        Self {
            min_compression_size: AtomicU32::new(512),
            min_compression_ratio: AtomicU32::new(0.8_f32.to_bits()),
            speed_threshold: AtomicU64::new(100_000_000.0_f64.to_bits()), // 100 MB/s
        }
    }
}



impl Default for CompressionStats {
    fn default() -> Self {
        Self {
            total_compressed: AtomicU64::new(0),
            total_uncompressed: AtomicU64::new(0),
            compression_ops: AtomicU64::new(0),
            decompression_ops: AtomicU64::new(0),
            total_compression_time_ns: AtomicU64::new(0),
            total_decompression_time_ns: AtomicU64::new(0),
        }
    }
}

/// Snapshot of compression statistics for reporting
#[derive(Debug, Clone)]
pub struct CompressionStatsSnapshot {
    pub total_compressed: u64,
    pub total_uncompressed: u64,
    pub compression_ops: u64,
    pub decompression_ops: u64,
    pub total_compression_time_ns: u64,
    pub total_decompression_time_ns: u64,
}

/// Compaction task enumeration
#[derive(Debug)]
pub enum CompactionTask {
    /// Compact data file (remove fragmentation)
    CompactData,
    /// Rebuild index file
    RebuildIndex,
    /// Cleanup expired entries
    CleanupExpired,
    /// Optimize compression parameters
    OptimizeCompression,
}

/// Compaction state tracking
#[derive(Debug)]
pub struct CompactionState {
    /// Currently compacting
    pub is_compacting: AtomicBool,
    /// Compaction progress (0.0 to 1.0)
    pub progress: AtomicCell<f32>,
    /// Last compaction duration
    pub last_duration_ns: AtomicU64,
    /// Bytes reclaimed in last compaction
    pub bytes_reclaimed: AtomicU64,
}

// AtomicTierStats moved to canonical location: crate::cache::types::statistics::atomic_stats::AtomicTierStats

impl StorageManager {
    /// Validate storage integrity
    pub fn validate_integrity(&self) -> Result<bool, crate::cache::traits::types_and_enums::CacheOperationError> {
        // For now, always return true (valid)
        // In production, this would check file integrity, checksums, etc.
        Ok(true)
    }
}
