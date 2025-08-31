//! Initialization and setup for persistent cold tier cache
//!
//! This module handles the construction and initialization of the PersistentColdTier
//! cache, including subsystem setup and background task management.

use std::io;
use std::path::Path;

use crate::cache::config::ColdTierConfig;
use crate::cache::tier::cold::data_structures::*;
use crate::cache::tier::cold::PersistentColdTier;
use crate::cache::traits::core::{CacheKey, CacheValue};
use crate::cache::manager::error_recovery::statistics::ErrorStatistics;
use crate::cache::types::statistics::atomic_stats::AtomicTierStats;

impl<K: CacheKey + Default, V: CacheValue + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static> PersistentColdTier<K, V> {
    /// Create new persistent cold tier cache
    pub fn new(config: ColdTierConfig) -> io::Result<Self> {
        let storage_path = Path::new(config.storage_path.as_str());

        // Create storage directory if it doesn't exist
        if let Some(parent) = storage_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let data_path = storage_path.with_extension("data");
        let index_path = storage_path.with_extension("idx");
        let log_path = storage_path.with_extension("log");

        let storage_manager = StorageManager::new(
            data_path.clone(),
            index_path.clone(),
            config.max_file_size,
        )?;

        let compression_engine = CompressionEngine::new(config.compression_level);
        let metadata_index = MetadataIndex::new()?;
        let mut compaction_system = CompactionSystem::new(config.compact_interval_ns)?;
        
        // Start the background compaction worker thread
        compaction_system.start_background_worker()
            .map_err(|e| std::io::Error::new(
                std::io::ErrorKind::Other, 
                format!("Failed to start compaction worker: {:?}", e)
            ))?;
        
        let sync_state = SyncState::new(10_000_000_000); // 10 seconds
        let recovery_system = RecoverySystem::new(log_path)?;

        let tier = Self {
            storage_manager,
            compression_engine,
            metadata_index,
            compaction_system,
            stats: AtomicTierStats::new(),
            error_stats: ErrorStatistics::new(),
            config,
            sync_state,
            recovery_system,
            _phantom: std::marker::PhantomData,
        };

        // Start background compaction thread
        tier.start_background_compaction();

        Ok(tier)
    }

    /// Start background compaction
    pub(super) fn start_background_compaction(&self) {
        // Background compaction thread is already running from initialization
        // Schedule initial optimization task to begin compaction work
        let _ = self
            .compaction_system
            .schedule_compaction(CompactionTask::OptimizeCompression);
    }
}
