//! Atomic operations for warm tier using service messages
//!
//! This module provides lock-free atomic operations using the warm tier
//! service channels for true lock-free concurrency, similar to hot tier.

use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

/// Atomically put value only if key is not present using service messages
pub fn put_if_absent_atomic<K: CacheKey + 'static, V: CacheValue + Default + 'static>(
    key: K,
    value: V,
) -> Result<Option<V>, CacheOperationError> {
    use crate::cache::tier::warm::global_api::{WarmCacheRequest, WarmTierCoordinator};
    use crossbeam_channel::bounded;
    use std::time::Duration;

    let coordinator =
        WarmTierCoordinator::get().map_err(|_| CacheOperationError::TierOperationFailed)?;
    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    let (response_tx, response_rx) = bounded(1);
    let request = WarmCacheRequest::PutIfAbsent {
        key,
        value,
        response: response_tx,
    };

    handle.send_request(request)?;

    response_rx
        .recv_timeout(Duration::from_millis(100))
        .map_err(|_| CacheOperationError::TierOperationFailed)
}

/// Atomically replace existing value with new value using service messages
pub fn replace_atomic<K: CacheKey + 'static, V: CacheValue + Default + 'static>(
    key: K,
    value: V,
) -> Result<Option<V>, CacheOperationError> {
    use crate::cache::tier::warm::global_api::{WarmCacheRequest, WarmTierCoordinator};
    use crossbeam_channel::bounded;
    use std::time::Duration;

    let coordinator =
        WarmTierCoordinator::get().map_err(|_| CacheOperationError::TierOperationFailed)?;
    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    let (response_tx, response_rx) = bounded(1);
    let request = WarmCacheRequest::Replace {
        key,
        value,
        response: response_tx,
    };

    handle.send_request(request)?;

    response_rx
        .recv_timeout(Duration::from_millis(100))
        .map_err(|_| CacheOperationError::TierOperationFailed)
}

/// Atomically compare and swap value if current equals expected using service messages
pub fn compare_and_swap_atomic<
    K: CacheKey + 'static,
    V: CacheValue + Default + PartialEq + 'static,
>(
    key: K,
    expected: V,
    new_value: V,
) -> Result<bool, CacheOperationError> {
    use crate::cache::tier::warm::global_api::{WarmCacheRequest, WarmTierCoordinator};
    use crossbeam_channel::bounded;
    use std::time::Duration;

    let coordinator =
        WarmTierCoordinator::get().map_err(|_| CacheOperationError::TierOperationFailed)?;
    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    let (response_tx, response_rx) = bounded(1);
    let request = WarmCacheRequest::CompareAndSwap {
        key,
        expected,
        new_value,
        response: response_tx,
    };

    handle.send_request(request)?;

    response_rx
        .recv_timeout(Duration::from_millis(100))
        .map_err(|_| CacheOperationError::TierOperationFailed)
}
