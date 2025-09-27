//! Atomic operations for cold tier with simple synchronization
//!
//! Since cold tier is disk-based, we use simpler synchronization mechanisms
//! as the disk I/O is already the bottleneck.

use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

/// Atomically put value only if key is not present in cold tier
pub fn put_if_absent_atomic<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    key: K,
    value: V,
) -> Result<Option<V>, CacheOperationError> {
    use crate::cache::tier::cold::{ColdTierCoordinator, ColdTierMessage};
    use crossbeam_channel::bounded;
    use std::any::TypeId;
    use std::time::Duration;

    let coordinator = ColdTierCoordinator::get()?;
    let type_key = (TypeId::of::<K>(), TypeId::of::<V>());

    let (response_tx, response_rx) = bounded(1);

    let key_clone = key.clone();
    let value_clone = value.clone();

    let op = Box::new(move |tier: &mut dyn std::any::Any| -> Vec<u8> {
        if let Some(cold_tier) = tier
            .downcast_mut::<crate::cache::tier::cold::data_structures::PersistentColdTier<K, V>>()
        {
            // Check if key exists first
            if let Some(existing) = cold_tier.get(&key_clone) {
                // Key exists, return the existing value
                bincode::encode_to_vec(Some(existing), bincode::config::standard())
                    .unwrap_or_default()
            } else {
                // Key doesn't exist, put new value
                match cold_tier.put(key_clone, value_clone) {
                    Ok(_) => bincode::encode_to_vec(&None::<V>, bincode::config::standard())
                        .unwrap_or_default(),
                    Err(_) => bincode::encode_to_vec(&None::<V>, bincode::config::standard())
                        .unwrap_or_default(),
                }
            }
        } else {
            bincode::encode_to_vec(&None::<V>, bincode::config::standard()).unwrap_or_default()
        }
    });

    let request = ColdTierMessage::ExecuteOp {
        type_key,
        op,
        response: response_tx,
    };

    coordinator
        .sender()
        .send(request)
        .map_err(|_| CacheOperationError::TierOperationFailed)?;

    let result_bytes = response_rx
        .recv_timeout(Duration::from_millis(500))
        .map_err(|_| CacheOperationError::TierOperationFailed)??;

    bincode::decode_from_slice(&result_bytes, bincode::config::standard())
        .map(|(result, _)| result)
        .map_err(|_| {
            CacheOperationError::SerializationError(
                "Failed to decode put_if_absent result".to_string(),
            )
        })
}

/// Atomically replace existing value with new value in cold tier
pub fn replace_atomic<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    key: K,
    value: V,
) -> Result<Option<V>, CacheOperationError> {
    use crate::cache::tier::cold::{ColdTierCoordinator, ColdTierMessage};
    use crossbeam_channel::bounded;
    use std::any::TypeId;
    use std::time::Duration;

    let coordinator = ColdTierCoordinator::get()?;
    let type_key = (TypeId::of::<K>(), TypeId::of::<V>());

    let (response_tx, response_rx) = bounded(1);

    let key_clone = key.clone();
    let value_clone = value.clone();

    let op = Box::new(move |tier: &mut dyn std::any::Any| -> Vec<u8> {
        if let Some(cold_tier) = tier
            .downcast_mut::<crate::cache::tier::cold::data_structures::PersistentColdTier<K, V>>()
        {
            let old_value = cold_tier.get(&key_clone);
            if old_value.is_some() {
                match cold_tier.put(key_clone, value_clone) {
                    Ok(_) => bincode::encode_to_vec(&old_value, bincode::config::standard())
                        .unwrap_or_default(),
                    Err(_) => bincode::encode_to_vec(&None::<V>, bincode::config::standard())
                        .unwrap_or_default(),
                }
            } else {
                bincode::encode_to_vec(&None::<V>, bincode::config::standard()).unwrap_or_default()
            }
        } else {
            bincode::encode_to_vec(&None::<V>, bincode::config::standard()).unwrap_or_default()
        }
    });

    let request = ColdTierMessage::ExecuteOp {
        type_key,
        op,
        response: response_tx,
    };

    coordinator
        .sender()
        .send(request)
        .map_err(|_| CacheOperationError::TierOperationFailed)?;

    let result_bytes = response_rx
        .recv_timeout(Duration::from_millis(500))
        .map_err(|_| CacheOperationError::TierOperationFailed)??;

    bincode::decode_from_slice(&result_bytes, bincode::config::standard())
        .map(|(result, _)| result)
        .map_err(|_| {
            CacheOperationError::SerializationError("Failed to decode replace result".to_string())
        })
}

/// Atomically compare and swap value if current equals expected in cold tier
pub fn compare_and_swap_atomic<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + PartialEq
        + 'static,
>(
    key: K,
    expected: V,
    new_value: V,
) -> Result<bool, CacheOperationError> {
    use crate::cache::tier::cold::{ColdTierCoordinator, ColdTierMessage};
    use crossbeam_channel::bounded;
    use std::any::TypeId;
    use std::time::Duration;

    let coordinator = ColdTierCoordinator::get()?;
    let type_key = (TypeId::of::<K>(), TypeId::of::<V>());

    let (response_tx, response_rx) = bounded(1);

    let key_clone = key.clone();
    let expected_clone = expected.clone();
    let new_value_clone = new_value.clone();

    let op = Box::new(move |tier: &mut dyn std::any::Any| -> Vec<u8> {
        if let Some(cold_tier) = tier
            .downcast_mut::<crate::cache::tier::cold::data_structures::PersistentColdTier<K, V>>()
        {
            if let Some(current) = cold_tier.get(&key_clone) {
                if current == expected_clone {
                    match cold_tier.put(key_clone, new_value_clone) {
                        Ok(_) => bincode::encode_to_vec(true, bincode::config::standard())
                            .unwrap_or_default(),
                        Err(_) => bincode::encode_to_vec(false, bincode::config::standard())
                            .unwrap_or_default(),
                    }
                } else {
                    bincode::encode_to_vec(false, bincode::config::standard()).unwrap_or_default()
                }
            } else {
                bincode::encode_to_vec(false, bincode::config::standard()).unwrap_or_default()
            }
        } else {
            bincode::encode_to_vec(false, bincode::config::standard()).unwrap_or_default()
        }
    });

    let request = ColdTierMessage::ExecuteOp {
        type_key,
        op,
        response: response_tx,
    };

    coordinator
        .sender()
        .send(request)
        .map_err(|_| CacheOperationError::TierOperationFailed)?;

    let result_bytes = response_rx
        .recv_timeout(Duration::from_millis(500))
        .map_err(|_| CacheOperationError::TierOperationFailed)??;

    bincode::decode_from_slice(&result_bytes, bincode::config::standard())
        .map(|(result, _)| result)
        .map_err(|_| {
            CacheOperationError::SerializationError(
                "Failed to decode compare_and_swap result".to_string(),
            )
        })
}
