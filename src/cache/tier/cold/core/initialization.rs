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
    pub fn new(
        config: ColdTierConfig,
        cache_id: &str,
        pool_coordinator: std::sync::Arc<crate::cache::memory::pool_manager::cleanup_manager::PoolCoordinator>,
    ) -> io::Result<Self> {
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

        // Worker owns all data directly - no locks needed
        let storage_manager = StorageManager::new(data_path.clone(), index_path.clone(), config.max_file_size)?;
        let compression_engine = std::sync::Arc::new(CompressionEngine::new(config.compression_level));
        let metadata_index = MetadataIndex::new()?;

        // Create compaction system (no background worker - polling-based via ColdTierService)
        let compaction_system = CompactionSystem::new()?;

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
            pool_coordinator,
            _phantom: std::marker::PhantomData,
        };

        // Compaction tasks will be polled and executed by ColdTierService worker
        // No background thread needed - worker owns the data

        Ok(tier)
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
