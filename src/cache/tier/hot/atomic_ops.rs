//! Atomic operations for hot tier using service messages
//!
//! This module provides lock-free atomic operations by routing them through
//! the hot tier service channels, ensuring atomicity via serialization.

use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::tier::hot::thread_local::{CacheRequest, HotTierCoordinator};

/// Atomically put value only if key is not present using service messages
pub async fn put_if_absent_atomic<K: CacheKey + Default + 'static, V: CacheValue + PartialEq + 'static>(
    coordinator: &HotTierCoordinator,
    key: K,
    value: V,
) -> Result<Option<V>, CacheOperationError> {
    use tokio::sync::oneshot;

    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    let (response_tx, response_rx) = oneshot::channel();
    let request = CacheRequest::PutIfAbsent {
        key,
        value,
        response: response_tx,
    };

    handle.send_request(request)?;

    response_rx
        .await
        .map_err(|_| CacheOperationError::TierOperationFailed)
}

/// Atomically replace existing value with new value using service messages
pub async fn replace_atomic<K: CacheKey + Default + 'static, V: CacheValue + PartialEq + 'static>(
    coordinator: &HotTierCoordinator,
    key: K,
    value: V,
) -> Result<Option<V>, CacheOperationError> {
    use tokio::sync::oneshot;

    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    let (response_tx, response_rx) = oneshot::channel();
    let request = CacheRequest::Replace {
        key,
        value,
        response: response_tx,
    };

    handle.send_request(request)?;

    response_rx
        .await
        .map_err(|_| CacheOperationError::TierOperationFailed)
}

/// Atomically compare and swap value if current equals expected using service messages
pub async fn compare_and_swap_atomic<
    K: CacheKey + Default + 'static,
    V: CacheValue + PartialEq + 'static,
>(
    coordinator: &HotTierCoordinator,
    key: K,
    expected: V,
    new_value: V,
) -> Result<bool, CacheOperationError> {
    use tokio::sync::oneshot;

    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    let (response_tx, response_rx) = oneshot::channel();
    let request = CacheRequest::CompareAndSwap {
        key,
        expected,
        new_value,
        response: response_tx,
    };

    handle.send_request(request)?;

    response_rx
        .await
        .map_err(|_| CacheOperationError::TierOperationFailed)
}
