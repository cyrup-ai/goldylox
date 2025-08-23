//! Global API functions for coherence protocol
//!
//! This module provides the public interface for external use of the
//! coherence protocol with simplified function signatures.

use super::super::communication::{CoherenceError, ReadResponse};
use super::super::data_structures::{CacheTier, CoherenceKey, ProtocolConfiguration};
use super::types::CoherenceController;
use crate::cache::traits::{CacheKey, CacheValue};

// Connect to existing sophisticated type-safe deserialization infrastructure
use crate::cache::traits::cache_entry::{CacheEntry, SerializationEnvelope};
use crate::cache::traits::types_and_enums::TierLocation;
use serde::{Serialize, de::DeserializeOwned};

// Global coherence functions for external use
pub fn init_coherence_controller<K: CacheKey, V: CacheValue>() -> CoherenceController<K, V> {
    CoherenceController::new(ProtocolConfiguration::default())
}

/// Helper function to serialize tier value with SerializationEnvelope
fn serialize_tier_value_with_envelope<K: CacheKey, V: CacheValue + Serialize>(
    key: &K,
    value: std::sync::Arc<V>,
    tier: CacheTier,
    controller: &CoherenceController<K, V>,
) -> Result<Vec<u8>, CoherenceError> {
    // Convert CacheTier to TierLocation for CacheEntry::new
    let tier_location = match tier {
        CacheTier::Hot => TierLocation::Hot,
        CacheTier::Warm => TierLocation::Warm,
        CacheTier::Cold => TierLocation::Cold,
    };
    
    // Create sophisticated CacheEntry with all metadata
    let cache_entry = CacheEntry::new(
        key.clone(), 
        value.as_ref().clone(), 
        tier_location
    );
    
    // Use existing SerializationEnvelope with coherence integration
    let envelope = SerializationEnvelope::new_with_coherence(
        cache_entry,
        controller
    ).map_err(|e| match e {
        super::super::communication::CoherenceError::CacheLineNotFound => CoherenceError::CacheLineNotFound,
        super::super::communication::CoherenceError::InvalidStateTransition { from, to } => CoherenceError::InvalidStateTransition { from, to },
        _ => CoherenceError::SerializationFailed,
    })?;
    
    // Serialize SerializationEnvelope directly with bincode (it has serde derives)
    bincode::serialize(&envelope).map_err(|e| {
        CoherenceError::SerializationFailed(format!("Bincode serialization failed: {}", e))
    })
}

pub fn coherent_read<K: CacheKey, V: CacheValue + Serialize>(
    controller: &CoherenceController<K, V>,
    key: &K,
    tier: CacheTier,
) -> Result<Option<Vec<u8>>, CoherenceError> {
    let _coherence_key = CoherenceKey::from_cache_key(key);

    // Handle read request through coherence protocol
    match controller.handle_read_request(key, tier)? {
        ReadResponse::Hit => {
            // Value is present and can be read directly
            match tier {
                CacheTier::Hot => {
                    if let Some(value) = super::super::super::tier::hot::simd_hot_get::<K, V>(key) {
                        let serialized = serialize_tier_value_with_envelope(key, value, tier, controller)?;
                        Ok(Some(serialized))
                    } else {
                        Ok(None)
                    }
                }
                CacheTier::Warm => {
                    if let Some(value) = super::super::super::tier::warm::warm_get::<K, V>(key) {
                        let serialized = serialize_tier_value_with_envelope(key, value, tier, controller)?;
                        Ok(Some(serialized))
                    } else {
                        Ok(None)
                    }
                }
                CacheTier::Cold => match super::super::super::tier::cold::cold_get::<K, V>(key) {
                    Ok(Some(value)) => {
                        let serialized = serialize_tier_value_with_envelope(key, value, tier, controller)?;
                        Ok(Some(serialized))
                    }
                    Ok(None) => Ok(None),
                    Err(e) => Err(CoherenceError::TierAccessFailed(format!(
                        "Cold tier access failed: {:?}", e
                    ))),
                },
            }
        }
        ReadResponse::Miss => Ok(None),
        ReadResponse::SharedGranted => {
            // Shared access granted - read the value
            match tier {
                CacheTier::Hot => {
                    if let Some(value) = super::super::super::tier::hot::simd_hot_get::<K, V>(key) {
                        let serialized = serialize_tier_value_with_envelope(key, value, tier, controller)?;
                        Ok(Some(serialized))
                    } else {
                        Ok(None)
                    }
                }
                CacheTier::Warm => {
                    if let Some(value) = super::super::super::tier::warm::warm_get::<K, V>(key) {
                        let serialized = serialize_tier_value_with_envelope(key, value, tier, controller)?;
                        Ok(Some(serialized))
                    } else {
                        Ok(None)
                    }
                }
                CacheTier::Cold => match super::super::super::tier::cold::cold_get::<K, V>(key) {
                    Ok(Some(value)) => {
                        let serialized = serialize_tier_value_with_envelope(key, value, tier, controller)?;
                        Ok(Some(serialized))
                    }
                    Ok(None) => Ok(None),
                    Err(e) => Err(CoherenceError::TierAccessFailed(format!(
                        "Cold tier access failed: {:?}", e
                    ))),
                },
            }
        }
    }
}

pub fn coherent_write<K: CacheKey, V: CacheValue + DeserializeOwned>(
    controller: &CoherenceController<K, V>,
    key: &K,
    data: Vec<u8>,
    tier: CacheTier,
) -> Result<(), CoherenceError> {
    // Connect to existing SerializationEnvelope<K,V> infrastructure  
    // Use bincode directly to deserialize SerializationEnvelope (it has serde derives)
    let envelope: SerializationEnvelope<K, V> = bincode::deserialize(&data).map_err(|e| {
        CoherenceError::SerializationFailed(format!("Bincode deserialization failed: {}", e))
    })?;

    // Extract value from sophisticated envelope with all metadata preserved
    let cache_entry = envelope.entry;
    let value = std::sync::Arc::new(cache_entry.value);

    // Connect to existing coherence validation
    controller.validate_schema_version(envelope.schema_version)?;
    controller.validate_checksum(key, envelope.checksum)?;

    // Handle write request through coherence protocol
    match controller.handle_write_request(key, tier, value.clone())? {
        super::super::communication::WriteResponse::Success => {
            // Write was approved by coherence protocol - execute the write
            match tier {
                CacheTier::Hot => {
                    super::super::super::tier::hot::simd_hot_put(key.clone(), value)
                        .map_err(|e| CoherenceError::TierAccessFailed(format!(
                            "Hot tier write failed: {:?}", e
                        )))?;
                }
                CacheTier::Warm => {
                    super::super::super::tier::warm::warm_put(key.clone(), value)
                        .map_err(|e| CoherenceError::TierAccessFailed(format!(
                            "Warm tier write failed: {:?}", e
                        )))?;
                }
                CacheTier::Cold => {
                    super::super::super::tier::cold::insert_demoted(key.clone(), value)
                        .map_err(|e| CoherenceError::TierAccessFailed(format!(
                            "Cold tier write failed: {:?}", e
                        )))?;
                }
            }
            Ok(())
        }
        super::super::communication::WriteResponse::Conflict => {
            // Write conflict detected by coherence protocol
            Err(CoherenceError::WriteConflict)
        }
    }
}
