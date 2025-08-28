//! Global API functions for coherence protocol
//!
//! This module provides the public interface for external use of the
//! coherence protocol with simplified function signatures.

use crate::cache::coherence::communication::{CoherenceError, ReadResponse};
use crate::cache::coherence::data_structures::{CacheTier, CoherenceKey, ProtocolConfiguration};
use crate::cache::coherence::data_structures::CoherenceController;
use crate::cache::traits::{CacheKey, CacheValue};

// Connect to existing sophisticated type-safe deserialization infrastructure
use crate::cache::traits::cache_entry::{CacheEntry, SerializationEnvelope};
use crate::cache::traits::types_and_enums::TierLocation;
use serde::{Serialize, de::DeserializeOwned};
use serde_json;

#[cfg(feature = "bincode")]
#[allow(unused_imports)] // These are used conditionally based on bincode feature
use bincode::{config, decode_from_slice, encode_to_vec};

// Global coherence functions for external use
pub fn init_coherence_controller<K: CacheKey, V: CacheValue>() -> CoherenceController<K, V> {
    CoherenceController::new(ProtocolConfiguration::default())
}

/// Helper function to serialize tier value with SerializationEnvelope
fn serialize_tier_value_with_envelope<K: CacheKey, V: CacheValue + Serialize>(
    key: &K,
    value: V,
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
        value.clone(), 
        tier_location
    );
    
    // Use existing SerializationEnvelope with coherence integration
    let envelope = SerializationEnvelope::new_with_coherence(
        cache_entry,
        controller
    ).map_err(|e| match e {
        crate::cache::coherence::CoherenceError::CacheLineNotFound => CoherenceError::CacheLineNotFound,
        crate::cache::coherence::CoherenceError::InvalidStateTransition { from, to } => CoherenceError::InvalidStateTransition { from, to },
        _ => CoherenceError::SerializationFailed("Coherence error during envelope creation".to_string()),
    })?;
    
    // Serialize SerializationEnvelope with serde_json (bincode not available due to Instant fields)
    serde_json::to_vec(&envelope).map_err(|e| {
        CoherenceError::SerializationFailed(format!("JSON serialization failed: {}", e))
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
                    if let Some(value) = crate::cache::tier::hot::simd_hot_get::<K, V>(key) {
                        let serialized = serialize_tier_value_with_envelope(key, value, tier, controller)?;
                        Ok(Some(serialized))
                    } else {
                        Ok(None)
                    }
                }
                CacheTier::Warm => {
                    if let Some(value) = crate::cache::tier::warm::warm_get::<K, V>(key) {
                        let serialized = serialize_tier_value_with_envelope(key, value, tier, controller)?;
                        Ok(Some(serialized))
                    } else {
                        Ok(None)
                    }
                }
                CacheTier::Cold => match crate::cache::tier::cold::cold_get::<K, V>(key) {
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
                    if let Some(value) = crate::cache::tier::hot::simd_hot_get::<K, V>(key) {
                        let serialized = serialize_tier_value_with_envelope(key, value, tier, controller)?;
                        Ok(Some(serialized))
                    } else {
                        Ok(None)
                    }
                }
                CacheTier::Warm => {
                    if let Some(value) = crate::cache::tier::warm::warm_get::<K, V>(key) {
                        let serialized = serialize_tier_value_with_envelope(key, value, tier, controller)?;
                        Ok(Some(serialized))
                    } else {
                        Ok(None)
                    }
                }
                CacheTier::Cold => match crate::cache::tier::cold::cold_get::<K, V>(key) {
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

pub fn coherent_write<K: CacheKey + bincode::Decode<()>, V: CacheValue + DeserializeOwned + bincode::Decode<()>>(
    controller: &CoherenceController<K, V>,
    key: &K,
    data: Vec<u8>,
    tier: CacheTier,
) -> Result<(), CoherenceError> {
    // Connect to existing SerializationEnvelope<K,V> infrastructure  
    // Use serde_json to deserialize SerializationEnvelope (bincode not available due to Instant fields)
    let envelope: SerializationEnvelope<K, V> = serde_json::from_slice(&data)
        .map_err(|e| {
            CoherenceError::SerializationFailed(format!("JSON deserialization failed: {}", e))
        })?;

    // Extract value from sophisticated envelope with all metadata preserved
    let cache_entry = envelope.entry;
    let value = cache_entry.value;

    // Connect to existing coherence validation
    controller.validate_schema_version(envelope.schema_version)?;
    controller.validate_checksum(key, envelope.checksum)?;

    // Handle write request through coherence protocol
    match controller.handle_write_request(key, tier, value.clone())? {
        crate::cache::coherence::WriteResponse::Success => {
            // Write was approved by coherence protocol - execute the write
            match tier {
                CacheTier::Hot => {
                    crate::cache::tier::hot::simd_hot_put(key.clone(), value)
                        .map_err(|e| CoherenceError::TierAccessFailed(format!(
                            "Hot tier write failed: {:?}", e
                        )))?;
                }
                CacheTier::Warm => {
                    crate::cache::tier::warm::warm_put(key.clone(), value)
                        .map_err(|e| CoherenceError::TierAccessFailed(format!(
                            "Warm tier write failed: {:?}", e
                        )))?;
                }
                CacheTier::Cold => {
                    crate::cache::tier::cold::insert_demoted(key.clone(), value)
                        .map_err(|e| CoherenceError::TierAccessFailed(format!(
                            "Cold tier write failed: {:?}", e
                        )))?;
                }
            }
            Ok(())
        }
        crate::cache::coherence::WriteResponse::Conflict => {
            // Write conflict detected by coherence protocol
            Err(CoherenceError::WriteConflict)
        }
    }
}
