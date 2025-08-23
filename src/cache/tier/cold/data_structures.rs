//! Core data structures for the persistent cold tier cache
//!
//! This module defines all the primary data structures used by the cold tier cache,
//! including storage managers, compression engines, metadata indexes, and related types.

use std::collections::HashMap;
use std::fs::File;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
use std::sync::Arc;
use dashmap::DashMap;

use crossbeam_channel::{Receiver, Sender};
use crossbeam_utils::atomic::AtomicCell;
use memmap2::MmapMut;

use crate::cache::config::ColdTierConfig;
use crate::cache::traits::*;

/// Binary format constants for cache value serialization
/// These constants define a stable, versioned binary format for persistent storage
pub const CACHE_VALUE_MAGIC: &[u8; 4] = b"BLZ1";
pub const CACHE_VALUE_VERSION: u8 = 0x01;
pub const HEADER_SIZE: usize = 16; // Magic(4) + Version(1) + Compression(1) + Reserved(2) + Timestamp(8)

/// Compression algorithm types for the compression engine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    pub algorithm: AtomicCell<CompressionAlgorithm>,
    /// Compression statistics
    pub compression_stats: CompressionStats,
    /// Per-algorithm performance metrics (thread-safe)
    pub algorithm_metrics: Arc<DashMap<CompressionAlgorithm, AlgorithmMetrics>>,
    /// Adaptive compression thresholds
    pub adaptive_thresholds: AdaptiveThresholds,
    /// Adaptation coordination counter
    pub adaptation_counter: AtomicU64,
    /// Last adaptation timestamp (nanoseconds)
    pub last_adaptation: AtomicU64,
    /// Compression level for algorithms that support it
    pub compression_level: u8,
}

/// Metadata index for fast key lookup
#[derive(Debug)]
pub struct MetadataIndex<K: CacheKey> {
    /// In-memory index (key -> file offset mapping)
    pub key_index: HashMap<ColdCacheKey<K>, IndexEntry>,
    /// Bloom filter for fast negative lookups
    pub bloom_filter: BloomFilter<K>,
    /// Index modification tracking
    pub dirty_entries: AtomicU32,
    /// Last index sync timestamp
    pub last_sync_ns: AtomicU64,
}

/// Background compaction system for file optimization
#[derive(Debug)]
pub struct CompactionSystem {
    /// Compaction task queue
    pub compaction_tx: Sender<CompactionTask>,
    pub compaction_rx: Receiver<CompactionTask>,
    /// Compaction state
    pub compaction_state: CompactionState,
    /// Last compaction timestamp
    pub last_compaction_ns: AtomicU64,
    /// Compaction thread handle
    pub compaction_handle: Option<std::thread::JoinHandle<()>>,
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    pub total_compressed: AtomicU64,
    /// Total bytes uncompressed
    pub total_uncompressed: AtomicU64,
    /// Compression operations count
    pub compression_ops: AtomicU64,
    /// Decompression operations count
    pub decompression_ops: AtomicU64,
    /// Total compression time
    pub total_compression_time_ns: AtomicU64,
    /// Total decompression time
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
    pub min_compression_size: u32,
    /// Compression ratio threshold
    pub min_compression_ratio: f32,
    /// Speed threshold for algorithm selection
    pub speed_threshold: f64,
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

/// Atomic tier statistics for thread-safe access
#[derive(Debug)]
pub struct AtomicTierStats {
    /// Total entries count
    pub entry_count: AtomicU64,
    /// Total storage size in bytes
    pub storage_size: AtomicU64,
    /// Cache hit count
    pub hit_count: AtomicU64,
    /// Cache miss count
    pub miss_count: AtomicU64,
    /// Last access timestamp
    pub last_access_ns: AtomicU64,
}

impl AtomicTierStats {
    /// Create new atomic tier statistics
    pub fn new() -> Self {
        Self {
            entry_count: AtomicU64::new(0),
            storage_size: AtomicU64::new(0),
            hit_count: AtomicU64::new(0),
            miss_count: AtomicU64::new(0),
            last_access_ns: AtomicU64::new(0),
        }
    }
}
