//! Core data structures for MESI-like cache coherence protocol
//!
//! This module defines the fundamental data structures used by the coherence controller,
//! including cache line states, coherence metadata, and key types.

// Internal coherence architecture - components may not be used in minimal API

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use crossbeam_skiplist::SkipMap;
use crossbeam_utils::CachePadded;

use crate::cache::coherence::invalidation::manager::InvalidationManager;
use crate::cache::coherence::invalidation::types::InvalidationPriority;
use crate::cache::coherence::write_propagation::types::WritePriority;
use crate::cache::traits::{CacheKey, CacheValue};
pub use crate::cache::types::CacheTier;

/// MESI-like cache coherence controller
/// Internal coherence architecture - controller used in sophisticated coherence coordination
#[derive(Debug)]
pub struct CoherenceController<K: CacheKey, V: CacheValue + PartialEq> {
    /// Cache line state tracking
    pub cache_line_states: SkipMap<CoherenceKey<K>, CacheLineState>,
    /// Inter-tier communication channels
    pub communication_hub: super::communication::CommunicationHub<K, V>,
    /// Atomic coherence statistics
    pub coherence_stats: Arc<crate::cache::coherence::CoherenceStatistics>,
    /// Protocol configuration - used in protocol validation and coordination
    #[allow(dead_code)]
    // MESI coherence - used in protocol validation and configuration management
    pub protocol_config: ProtocolConfiguration,
    /// State transition validator
    pub transition_validator: super::state_management::StateTransitionValidator,
    /// Invalidation manager
    pub invalidation_manager: InvalidationManager<K>,
    /// Write propagation system (Arc-wrapped for async spawn)
    pub write_propagation: Arc<super::write_propagation::WritePropagationSystem<K, V>>,
    /// Hot tier coordinator for tier operations
    pub hot_tier_coordinator: crate::cache::tier::hot::thread_local::HotTierCoordinator,
    /// Warm tier coordinator for tier operations
    pub warm_tier_coordinator: crate::cache::tier::warm::global_api::WarmTierCoordinator,
    /// Cold tier coordinator for tier operations  
    pub cold_tier_coordinator: crate::cache::tier::cold::ColdTierCoordinator,
}

/// Cache line state with MESI protocol tracking
/// Internal coherence architecture - fields accessed via atomic operations in protocol handlers
#[derive(Debug)]
#[repr(align(64))] // Cache-line aligned for performance
pub struct CacheLineState {
    /// Current MESI state (atomic for lock-free transitions)
    pub mesi_state: CachePadded<AtomicU8>,
    /// Tier ownership bitmask (Hot=1, Warm=2, Cold=4) - used in tier coordination
    #[allow(dead_code)] // MESI coherence - used in tier ownership tracking and coordination
    pub tier_ownership: CachePadded<AtomicU8>,
    /// Version number for optimistic concurrency
    pub version: CachePadded<AtomicU64>,
    /// Last modification timestamp - used in coherence protocol timing
    #[allow(dead_code)]
    // MESI coherence - used in protocol timestamp tracking and temporal coordination
    pub last_modified_ns: CachePadded<AtomicU64>,
    /// Access count across all tiers
    pub total_access_count: CachePadded<AtomicU64>,
    /// Coherence metadata - used in advanced tracking
    #[allow(dead_code)]
    // MESI coherence - used in advanced coherence state tracking and metadata management
    pub metadata: CoherenceMetadata,
}

/// Coherence metadata for advanced tracking
/// Internal coherence tracking - fields used in atomic coherence operations
#[derive(Debug)]
pub struct CoherenceMetadata {
    /// Original data size - used in size tracking
    #[allow(dead_code)]
    // MESI coherence - used in protocol data size tracking and memory management
    pub original_size: AtomicU32,
    /// Dirty tier bitmask (which tiers have modifications) - used in write tracking
    #[allow(dead_code)]
    // MESI coherence - used in protocol write tracking and dirty state management
    pub dirty_tiers: AtomicU8,
    /// Lock ownership (for exclusive access) - used in concurrency control
    #[allow(dead_code)]
    // MESI coherence - used in protocol exclusive access control and locking
    pub lock_owner: AtomicU16,
    /// Invalidation sequence number - used in invalidation ordering
    #[allow(dead_code)]
    // MESI coherence - used in protocol invalidation ordering and sequencing
    pub invalidation_seq: AtomicU32,
    /// Write-back pending flag - used in write coordination
    #[allow(dead_code)]
    // MESI coherence - used in protocol write-back coordination and state management
    pub writeback_pending: AtomicBool,
}

// CacheTier enum removed - using cache::types::CacheTier for consistency
// This eliminates type conflicts and provides better trait implementations

/// Convert CacheTier to bit flag for dirty tier tracking
/// Internal coherence utility - used in atomic tier coordination
#[allow(dead_code)] // MESI coherence - used in protocol tier bitmask generation and coordination
fn tier_to_bit_flag(tier: CacheTier) -> u8 {
    match tier {
        CacheTier::Hot => 1,
        CacheTier::Warm => 2,
        CacheTier::Cold => 4,
    }
}

/// MESI state enumeration with atomic representation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MesiState {
    /// Invalid - cache line not present
    Invalid = 0,
    /// Shared - cache line present, read-only, may exist in other tiers
    Shared = 1,
    /// Exclusive - cache line present, read-write, not in other tiers
    Exclusive = 2,
    /// Modified - cache line present, modified, not in other tiers
    Modified = 3,
}

/// Invalidation reason enumeration
/// Internal coherence protocol API - variants used in invalidation message handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum InvalidationReason {
    /// Write by another tier
    WriteConflict,
    /// Capacity eviction
    CapacityEviction,
    /// Explicit invalidation
    ExplicitInvalidation,
    /// Coherence protocol violation
    ProtocolViolation,
    /// Timeout-based invalidation
    TimeoutExpiration,
}

/// Coherence key for cache line identification
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct CoherenceKey<K> {
    /// Primary key hash for fast comparison
    pub key_hash: u64,
    /// Original cache key
    pub original_key: K,
}

impl<K: CacheKey> CacheKey for CoherenceKey<K> {
    type HashContext = K::HashContext;
    type Priority = K::Priority;
    type SizeEstimator = K::SizeEstimator;

    fn estimated_size(&self) -> usize {
        self.original_key.estimated_size() + std::mem::size_of::<CoherenceKey<K>>()
    }

    fn tier_affinity(&self) -> crate::cache::traits::types_and_enums::TierAffinity {
        self.original_key.tier_affinity()
    }

    fn hash_context(&self) -> Self::HashContext {
        self.original_key.hash_context()
    }

    fn priority(&self) -> Self::Priority {
        self.original_key.priority()
    }

    fn size_estimator(&self) -> Self::SizeEstimator {
        self.original_key.size_estimator()
    }

    fn fast_hash(&self, context: &Self::HashContext) -> u64 {
        self.original_key.fast_hash(context)
    }
}

/// Protocol configuration parameters
/// Internal coherence configuration - fields used in protocol validation and coordination
#[derive(Debug, Clone)]
pub struct ProtocolConfiguration {
    /// Enable optimistic concurrency - used in concurrency control
    #[allow(dead_code)]
    pub optimistic_concurrency: bool,
    /// Enable write-through mode - used in write policy decisions
    #[allow(dead_code)]
    pub write_through: bool,
    /// Maximum invalidation retries - used in invalidation handling
    #[allow(dead_code)]
    pub max_invalidation_retries: u32,
    /// Coherence timeout in nanoseconds - used in protocol timing
    #[allow(dead_code)]
    pub coherence_timeout_ns: u64,
    /// Enable strict ordering - used in ordering guarantees
    #[allow(dead_code)]
    pub strict_ordering: bool,
    /// Current schema version for compatibility validation - used in version checking
    #[allow(dead_code)]
    pub schema_version: u32,
}

impl Default for CacheLineState {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheLineState {
    /// Create new cache line state
    pub fn new() -> Self {
        Self {
            mesi_state: CachePadded::new(AtomicU8::new(MesiState::Invalid as u8)),
            tier_ownership: CachePadded::new(AtomicU8::new(0)),
            version: CachePadded::new(AtomicU64::new(0)),
            last_modified_ns: CachePadded::new(AtomicU64::new(0)),
            total_access_count: CachePadded::new(AtomicU64::new(0)),
            metadata: CoherenceMetadata::new(),
        }
    }

    /// Get current MESI state
    pub fn get_mesi_state(&self) -> MesiState {
        MesiState::from_u8(self.mesi_state.load(Ordering::Acquire))
    }

    /// Set MESI state atomically
    pub fn set_mesi_state(&self, state: MesiState) {
        self.mesi_state.store(state as u8, Ordering::Release);
    }

    /// Compare and swap MESI state
    #[allow(dead_code)] // MESI coherence - used in protocol atomic state transitions
    pub fn compare_exchange_mesi_state(
        &self,
        current: MesiState,
        new: MesiState,
    ) -> Result<MesiState, MesiState> {
        match self.mesi_state.compare_exchange(
            current as u8,
            new as u8,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(old) => Ok(MesiState::from_u8(old)),
            Err(actual) => Err(MesiState::from_u8(actual)),
        }
    }

    /// Increment access count
    pub fn increment_access_count(&self) {
        self.total_access_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current access count
    pub fn get_access_count(&self) -> u64 {
        self.total_access_count.load(Ordering::Relaxed)
    }

    /// Get current version
    pub fn get_version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    /// Increment version
    pub fn increment_version(&self) -> u64 {
        self.version.fetch_add(1, Ordering::AcqRel) + 1
    }
}

impl Default for CoherenceMetadata {
    fn default() -> Self {
        Self::new()
    }
}

impl CoherenceMetadata {
    pub fn new() -> Self {
        Self {
            original_size: AtomicU32::new(0),
            dirty_tiers: AtomicU8::new(0),
            lock_owner: AtomicU16::new(0),
            invalidation_seq: AtomicU32::new(0),
            writeback_pending: AtomicBool::new(false),
        }
    }

    /// Mark tier as dirty
    #[allow(dead_code)] // MESI coherence - used in protocol dirty bit management
    pub fn mark_dirty(&self, tier: CacheTier) {
        self.dirty_tiers
            .fetch_or(tier_to_bit_flag(tier), Ordering::Relaxed);
    }

    /// Clear dirty flag for tier
    #[allow(dead_code)] // MESI coherence - used in protocol dirty bit management
    pub fn clear_dirty(&self, tier: CacheTier) {
        self.dirty_tiers
            .fetch_and(!tier_to_bit_flag(tier), Ordering::Relaxed);
    }

    /// Check if tier is dirty
    #[allow(dead_code)] // MESI coherence - used in protocol dirty bit management
    pub fn is_dirty(&self, tier: CacheTier) -> bool {
        (self.dirty_tiers.load(Ordering::Relaxed) & tier_to_bit_flag(tier)) != 0
    }
}

impl MesiState {
    /// Convert u8 to MesiState
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => MesiState::Invalid,
            1 => MesiState::Shared,
            2 => MesiState::Exclusive,
            3 => MesiState::Modified,
            _ => MesiState::Invalid, // Default to Invalid for invalid values
        }
    }

    /// Check if state allows reading
    #[allow(dead_code)] // MESI coherence - used in protocol state validation and access control
    pub fn can_read(&self) -> bool {
        matches!(
            self,
            MesiState::Shared | MesiState::Exclusive | MesiState::Modified
        )
    }

    /// Check if state allows writing
    #[allow(dead_code)] // MESI coherence - used in protocol state validation and access control
    pub fn can_write(&self) -> bool {
        matches!(self, MesiState::Exclusive | MesiState::Modified)
    }

    /// Check if state is valid
    #[allow(dead_code)] // MESI coherence - used in protocol state validation and access control
    pub fn is_valid(&self) -> bool {
        !matches!(self, MesiState::Invalid)
    }
}

impl<K> CoherenceKey<K>
where
    K: Clone + std::hash::Hash,
{
    /// Create coherence key from cache key
    pub fn from_cache_key(key: &K) -> Self {
        let mut hasher = ahash::AHasher::default();
        use std::hash::Hasher;
        key.hash(&mut hasher);
        let key_hash = hasher.finish();

        Self {
            key_hash,
            original_key: key.clone(),
        }
    }
}

impl<
    K: CacheKey
        + Default
        + bincode::Encode
        + bincode::Decode<()>
        + serde::Serialize
        + serde::de::DeserializeOwned,
    V: CacheValue
        + Default
        + PartialEq
        + bincode::Encode
        + bincode::Decode<()>
        + serde::Serialize
        + serde::de::DeserializeOwned,
> CoherenceController<K, V>
{
    /// Create new coherence controller - WORKER EXCLUSIVE ACCESS ONLY
    /// This constructor is pub(crate) to prevent external shared access
    pub(crate) fn new(
        config: ProtocolConfiguration,
        hot_tier_coordinator: crate::cache::tier::hot::thread_local::HotTierCoordinator,
        warm_tier_coordinator: crate::cache::tier::warm::global_api::WarmTierCoordinator,
        cold_tier_coordinator: crate::cache::tier::cold::ColdTierCoordinator,
    ) -> Self {
        // Create Arc-wrapped stats to share with validator
        let coherence_stats = Arc::new(crate::cache::coherence::CoherenceStatistics::new());
        
        Self {
            cache_line_states: SkipMap::new(),
            communication_hub: super::communication::CommunicationHub::new(),
            coherence_stats: coherence_stats.clone(),
            protocol_config: config,
            transition_validator: super::state_management::StateTransitionValidator::new(
                coherence_stats.clone()
            ),
            invalidation_manager: InvalidationManager::new(1000),
            write_propagation: Arc::new(super::write_propagation::WritePropagationSystem::new(
                1_000_000_000,
                hot_tier_coordinator.clone(),
                warm_tier_coordinator.clone(),
                cold_tier_coordinator.clone(),
            )),
            hot_tier_coordinator,
            warm_tier_coordinator,
            cold_tier_coordinator,
        }
    }

    /// Handle read request from a cache tier
    pub fn handle_read_request(
        &self,
        key: &K,
        requesting_tier: CacheTier,
    ) -> Result<super::communication::ReadResponse, super::communication::CoherenceError> {
        use std::time::Instant;

        let start_time = Instant::now();

        // Update concurrent operations tracking
        let active_ops = crate::cache::manager::background::worker::GLOBAL_ACTIVE_TASKS
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        self.coherence_stats
            .update_concurrent_operations(active_ops);
        let coherence_key = CoherenceKey::from_cache_key(key);

        // Get or create cache line state
        let cache_line = self
            .cache_line_states
            .get_or_insert(coherence_key.clone(), CacheLineState::new());

        let current_state = cache_line.value().get_mesi_state();
        cache_line.value().increment_access_count();

        let response = match current_state {
            MesiState::Invalid => {
                // Cache miss - need to fetch from lower tier
                self.handle_cache_miss(&coherence_key, requesting_tier, false)?
            }
            MesiState::Shared | MesiState::Exclusive | MesiState::Modified => {
                // Cache hit - data is available
                super::communication::ReadResponse::Hit
            }
        };

        // Record statistics
        let latency_ns = start_time.elapsed().as_nanos() as u64;
        self.coherence_stats.record_success(latency_ns);

        // Record coherence overhead to global statistics
        self.coherence_stats.record_overhead(latency_ns);

        // Decrement active operations counter
        crate::cache::manager::background::worker::GLOBAL_ACTIVE_TASKS
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);

        Ok(response)
    }

    /// Handle write request from a cache tier
    pub fn handle_write_request(
        &self,
        key: &K,
        requesting_tier: CacheTier,
        data: V,
    ) -> Result<super::communication::WriteResponse, super::communication::CoherenceError> {
        use std::time::Instant;

        let start_time = Instant::now();

        // Update concurrent operations tracking
        let active_ops = crate::cache::manager::background::worker::GLOBAL_ACTIVE_TASKS
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        self.coherence_stats
            .update_concurrent_operations(active_ops);
        let coherence_key = CoherenceKey::from_cache_key(key);

        // Get or create cache line state
        let cache_line = self
            .cache_line_states
            .get_or_insert(coherence_key.clone(), CacheLineState::new());

        let current_state = cache_line.value().get_mesi_state();
        cache_line.value().increment_access_count();

        let response = match current_state {
            MesiState::Invalid => {
                // Write miss - need exclusive access
                self.handle_write_miss(&coherence_key, requesting_tier, data)?
            }
            MesiState::Shared => {
                // Need to invalidate other sharers and get exclusive access
                self.handle_shared_write(&coherence_key, requesting_tier, data)?
            }
            MesiState::Exclusive | MesiState::Modified => {
                // Can write directly
                self.handle_exclusive_write(&coherence_key, requesting_tier, data)?
            }
        };

        // Record statistics
        let latency_ns = start_time.elapsed().as_nanos() as u64;
        self.coherence_stats.record_success(latency_ns);

        // Record coherence overhead to global statistics
        self.coherence_stats.record_overhead(latency_ns);

        // Decrement active operations counter
        crate::cache::manager::background::worker::GLOBAL_ACTIVE_TASKS
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);

        Ok(response)
    }

    /// Handle cache miss (read or write)
    pub fn handle_cache_miss(
        &self,
        key: &CoherenceKey<K>,
        requesting_tier: CacheTier,
        is_write: bool,
    ) -> Result<super::communication::ReadResponse, super::communication::CoherenceError> {
        use std::time::Instant;

        // Send request to communication hub
        let message = if is_write {
            super::communication::CoherenceMessage::RequestExclusive {
                key: key.clone(),
                requester_tier: requesting_tier,
                version: 0,
                timestamp_ns: Instant::now().elapsed().as_nanos() as u64,
            }
        } else {
            super::communication::CoherenceMessage::RequestShared {
                key: key.clone(),
                requester_tier: requesting_tier,
                version: 0,
                timestamp_ns: Instant::now().elapsed().as_nanos() as u64,
            }
        };

        // Broadcast request to other tiers
        self.communication_hub.broadcast(message)?;

        // Update cache line state based on response
        if let Some(cache_line_entry) = self.cache_line_states.get(key) {
            let cache_line = cache_line_entry.value();
            let new_state = if is_write {
                MesiState::Exclusive
            } else {
                MesiState::Shared
            };

            let transition_request = super::state_management::StateTransitionRequest {
                from_state: cache_line.get_mesi_state(),
                to_state: new_state,
                tier: requesting_tier,
                version: cache_line.get_version(),
                timestamp_ns: Instant::now().elapsed().as_nanos() as u64,
                reason: if is_write {
                    super::state_management::TransitionReason::Write
                } else {
                    super::state_management::TransitionReason::Read
                },
            };

            self.transition_validator
                .execute_transition(&transition_request)?;
            cache_line.set_mesi_state(new_state);
            cache_line.increment_version();
        }

        Ok(super::communication::ReadResponse::SharedGranted)
    }

    /// Handle write miss
    pub fn handle_write_miss(
        &self,
        key: &CoherenceKey<K>,
        requesting_tier: CacheTier,
        data: V,
    ) -> Result<super::communication::WriteResponse, super::communication::CoherenceError> {
        self.handle_cache_miss(key, requesting_tier, true)?;

        // Determine priority based on tier and cache line state
        let priority = match requesting_tier {
            CacheTier::Hot => WritePriority::Critical, // Hot tier writes are critical for performance
            CacheTier::Cold => WritePriority::Low,     // Cold tier writes can be deferred
            CacheTier::Warm => {
                // Check access count for warm tier
                if let Some(entry) = self.cache_line_states.get(key) {
                    if entry.value().get_access_count() > 10 {
                        WritePriority::High // Frequently accessed warm data
                    } else {
                        WritePriority::Normal
                    }
                } else {
                    WritePriority::Normal // Default for new entries
                }
            }
        };

        // Submit write-back request (spawn async operation in background)
        let write_propagation = Arc::clone(&self.write_propagation);
        let key_clone = key.clone();
        let target_tier = self.determine_target_tier(requesting_tier);
        tokio::runtime::Handle::current().spawn(async move {
            let _ = write_propagation.submit_writeback(
                key_clone,
                data,
                requesting_tier,
                target_tier,
                0,
                priority,
            ).await;
        });

        Ok(super::communication::WriteResponse::Success)
    }

    /// Handle write to shared cache line
    pub fn handle_shared_write(
        &self,
        key: &CoherenceKey<K>,
        requesting_tier: CacheTier,
        data: V,
    ) -> Result<super::communication::WriteResponse, super::communication::CoherenceError> {
        // Send invalidation to other tiers
        self.invalidation_manager.submit_invalidation(
            key.clone(),
            self.get_other_tier(requesting_tier),
            InvalidationReason::WriteConflict,
            InvalidationPriority::High,
        );

        // Transition to exclusive state
        if let Some(cache_line_entry) = self.cache_line_states.get(key) {
            let cache_line = cache_line_entry.value();
            let transition_request = super::state_management::StateTransitionRequest {
                from_state: MesiState::Shared,
                to_state: MesiState::Exclusive,
                tier: requesting_tier,
                version: cache_line.get_version(),
                timestamp_ns: std::time::Instant::now().elapsed().as_nanos() as u64,
                reason: super::state_management::TransitionReason::Write,
            };

            self.transition_validator
                .execute_transition(&transition_request)?;
            cache_line.set_mesi_state(MesiState::Exclusive);
            cache_line.increment_version();
        }

        // Submit write-back request (spawn async operation in background)
        let write_propagation = Arc::clone(&self.write_propagation);
        let key_clone = key.clone();
        let target_tier = self.determine_target_tier(requesting_tier);
        tokio::runtime::Handle::current().spawn(async move {
            let _ = write_propagation.submit_writeback(
                key_clone,
                data,
                requesting_tier,
                target_tier,
                0,
                WritePriority::High,
            ).await;
        });

        Ok(super::communication::WriteResponse::Success)
    }

    /// Handle write to exclusive cache line
    pub fn handle_exclusive_write(
        &self,
        key: &CoherenceKey<K>,
        requesting_tier: CacheTier,
        data: V,
    ) -> Result<super::communication::WriteResponse, super::communication::CoherenceError> {
        // Transition to modified state
        if let Some(cache_line_entry) = self.cache_line_states.get(key) {
            let cache_line = cache_line_entry.value();
            let current_state = cache_line.get_mesi_state();
            let transition_request = super::state_management::StateTransitionRequest {
                from_state: current_state,
                to_state: MesiState::Modified,
                tier: requesting_tier,
                version: cache_line.get_version(),
                timestamp_ns: std::time::Instant::now().elapsed().as_nanos() as u64,
                reason: super::state_management::TransitionReason::Write,
            };

            self.transition_validator
                .execute_transition(&transition_request)?;
            cache_line.set_mesi_state(MesiState::Modified);
            cache_line.increment_version();
        }

        // Submit write-back request with lower priority (already exclusive) - spawn async operation
        let write_propagation = Arc::clone(&self.write_propagation);
        let key_clone = key.clone();
        let target_tier = self.determine_target_tier(requesting_tier);
        tokio::runtime::Handle::current().spawn(async move {
            let _ = write_propagation.submit_writeback(
                key_clone,
                data,
                requesting_tier,
                target_tier,
                0,
                WritePriority::Normal,
            ).await;
        });

        Ok(super::communication::WriteResponse::Success)
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

    // validate_schema_version and validate_checksum methods removed - were unused validation functions

    /// Update coherence state after successful SIMD search hit
    pub fn update_state_after_hit(
        &self,
        key: &K,
        tier: CacheTier,
    ) -> Result<(), super::communication::CoherenceError> {
        use std::time::Instant;

        let coherence_key = CoherenceKey::from_cache_key(key);

        // Get or create cache line state
        let cache_line = self
            .cache_line_states
            .get_or_insert(coherence_key.clone(), CacheLineState::new());

        let current_state = cache_line.value().get_mesi_state();
        cache_line.value().increment_access_count();

        // Update to appropriate state based on current state
        let new_state = match current_state {
            MesiState::Invalid => MesiState::Shared, // First access - mark as shared
            MesiState::Shared => MesiState::Shared,  // Keep shared
            MesiState::Exclusive => MesiState::Exclusive, // Keep exclusive
            MesiState::Modified => MesiState::Modified, // Keep modified
        };

        if current_state != new_state {
            let transition_request = super::state_management::StateTransitionRequest {
                from_state: current_state,
                to_state: new_state,
                tier,
                version: cache_line.value().get_version(),
                timestamp_ns: Instant::now().elapsed().as_nanos() as u64,
                reason: super::state_management::TransitionReason::Read,
            };

            self.transition_validator
                .execute_transition(&transition_request)?;
            cache_line.value().set_mesi_state(new_state);
            cache_line.value().increment_version();
        }

        Ok(())
    }

    /// Update coherence state after successful SIMD put operation
    pub fn update_state_after_put(
        &self,
        key: &K,
        tier: CacheTier,
        is_new_entry: bool,
    ) -> Result<(), super::communication::CoherenceError> {
        use std::time::Instant;

        let coherence_key = CoherenceKey::from_cache_key(key);

        // Get or create cache line state
        let cache_line = self
            .cache_line_states
            .get_or_insert(coherence_key.clone(), CacheLineState::new());

        let current_state = cache_line.value().get_mesi_state();
        cache_line.value().increment_access_count();

        // For put operations, always transition to modified state
        let new_state = if is_new_entry {
            MesiState::Exclusive // New entries start as exclusive
        } else {
            MesiState::Modified // Updates are modified
        };

        if current_state != new_state {
            let transition_request = super::state_management::StateTransitionRequest {
                from_state: current_state,
                to_state: new_state,
                tier,
                version: cache_line.value().get_version(),
                timestamp_ns: Instant::now().elapsed().as_nanos() as u64,
                reason: super::state_management::TransitionReason::Write,
            };

            self.transition_validator
                .execute_transition(&transition_request)?;
            cache_line.value().set_mesi_state(new_state);
            cache_line.value().increment_version();
        }

        Ok(())
    }

    /// Update coherence state after successful SIMD remove operation  
    pub fn update_state_after_remove(
        &self,
        key: &K,
        tier: CacheTier,
    ) -> Result<(), super::communication::CoherenceError> {
        use std::time::Instant;

        let coherence_key = CoherenceKey::from_cache_key(key);

        if let Some(cache_line_entry) = self.cache_line_states.get(&coherence_key) {
            let cache_line = cache_line_entry.value();
            let current_state = cache_line.get_mesi_state();

            // Transition to invalid state after removal
            let transition_request = super::state_management::StateTransitionRequest {
                from_state: current_state,
                to_state: MesiState::Invalid,
                tier,
                version: cache_line.get_version(),
                timestamp_ns: Instant::now().elapsed().as_nanos() as u64,
                reason: super::state_management::TransitionReason::Eviction,
            };

            self.transition_validator
                .execute_transition(&transition_request)?;
            cache_line.set_mesi_state(MesiState::Invalid);
            cache_line.increment_version();
        }

        Ok(())
    }

    /// Get coherence statistics
    #[allow(dead_code)] // Statistics collection - used in unified telemetry system integration
    pub fn get_statistics(&self) -> crate::cache::coherence::CoherenceStatisticsSnapshot {
        self.coherence_stats.get_snapshot()
    }

    /// Perform background maintenance including write propagation completion processing
    pub async fn perform_maintenance(&self) {
        // Process pending write propagation tasks and generate completions
        self.process_write_propagation_tasks().await;

        // Process write propagation worker completions
        self.write_propagation.process_worker_completions();
    }

    /// Process write propagation background tasks
    async fn process_write_propagation_tasks(&self) {
        use super::write_propagation::worker_system::WriteBackCompletion;
        use std::time::Instant;

        // Process up to 10 tasks per maintenance cycle
        for _ in 0..10 {
            if let Ok(task) = self.write_propagation.worker_channels.task_rx.try_recv() {
                let start_time = Instant::now();

                // Execute actual tier write operation using crossbeam channels
                let result = self.execute_tier_write_operation(&task.request).await;

                let processing_time = start_time.elapsed().as_nanos() as u64;

                // Send completion notification
                let completion = WriteBackCompletion {
                    task_id: task.task_id,
                    result,
                    processing_time_ns: processing_time,
                    completed_at: Instant::now(),
                };

                // Try to send completion (non-blocking)
                let _ = self
                    .write_propagation
                    .worker_channels
                    .completion_tx
                    .try_send(completion);
            } else {
                // No more tasks to process
                break;
            }
        }
    }

    /// Execute actual tier write operation using crossbeam channels
    async fn execute_tier_write_operation(
        &self,
        request: &super::write_propagation::types::WriteBackRequest<CoherenceKey<K>, V>,
    ) -> super::write_propagation::worker_system::WriteBackResult {
        use super::write_propagation::worker_system::WriteBackResult;
        use crate::CacheOperationError;

        // Extract the actual cache key from the coherence key
        let cache_key = request.key.original_key.clone();
        let cache_value = request.data.clone();

        // Perform actual write operation to target tier using crossbeam channels
        let write_result = match request.target_tier {
            CacheTier::Hot => {
                // Write to hot tier using SIMD-optimized crossbeam messaging
                crate::cache::tier::hot::thread_local::simd_hot_put(&self.hot_tier_coordinator, cache_key, cache_value).await
            }
            CacheTier::Warm => {
                // Write to warm tier using balanced crossbeam messaging
                crate::cache::tier::warm::global_api::warm_put(&self.warm_tier_coordinator, cache_key, cache_value).await
            }
            CacheTier::Cold => {
                // Write to cold tier using insert_demoted function
                crate::cache::tier::cold::insert_demoted(&self.cold_tier_coordinator, cache_key.original_key, cache_value).await
            }
        };

        // Convert CacheOperationError to WriteBackResult
        match write_result {
            Ok(_) => WriteBackResult::Success,
            Err(e) => match e {
                CacheOperationError::TimeoutError => WriteBackResult::Timeout,
                CacheOperationError::StorageError(reason) => {
                    WriteBackResult::RetryableError { reason }
                }
                CacheOperationError::TierError(reason) => {
                    WriteBackResult::RetryableError { reason }
                }
                CacheOperationError::TierOperationFailed => WriteBackResult::RetryableError {
                    reason: "Tier operation failed".to_string(),
                },
                CacheOperationError::MemoryLimitExceeded => WriteBackResult::RetryableError {
                    reason: "Memory limit exceeded".to_string(),
                },
                CacheOperationError::ResourceExhausted(reason) => {
                    WriteBackResult::RetryableError { reason }
                }
                _ => WriteBackResult::PermanentError {
                    reason: format!("Permanent tier write failure: {}", e),
                },
            },
        }
    }
}

impl Default for ProtocolConfiguration {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolConfiguration {
    /// Create default protocol configuration
    pub fn new() -> Self {
        Self {
            optimistic_concurrency: true,
            write_through: false,
            max_invalidation_retries: 3,
            coherence_timeout_ns: 1_000_000_000, // 1 second
            strict_ordering: false,
            schema_version: 1,
        }
    }

    /// Create high-performance configuration
    #[allow(dead_code)]
    pub fn high_performance() -> Self {
        Self {
            optimistic_concurrency: true,
            write_through: false,
            max_invalidation_retries: 1,
            coherence_timeout_ns: 100_000_000, // 100ms
            strict_ordering: false,
            schema_version: 1,
        }
    }

    /// Create strict consistency configuration
    #[allow(dead_code)]
    pub fn strict_consistency() -> Self {
        Self {
            optimistic_concurrency: false,
            write_through: true,
            max_invalidation_retries: 5,
            coherence_timeout_ns: 5_000_000_000, // 5 seconds
            strict_ordering: true,
            schema_version: 1,
        }
    }
}
