//! Core types and data structures for coherence protocol
//!
//! This module defines the fundamental types used throughout the coherence protocol
//! implementation including controller structure and configuration.

use crate::cache::coherence::communication::CommunicationHub;
use crate::cache::coherence::data_structures::{CacheTier, CoherenceKey, ProtocolConfiguration};
use crate::cache::coherence::invalidation::InvalidationManager;
use crate::cache::coherence::state_management::StateTransitionValidator;
use crate::cache::coherence::statistics::CoherenceStatistics;
use crate::cache::coherence::write_propagation::WritePropagationSystem;
use crate::cache::traits::{CacheKey, CacheValue};

/// Main coherence controller coordinating all coherence operations
#[derive(Debug)]
pub struct CoherenceController<K: CacheKey, V: CacheValue> {
    /// Cache line states indexed by coherence key
    pub cache_line_states:
        crossbeam_skiplist::SkipMap<CoherenceKey<K>, crate::cache::coherence::CacheLineState>,
    /// Communication hub for inter-tier messaging
    pub communication_hub: CommunicationHub<K, V>,
    /// Statistics collection
    pub coherence_stats: CoherenceStatistics,
    /// Protocol configuration
    pub protocol_config: ProtocolConfiguration,
    /// State transition validator
    pub transition_validator: StateTransitionValidator,
    /// Invalidation manager
    pub invalidation_manager: InvalidationManager<K>,
    /// Write propagation system
    pub write_propagation: WritePropagationSystem<K, V>,
}

impl<K: CacheKey, V: CacheValue> CoherenceController<K, V> {
    /// Create new coherence controller
    pub fn new(config: ProtocolConfiguration) -> Self {
        Self {
            cache_line_states: crossbeam_skiplist::SkipMap::new(),
            communication_hub: CommunicationHub::new(),
            coherence_stats: CoherenceStatistics::new(),
            protocol_config: config,
            transition_validator: StateTransitionValidator::new(),
            invalidation_manager: InvalidationManager::new(1000),
            write_propagation: WritePropagationSystem::new(1_000_000_000),
        }
    }

    /// Get coherence statistics
    pub fn get_statistics(&self) -> crate::cache::coherence::CoherenceStatisticsSnapshot {
        self.coherence_stats.get_snapshot()
    }

    /// Determine target tier for write-back based on source tier
    pub fn determine_target_tier(&self, source_tier: CacheTier) -> CacheTier {
        match source_tier {
            CacheTier::Hot => CacheTier::Warm,
            CacheTier::Warm => CacheTier::Cold,
            CacheTier::Cold => CacheTier::Cold, // Cold tier writes to itself (persistence)
        }
    }

    /// Get the "other" tier for invalidation purposes (simplified)
    pub fn get_other_tier(&self, tier: CacheTier) -> CacheTier {
        match tier {
            CacheTier::Hot => CacheTier::Warm,
            CacheTier::Warm => CacheTier::Hot,
            CacheTier::Cold => CacheTier::Warm,
        }
    }

    /// Validate schema version compatibility
    pub fn validate_schema_version(&self, version: u32) -> Result<(), crate::cache::coherence::CoherenceError> {
        let current_schema_version = self.protocol_config.schema_version;
        if version > current_schema_version {
            return Err(crate::cache::coherence::CoherenceError::UnsupportedSchemaVersion(version));
        }
        Ok(())
    }
    
    /// Validate data integrity checksum using coherence invalidation sequence
    pub fn validate_checksum(&self, key: &K, expected: u64) -> Result<(), crate::cache::coherence::CoherenceError> {
        // Use coherence invalidation sequence as checksum (matches SerializationEnvelope implementation)
        let coherence_key = crate::cache::coherence::CoherenceKey::from_cache_key(key);
        
        if let Some(cache_line_entry) = self.cache_line_states.get(&coherence_key) {
            let cache_line = cache_line_entry.value();
            let actual_sequence = cache_line.metadata.invalidation_seq.load(std::sync::atomic::Ordering::Acquire) as u64;
            
            if actual_sequence != expected {
                return Err(crate::cache::coherence::CoherenceError::ChecksumMismatch { 
                    expected, 
                    actual: actual_sequence 
                });
            }
        } else {
            return Err(crate::cache::coherence::CoherenceError::CacheLineNotFound);
        }
        Ok(())
    }
}
