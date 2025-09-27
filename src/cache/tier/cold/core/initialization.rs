//! Initialization and setup for persistent cold tier cache
//!
//! This module handles the construction and initialization of the PersistentColdTier
//! cache, including subsystem setup and background task management.

use std::io;
use std::path::Path;

use crate::cache::config::types::ColdTierConfig;
use crate::cache::tier::cold::PersistentColdTier;
use crate::cache::tier::cold::data_structures::*;
use crate::cache::traits::core::{CacheKey, CacheValue};
use crate::cache::types::statistics::ErrorStatistics;
use crate::cache::types::statistics::atomic_stats::AtomicTierStats;

impl<
    K: CacheKey + Default,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
> PersistentColdTier<K, V>
{
    /// Create new persistent cold tier cache
    pub fn new(config: ColdTierConfig, cache_id: &str) -> io::Result<Self> {
        let base_dir = Path::new(config.base_dir.as_str());

        // Create full cache directory path: {base_dir}/{cache_id}/
        let cache_dir = base_dir.join(cache_id);

        // Create the full directory structure if it doesn't exist
        if !config.base_dir.is_empty() {
            std::fs::create_dir_all(&cache_dir)?;
        }

        let data_path = cache_dir.join("lox.data");
        let index_path = cache_dir.join("lox.idx");
        let log_path = cache_dir.join("lox.log");

        let storage_manager =
            StorageManager::new(data_path.clone(), index_path.clone(), config.max_file_size)?;

        let compression_engine = CompressionEngine::new(config.compression_level);
        let metadata_index = MetadataIndex::new()?;
        let mut compaction_system = CompactionSystem::new(config.compact_interval_ns)?;

        // Start the background compaction worker thread
        compaction_system.start_background_worker().map_err(|e| {
            std::io::Error::other(format!("Failed to start compaction worker: {:?}", e))
        })?;

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
            maintenance_sender: None, // Will be set by factory
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

    /// Set maintenance task sender for scheduling maintenance operations
    pub fn set_maintenance_sender(
        &mut self,
        sender: crossbeam_channel::Sender<
            crate::cache::manager::background::types::MaintenanceTask,
        >,
    ) {
        self.maintenance_sender = Some(sender);
    }
}
