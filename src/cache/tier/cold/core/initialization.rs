//! Initialization and setup for persistent cold tier cache
//!
//! This module handles the construction and initialization of the PersistentColdTier
//! cache, including subsystem setup and background task management.

use std::io;
use std::path::Path;

use super::super::super::super::config::ColdTierConfig;
use super::super::data_structures::*;
use crate::cache::traits::core::{CacheKey, CacheValue};

impl<K: CacheKey, V: CacheValue> super::super::PersistentColdTier<K, V> {
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

        let storage_manager = super::super::data_structures::StorageManager::new(
            data_path.clone(),
            index_path.clone(),
            config.max_file_size,
        )?;

        let compression_engine =
            super::super::data_structures::CompressionEngine::new(config.compression_level);
        let metadata_index = super::super::data_structures::MetadataIndex::new()?;
        let compaction_system =
            super::super::data_structures::CompactionSystem::new(config.compact_interval_ns)?;
        let sync_state = super::super::data_structures::SyncState::new(10_000_000_000); // 10 seconds
        let recovery_system = super::super::data_structures::RecoverySystem::new(log_path)?;

        let tier = Self {
            storage_manager,
            compression_engine,
            metadata_index,
            compaction_system,
            stats: crate::cache::tier::cold::data_structures::AtomicTierStats::new(),
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
        // In a real implementation, this would start the background compaction thread
        // For now, we'll just schedule initial tasks
        let _ = self
            .compaction_system
            .schedule_compaction(CompactionTask::OptimizeCompression);
    }
}
