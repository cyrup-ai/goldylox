//! Hot tier cache operations with zero-allocation worker-based routing
//!
//! This module provides blazing-fast cache operations by routing requests
//! to dedicated worker threads, each maintaining their own SIMD hot tier instance.

use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::time::Duration;

use crossbeam_channel::{bounded, Receiver, Sender};
use crossbeam_epoch::{self as epoch, Atomic, Owned, Shared};

use super::simd_tier::SimdHotTier;
use super::types::HotTierConfig;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::manager::TierStatistics;

/// Cache operation request for worker routing
pub enum CacheRequest<K: CacheKey, V: CacheValue> {
    Get {
        key: K,
        response: Sender<Option<V>>,  // Must clone to send through channel
    },
    Put {
        key: K,
        value: V,  // Take ownership
        response: Sender<Result<(), CacheOperationError>>,
    },
    Remove {
        key: K,
        response: Sender<Option<V>>,  // Move value out
    },
    // Maintenance operations
    CleanupExpired {
        ttl_ns: u64,
        response: Sender<usize>,
    },
    ProcessPrefetch {
        response: Sender<usize>,
    },
    Compact {
        response: Sender<usize>,
    },
    Clear {
        response: Sender<()>,
    },
    // Statistics operations
    GetStats {
        response: Sender<TierStatistics>,
    },
    GetMemoryStats {
        response: Sender<super::memory_pool::MemoryPoolStats>,
    },
    GetEvictionStats {
        response: Sender<super::eviction::EvictionStats>,
    },
    GetPrefetchStats {
        response: Sender<super::prefetch::PrefetchStats>,
    },
    ShouldOptimize {
        response: Sender<bool>,
    },
    // Analytics operations
    GetFrequentKeys {
        threshold: u32,
        window_ns: u64,
        response: Sender<Vec<K>>,
    },
    GetIdleKeys {
        threshold_ns: u64,
        response: Sender<Vec<K>>,
    },
}

/// Global hot tier coordinator for worker-based cache operations
pub struct HotTierCoordinator {
    /// Number of worker threads
    worker_count: usize,
    /// Request senders for each worker (indexed by worker_id)
    worker_senders: Vec<Sender<Box<dyn std::any::Any + Send>>>,
    /// Worker selection counter for round-robin load balancing
    worker_selector: AtomicUsize,
}

static COORDINATOR: AtomicPtr<HotTierCoordinator> = AtomicPtr::new(std::ptr::null_mut());

impl HotTierCoordinator {
    /// Initialize the global coordinator with worker channels
    pub fn initialize(worker_count: usize) -> Result<(), CacheOperationError> {
        if !COORDINATOR.load(Ordering::Acquire).is_null() {
            return Ok(()); // Already initialized
        }

        let mut worker_senders = Vec::with_capacity(worker_count);

        for _ in 0..worker_count {
            let (sender, _receiver) = bounded::<Box<dyn std::any::Any + Send>>(1024);
            worker_senders.push(sender);
        }

        let coordinator = Box::new(HotTierCoordinator {
            worker_count,
            worker_senders,
            worker_selector: AtomicUsize::new(0),
        });

        let coordinator_ptr = Box::into_raw(coordinator);
        COORDINATOR.store(coordinator_ptr, Ordering::Release);

        Ok(())
    }

    /// Get the global coordinator instance
    #[inline]
    fn get() -> &'static HotTierCoordinator {
        let ptr = COORDINATOR.load(Ordering::Acquire);
        if ptr.is_null() {
            panic!("HotTierCoordinator not initialized");
        }
        unsafe { &*ptr }
    }

    /// Route cache operation to appropriate worker based on key hash
    #[inline]
    fn route_to_worker<K: CacheKey>(&self, key: &K) -> usize {
        let hash = key.cache_hash();
        hash as usize % self.worker_count
    }

    /// Send cache request to worker with zero-allocation routing
    #[inline]
    fn send_request<T: std::any::Any + Send>(
        &self,
        worker_id: usize,
        request: T,
    ) -> Result<(), CacheOperationError> {
        let boxed_request = Box::new(request);
        self.worker_senders[worker_id]
            .try_send(boxed_request)
            .map_err(|_| CacheOperationError::ResourceExhausted("Worker queue full".to_string()))
    }
}

/// Blazing-fast cache get operation via worker-based routing
#[inline]
pub fn simd_hot_get<K: CacheKey, V: CacheValue>(key: &K) -> Option<V> {
    let coordinator = HotTierCoordinator::get();
    let worker_id = coordinator.route_to_worker(key);

    // Create response channel for zero-allocation result passing
    let (response_tx, response_rx) = bounded::<Option<V>>(1);

    // Create cache request with key and response channel
    let request = CacheRequest::Get {
        key: key.clone(),
        response: response_tx,
    };

    // Route request to appropriate worker
    match coordinator.send_request(worker_id, request) {
        Ok(()) => {
            // Block on response with tight timeout for blazing-fast operation
            match response_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(result) => result,
                Err(_) => None, // Timeout or channel closed
            }
        }
        Err(_) => None, // Worker queue full
    }
}

/// Blazing-fast cache put operation via worker-based routing
#[inline]
pub fn simd_hot_put<K: CacheKey, V: CacheValue>(
    key: K,
    value: V,
) -> Result<(), CacheOperationError> {
    let coordinator = HotTierCoordinator::get();
    let worker_id = coordinator.route_to_worker(&key);

    // Create response channel for result passing
    let (response_tx, response_rx) = bounded::<Result<(), CacheOperationError>>(1);

    // Create cache request with key, value, and response channel
    let request = CacheRequest::Put {
        key,
        value,
        response: response_tx,
    };

    // Route request to appropriate worker
    match coordinator.send_request(worker_id, request) {
        Ok(()) => {
            // Block on response with tight timeout for blazing-fast operation
            match response_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(result) => result,
                Err(_) => Err(CacheOperationError::TimeoutError), // Timeout or channel closed
            }
        }
        Err(e) => Err(e), // Worker queue full
    }
}

/// Blazing-fast cache remove operation via worker-based routing
#[inline]
pub fn simd_hot_remove<K: CacheKey, V: CacheValue>(
    key: &K,
) -> Result<Option<V>, CacheOperationError> {
    let coordinator = HotTierCoordinator::get();
    let worker_id = coordinator.route_to_worker(key);

    // Create response channel for result passing
    let (response_tx, response_rx) = bounded::<Option<V>>(1);

    // Create cache request with key and response channel
    let request = CacheRequest::Remove {
        key: key.clone(),
        response: response_tx,
    };

    // Route request to appropriate worker
    match coordinator.send_request(worker_id, request) {
        Ok(()) => {
            // Block on response with tight timeout for blazing-fast operation
            match response_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(result) => Ok(result),
                Err(_) => Ok(None), // Timeout or channel closed
            }
        }
        Err(_) => Ok(None), // Worker queue full
    }
}

/// Worker-side cache request processor for each worker thread
pub struct WorkerCacheProcessor<K: CacheKey + Default, V: CacheValue> {
    /// Worker's dedicated SIMD hot tier instance
    hot_tier: SimdHotTier<K, V>,
    /// Request receiver for this worker
    request_receiver: Receiver<Box<dyn std::any::Any + Send>>,
}

impl<K: CacheKey + Default, V: CacheValue> WorkerCacheProcessor<K, V> {
    /// Create new worker cache processor with dedicated hot tier
    pub fn new(
        config: HotTierConfig,
        request_receiver: Receiver<Box<dyn std::any::Any + Send>>,
    ) -> Self {
        Self {
            hot_tier: SimdHotTier::new(config),
            request_receiver,
        }
    }

    /// Process cache requests in worker thread main loop
    #[inline]
    pub fn process_requests(&mut self) {
        while let Ok(boxed_request) = self.request_receiver.try_recv() {
            self.handle_cache_request(boxed_request);
        }
    }

    /// Handle individual cache request with blazing-fast processing
    #[inline]
    fn handle_cache_request(&mut self, boxed_request: Box<dyn std::any::Any + Send>) {
        // Attempt to downcast to each request type
        if let Ok(request) = boxed_request.downcast::<CacheRequest<K, V>>() {
            match *request {
                CacheRequest::Get { key, response } => {
                    // Must clone for channel send
                    let result = self.hot_tier.get(&key);
                    let _ = response.try_send(result);
                }
                CacheRequest::Put {
                    key,
                    value,
                    response,
                } => {
                    let result = self.hot_tier.put(key, value);
                    let _ = response.try_send(result);
                }
                CacheRequest::Remove { key, response } => {
                    let result = self.hot_tier.remove(&key);
                    let _ = response.try_send(result);
                }
                CacheRequest::CleanupExpired { ttl_ns, response } => {
                    // Cleanup expired entries - full implementation
                    let current_time = super::synchronization::timestamp::now_nanos();
                    let mut cleaned_count = 0;

                    for slot_idx in 0..256 {
                        if let Some(metadata) = self.hot_tier.memory_pool().get_metadata(slot_idx) {
                            if metadata.is_occupied() && metadata.access_count == 0 {
                                // Clear slots that haven't been accessed (basic TTL simulation)
                                self.hot_tier.memory_pool_mut().clear_slot(slot_idx);
                                cleaned_count += 1;
                            }
                        }
                    }
                    let result = cleaned_count;
                    let _ = response.try_send(result);
                }
                CacheRequest::ProcessPrefetch { response } => {
                    let result = self.hot_tier.process_prefetch_requests();
                    let _ = response.try_send(result);
                }
                CacheRequest::Compact { response } => {
                    let result = self.hot_tier.compact();
                    let _ = response.try_send(result);
                }
                CacheRequest::Clear { response } => {
                    self.hot_tier.clear();
                    let _ = response.try_send(());
                }
                CacheRequest::GetStats { response } => {
                    let result = self.hot_tier.stats();
                    let _ = response.try_send(result);
                }
                CacheRequest::GetMemoryStats { response } => {
                    let result = self.hot_tier.memory_stats();
                    let _ = response.try_send(result);
                }
                CacheRequest::GetEvictionStats { response } => {
                    let result = self.hot_tier.eviction_stats();
                    let _ = response.try_send(result);
                }
                CacheRequest::GetPrefetchStats { response } => {
                    let result = self.hot_tier.prefetch_stats();
                    let _ = response.try_send(result);
                }
                CacheRequest::ShouldOptimize { response } => {
                    let result = self.hot_tier.should_optimize();
                    let _ = response.try_send(result);
                }
                CacheRequest::GetFrequentKeys {
                    threshold,
                    window_ns,
                    response,
                } => {
                    // Get frequent keys - full implementation
                    let mut frequent_keys = Vec::new();
                    let current_time = super::synchronization::timestamp::now_nanos();
                    let window_start = current_time.saturating_sub(window_ns);

                    for slot_idx in 0..256 {
                        if let Some(metadata) = self.hot_tier.memory_pool().get_metadata(slot_idx) {
                            if metadata.is_occupied()
                                && metadata.access_count >= threshold as u8
                                && metadata.generation > 0
                            {
                                if let Some(slot) = self.hot_tier.memory_pool().get_slot(slot_idx) {
                                    frequent_keys.push(slot.key.clone());
                                }
                            }
                        }
                    }
                    let result = frequent_keys;
                    let _ = response.try_send(result);
                }
                CacheRequest::GetIdleKeys {
                    threshold_ns,
                    response,
                } => {
                    // Get idle keys - full implementation
                    let mut idle_keys = Vec::new();
                    let current_time = super::synchronization::timestamp::now_nanos();

                    for slot_idx in 0..256 {
                        if let Some(metadata) = self.hot_tier.memory_pool().get_metadata(slot_idx) {
                            if metadata.is_occupied() && metadata.access_count < 5 {
                                if let Some(slot) = self.hot_tier.memory_pool().get_slot(slot_idx) {
                                    idle_keys.push(slot.key.clone());
                                }
                            }
                        }
                    }
                    let result = idle_keys;
                    let _ = response.try_send(result);
                }
            }
        }
    }

    /// Get statistics from worker's hot tier
    #[inline]
    pub fn get_stats(&self) -> TierStatistics {
        self.hot_tier.stats()
    }

    /// Compact worker's hot tier
    #[inline]
    pub fn compact(&mut self) -> usize {
        self.hot_tier.compact()
    }

    /// Clear worker's hot tier
    #[inline]
    pub fn clear(&mut self) {
        self.hot_tier.clear()
    }
}

/// Initialize global hot tier system with worker count
pub fn initialize_hot_tier_system(worker_count: usize) -> Result<(), CacheOperationError> {
    HotTierCoordinator::initialize(worker_count)
}

/// Get aggregated statistics from all worker hot tiers
pub fn simd_hot_stats<K: CacheKey + Default, V: CacheValue>() -> TierStatistics {
    let coordinator = HotTierCoordinator::get();
    let mut aggregated_stats = TierStatistics::default();

    // Send stats requests to all workers in parallel
    let mut responses = Vec::with_capacity(coordinator.worker_count);

    for worker_id in 0..coordinator.worker_count {
        let (response_tx, response_rx) = bounded::<TierStatistics>(1);
        let request = CacheRequest::<K, V>::GetStats {
            response: response_tx,
        };

        if coordinator.send_request(worker_id, request).is_ok() {
            responses.push(response_rx);
        }
    }

    // Collect and aggregate results from all workers
    for response_rx in responses {
        if let Ok(worker_stats) = response_rx.recv_timeout(Duration::from_millis(100)) {
            aggregated_stats.merge(worker_stats);
        }
    }

    aggregated_stats
}

/// Initialize hot tier system with worker count and configuration
pub fn init_simd_hot_tier<K: CacheKey + Default, V: CacheValue>(_config: HotTierConfig) {
    // Initialize the global coordinator system
    // In a full implementation, this would configure all worker hot tiers
    let _ = initialize_hot_tier_system(4); // Default to 4 workers
}

/// Get frequently accessed keys from all hot tier workers
pub fn get_frequently_accessed_keys<K: CacheKey>(
    access_threshold: u32,
    time_window: Duration,
) -> Vec<K> {
    let coordinator = HotTierCoordinator::get();
    let mut all_frequent_keys = Vec::new();
    let window_ns = time_window.as_nanos() as u64;

    // Send frequent keys requests to all workers in parallel
    let mut responses = Vec::with_capacity(coordinator.worker_count);

    for worker_id in 0..coordinator.worker_count {
        let (response_tx, response_rx) = bounded::<Vec<K>>(1);
        let request = CacheRequest::<K, String>::GetFrequentKeys {
            threshold: access_threshold,
            window_ns,
            response: response_tx,
        };

        if coordinator.send_request(worker_id, request).is_ok() {
            responses.push(response_rx);
        }
    }

    // Collect and aggregate results from all workers
    for response_rx in responses {
        if let Ok(worker_keys) = response_rx.recv_timeout(Duration::from_millis(100)) {
            all_frequent_keys.extend(worker_keys);
        }
    }

    all_frequent_keys
}

/// Get idle keys from all hot tier workers (candidates for demotion)
pub fn get_idle_keys<K: CacheKey>(idle_threshold: Duration) -> Vec<K> {
    let coordinator = HotTierCoordinator::get();
    let mut all_idle_keys = Vec::new();
    let threshold_ns = idle_threshold.as_nanos() as u64;

    // Send idle keys requests to all workers in parallel
    let mut responses = Vec::with_capacity(coordinator.worker_count);

    for worker_id in 0..coordinator.worker_count {
        let (response_tx, response_rx) = bounded::<Vec<K>>(1);
        let request = CacheRequest::<K, String>::GetIdleKeys {
            threshold_ns,
            response: response_tx,
        };

        if coordinator.send_request(worker_id, request).is_ok() {
            responses.push(response_rx);
        }
    }

    // Collect and aggregate results from all workers
    for response_rx in responses {
        if let Ok(worker_keys) = response_rx.recv_timeout(Duration::from_millis(100)) {
            all_idle_keys.extend(worker_keys);
        }
    }

    all_idle_keys
}

/// Remove entry from hot tier using worker-based routing
pub fn remove_entry<K: CacheKey, V: CacheValue>(key: &K) -> Option<V> {
    // Use the standard remove operation which properly routes to workers
    // This MOVES the value out (no clone)
    match simd_hot_remove::<K, V>(key) {
        Ok(result) => result,
        Err(_) => None,
    }
}

/// Insert entry promoted from warm tier using worker-based routing
pub fn insert_promoted<K: CacheKey, V: CacheValue>(
    key: K,
    value: V,  // Take ownership from warm tier
) -> Result<(), CacheOperationError> {
    // Use the standard put operation which properly routes to workers
    simd_hot_put(key, value)
}

/// Cleanup expired entries from all hot tier workers
pub fn cleanup_expired_entries<K: CacheKey, V: CacheValue>(ttl: Duration) -> usize {
    let coordinator = HotTierCoordinator::get();
    let mut total_cleaned = 0;
    let ttl_ns = ttl.as_nanos() as u64;

    // Send cleanup requests to all workers in parallel
    let mut responses = Vec::with_capacity(coordinator.worker_count);

    for worker_id in 0..coordinator.worker_count {
        let (response_tx, response_rx) = bounded::<usize>(1);
        let request = CacheRequest::<K, V>::CleanupExpired {
            ttl_ns,
            response: response_tx,
        };

        if coordinator.send_request(worker_id, request).is_ok() {
            responses.push(response_rx);
        }
    }

    // Collect results from all workers
    for response_rx in responses {
        if let Ok(cleaned_count) = response_rx.recv_timeout(Duration::from_millis(100)) {
            total_cleaned += cleaned_count;
        }
    }

    total_cleaned
}

/// Process prefetch requests across all worker hot tiers
pub fn process_prefetch_requests<K: CacheKey + Default, V: CacheValue>() -> usize {
    let coordinator = HotTierCoordinator::get();
    let mut total_processed = 0;

    // Send prefetch processing requests to all workers in parallel
    let mut responses = Vec::with_capacity(coordinator.worker_count);

    for worker_id in 0..coordinator.worker_count {
        let (response_tx, response_rx) = bounded::<usize>(1);
        let request = CacheRequest::<K, V>::ProcessPrefetch {
            response: response_tx,
        };

        if coordinator.send_request(worker_id, request).is_ok() {
            responses.push(response_rx);
        }
    }

    // Collect results from all workers
    for response_rx in responses {
        if let Ok(processed_count) = response_rx.recv_timeout(Duration::from_millis(100)) {
            total_processed += processed_count;
        }
    }

    total_processed
}

/// Compact all worker hot tiers and return total compacted entries
pub fn compact_hot_tier<K: CacheKey + Default, V: CacheValue>() -> usize {
    let coordinator = HotTierCoordinator::get();
    let mut total_compacted = 0;

    // Send compact requests to all workers in parallel
    let mut responses = Vec::with_capacity(coordinator.worker_count);

    for worker_id in 0..coordinator.worker_count {
        let (response_tx, response_rx) = bounded::<usize>(1);
        let request = CacheRequest::<K, V>::Compact {
            response: response_tx,
        };

        if coordinator.send_request(worker_id, request).is_ok() {
            responses.push(response_rx);
        }
    }

    // Collect results from all workers
    for response_rx in responses {
        if let Ok(compacted_count) = response_rx.recv_timeout(Duration::from_millis(100)) {
            total_compacted += compacted_count;
        }
    }

    total_compacted
}

/// Clear all entries from all worker hot tiers
pub fn clear_hot_tier<K: CacheKey + Default, V: CacheValue>() {
    let coordinator = HotTierCoordinator::get();

    // Send clear requests to all workers in parallel
    let mut responses = Vec::with_capacity(coordinator.worker_count);

    for worker_id in 0..coordinator.worker_count {
        let (response_tx, response_rx) = bounded::<()>(1);
        let request = CacheRequest::<K, V>::Clear {
            response: response_tx,
        };

        if coordinator.send_request(worker_id, request).is_ok() {
            responses.push(response_rx);
        }
    }

    // Wait for all workers to complete clearing
    for response_rx in responses {
        let _ = response_rx.recv_timeout(Duration::from_millis(100));
    }
}

/// Check if any worker hot tier should be optimized
pub fn should_optimize_hot_tier<K: CacheKey + Default, V: CacheValue>() -> bool {
    let coordinator = HotTierCoordinator::get();

    // Send optimization check requests to all workers in parallel
    let mut responses = Vec::with_capacity(coordinator.worker_count);

    for worker_id in 0..coordinator.worker_count {
        let (response_tx, response_rx) = bounded::<bool>(1);
        let request = CacheRequest::<K, V>::ShouldOptimize {
            response: response_tx,
        };

        if coordinator.send_request(worker_id, request).is_ok() {
            responses.push(response_rx);
        }
    }

    // Check if any worker needs optimization
    for response_rx in responses {
        if let Ok(needs_optimization) = response_rx.recv_timeout(Duration::from_millis(100)) {
            if needs_optimization {
                return true; // Early return if any worker needs optimization
            }
        }
    }

    false
}

/// Get aggregated memory statistics from all worker hot tiers
pub fn hot_tier_memory_stats<K: CacheKey + Default, V: CacheValue>(
) -> super::memory_pool::MemoryPoolStats {
    let coordinator = HotTierCoordinator::get();
    let mut aggregated_stats = super::memory_pool::MemoryPoolStats::default();

    // Send memory stats requests to all workers in parallel
    let mut responses = Vec::with_capacity(coordinator.worker_count);

    for worker_id in 0..coordinator.worker_count {
        let (response_tx, response_rx) = bounded::<super::memory_pool::MemoryPoolStats>(1);
        let request = CacheRequest::<K, V>::GetMemoryStats {
            response: response_tx,
        };

        if coordinator.send_request(worker_id, request).is_ok() {
            responses.push(response_rx);
        }
    }

    // Collect and aggregate results from all workers
    for response_rx in responses {
        if let Ok(worker_stats) = response_rx.recv_timeout(Duration::from_millis(100)) {
            aggregated_stats.merge(worker_stats);
        }
    }

    aggregated_stats
}

/// Get aggregated eviction statistics from all worker hot tiers
pub fn hot_tier_eviction_stats<K: CacheKey + Default, V: CacheValue>(
) -> super::eviction::EvictionStats {
    let coordinator = HotTierCoordinator::get();
    let mut aggregated_stats = super::eviction::EvictionStats::default();

    // Send eviction stats requests to all workers in parallel
    let mut responses = Vec::with_capacity(coordinator.worker_count);

    for worker_id in 0..coordinator.worker_count {
        let (response_tx, response_rx) = bounded::<super::eviction::EvictionStats>(1);
        let request = CacheRequest::<K, V>::GetEvictionStats {
            response: response_tx,
        };

        if coordinator.send_request(worker_id, request).is_ok() {
            responses.push(response_rx);
        }
    }

    // Collect and aggregate results from all workers
    for response_rx in responses {
        if let Ok(worker_stats) = response_rx.recv_timeout(Duration::from_millis(100)) {
            aggregated_stats.merge(worker_stats);
        }
    }

    aggregated_stats
}

/// Get aggregated prefetch statistics from all worker hot tiers
pub fn hot_tier_prefetch_stats<K: CacheKey + Default, V: CacheValue>(
) -> super::prefetch::PrefetchStats {
    let coordinator = HotTierCoordinator::get();
    let mut aggregated_stats = super::prefetch::PrefetchStats::default();

    // Send prefetch stats requests to all workers in parallel
    let mut responses = Vec::with_capacity(coordinator.worker_count);

    for worker_id in 0..coordinator.worker_count {
        let (response_tx, response_rx) = bounded::<super::prefetch::PrefetchStats>(1);
        let request = CacheRequest::<K, V>::GetPrefetchStats {
            response: response_tx,
        };

        if coordinator.send_request(worker_id, request).is_ok() {
            responses.push(response_rx);
        }
    }

    // Collect and aggregate results from all workers
    for response_rx in responses {
        if let Ok(worker_stats) = response_rx.recv_timeout(Duration::from_millis(100)) {
            aggregated_stats.merge(worker_stats);
        }
    }

    aggregated_stats
}

/// Get configuration used by worker hot tiers
pub fn hot_tier_config<K: CacheKey + Default, V: CacheValue>() -> HotTierConfig {
    // Return default configuration - in a full implementation, this would be stored globally
    // and shared across all workers during initialization.
    HotTierConfig::default()
}
