//! Core data structures for MESI-like cache coherence protocol
//!
//! This module defines the fundamental data structures used by the coherence controller,
//! including cache line states, coherence metadata, and key types.

use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, AtomicU8, Ordering};

use crossbeam_skiplist::SkipMap;
use crossbeam_utils::CachePadded;

use crate::cache::traits::{CacheKey, CacheValue};

/// MESI-like cache coherence controller
#[derive(Debug)]
pub struct CoherenceController<K: CacheKey, V: CacheValue> {
    /// Cache line state tracking
    pub cache_line_states: SkipMap<CoherenceKey<K>, CacheLineState>,
    /// Inter-tier communication channels
    pub communication_hub: super::communication::CommunicationHub<K, V>,
    /// Atomic coherence statistics
    pub coherence_stats: super::statistics::CoherenceStatistics,
    /// Protocol configuration
    pub protocol_config: ProtocolConfiguration,
    /// State transition validator
    pub transition_validator: super::state_management::StateTransitionValidator,
    /// Invalidation manager
    pub invalidation_manager: super::invalidation::InvalidationManager<K>,
    /// Write propagation system
    pub write_propagation: super::write_propagation::WritePropagationSystem<K, V>,
}

/// Cache line state with MESI protocol tracking
#[derive(Debug)]
#[repr(align(64))] // Cache-line aligned for performance
pub struct CacheLineState {
    /// Current MESI state (atomic for lock-free transitions)
    pub mesi_state: CachePadded<AtomicU8>,
    /// Tier ownership bitmask (Hot=1, Warm=2, Cold=4)
    pub tier_ownership: CachePadded<AtomicU8>,
    /// Version number for optimistic concurrency
    pub version: CachePadded<AtomicU64>,
    /// Last modification timestamp
    pub last_modified_ns: CachePadded<AtomicU64>,
    /// Access count across all tiers
    pub total_access_count: CachePadded<AtomicU64>,
    /// Coherence metadata
    pub metadata: CoherenceMetadata,
}

/// Coherence metadata for advanced tracking
#[derive(Debug)]
pub struct CoherenceMetadata {
    /// Original data size
    pub original_size: AtomicU32,
    /// Dirty tier bitmask (which tiers have modifications)
    pub dirty_tiers: AtomicU8,
    /// Lock ownership (for exclusive access)
    pub lock_owner: AtomicU16,
    /// Invalidation sequence number
    pub invalidation_seq: AtomicU32,
    /// Write-back pending flag
    pub writeback_pending: AtomicBool,
}

/// Cache tier enumeration for coherence tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheTier {
    Hot = 1,
    Warm = 2,
    Cold = 4,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[derive(Debug, Clone)]
pub struct ProtocolConfiguration {
    /// Enable optimistic concurrency
    pub optimistic_concurrency: bool,
    /// Enable write-through mode
    pub write_through: bool,
    /// Maximum invalidation retries
    pub max_invalidation_retries: u32,
    /// Coherence timeout in nanoseconds
    pub coherence_timeout_ns: u64,
    /// Enable strict ordering
    pub strict_ordering: bool,
    /// Current schema version for compatibility validation
    pub schema_version: u32,
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

    /// Get current version
    pub fn get_version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    /// Increment version
    pub fn increment_version(&self) -> u64 {
        self.version.fetch_add(1, Ordering::AcqRel) + 1
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
    pub fn mark_dirty(&self, tier: CacheTier) {
        self.dirty_tiers.fetch_or(tier as u8, Ordering::Relaxed);
    }

    /// Clear dirty flag for tier
    pub fn clear_dirty(&self, tier: CacheTier) {
        self.dirty_tiers.fetch_and(!(tier as u8), Ordering::Relaxed);
    }

    /// Check if tier is dirty
    pub fn is_dirty(&self, tier: CacheTier) -> bool {
        (self.dirty_tiers.load(Ordering::Relaxed) & (tier as u8)) != 0
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
    pub fn can_read(&self) -> bool {
        matches!(
            self,
            MesiState::Shared | MesiState::Exclusive | MesiState::Modified
        )
    }

    /// Check if state allows writing
    pub fn can_write(&self) -> bool {
        matches!(self, MesiState::Exclusive | MesiState::Modified)
    }

    /// Check if state is valid
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

impl<K: CacheKey, V: CacheValue> CoherenceController<K, V> {
    /// Create new coherence controller
    pub fn new(config: ProtocolConfiguration) -> Self {
        Self {
            cache_line_states: SkipMap::new(),
            communication_hub: super::communication::CommunicationHub::new(),
            coherence_stats: super::statistics::CoherenceStatistics::new(),
            protocol_config: config,
            transition_validator: super::state_management::StateTransitionValidator::new(),
            invalidation_manager: super::invalidation::InvalidationManager::new(1000),
            write_propagation: super::write_propagation::WritePropagationSystem::new(1_000_000_000),
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

        Ok(response)
    }

    /// Handle write request from a cache tier
    pub fn handle_write_request(
        &self,
        key: &K,
        requesting_tier: CacheTier,
        data: std::sync::Arc<V>,
    ) -> Result<super::communication::WriteResponse, super::communication::CoherenceError> {
        use std::time::Instant;

        let start_time = Instant::now();
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
        data: std::sync::Arc<V>,
    ) -> Result<super::communication::WriteResponse, super::communication::CoherenceError> {
        self.handle_cache_miss(key, requesting_tier, true)?;

        // Submit write-back request
        self.write_propagation.submit_writeback(
            key.clone(),
            data,
            requesting_tier,
            self.determine_target_tier(requesting_tier),
            0,
            super::write_propagation::WritePriority::Normal,
        );

        Ok(super::communication::WriteResponse::Success)
    }

    /// Handle write to shared cache line
    pub fn handle_shared_write(
        &self,
        key: &CoherenceKey<K>,
        requesting_tier: CacheTier,
        data: std::sync::Arc<V>,
    ) -> Result<super::communication::WriteResponse, super::communication::CoherenceError> {
        // Send invalidation to other tiers
        self.invalidation_manager.submit_invalidation(
            key.clone(),
            self.get_other_tier(requesting_tier),
            InvalidationReason::WriteConflict,
            super::invalidation::InvalidationPriority::High,
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

        // Submit write-back request
        self.write_propagation.submit_writeback(
            key.clone(),
            data,
            requesting_tier,
            self.determine_target_tier(requesting_tier),
            0,
            super::write_propagation::WritePriority::High,
        );

        Ok(super::communication::WriteResponse::Success)
    }

    /// Handle write to exclusive cache line
    pub fn handle_exclusive_write(
        &self,
        key: &CoherenceKey<K>,
        requesting_tier: CacheTier,
        data: std::sync::Arc<V>,
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

        // Submit write-back request with lower priority (already exclusive)
        self.write_propagation.submit_writeback(
            key.clone(),
            data,
            requesting_tier,
            self.determine_target_tier(requesting_tier),
            0,
            super::write_propagation::WritePriority::Normal,
        );

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
}

impl ProtocolConfiguration {
    /// Create default protocol configuration
    pub fn default() -> Self {
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
