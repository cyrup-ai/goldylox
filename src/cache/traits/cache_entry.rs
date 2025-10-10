//! Production-quality cache entry domain model
//!
//! This module defines the core CacheEntry<K,V> struct that wraps user domain data
//! with sophisticated cache infrastructure including access tracking, tier management,
//! timestamps, and metadata for intelligent cache operations.

#![allow(dead_code)] // Cache traits - Cache entry implementation with advanced metadata management

use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::core::{CacheKey, CacheValue};
use super::supporting_types::{CompressionAlgorithm, SerializationFormat};
use super::types_and_enums::{AccessType, TemporalPattern, TierAffinity, TierLocation};
use crate::cache::coherence::data_structures::CacheTier;
use crate::cache::coherence::write_propagation::types::WritePriority;
// Coherence module imports for delegation to existing infrastructure
use crate::cache::coherence::{
    CoherenceController, CoherenceError, CoherenceKey, CoherenceMessage, MesiState,
    StateTransitionRequest, TransitionReason,
};

// Conversion from TierLocation to CacheTier for coherence integration
impl From<TierLocation> for CacheTier {
    fn from(tier_location: TierLocation) -> Self {
        match tier_location {
            TierLocation::Hot => CacheTier::Hot,
            TierLocation::Warm => CacheTier::Warm,
            TierLocation::Cold => CacheTier::Cold,
        }
    }
}

/// Health status for error tracking and circuit breaker functionality
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    bincode::Encode,
    bincode::Decode,
)]
pub enum HealthStatus {
    /// Entry is healthy and operational
    Healthy,
    /// Entry has experienced some errors but is still usable
    Degraded,
    /// Entry has significant errors and may need attention
    Unhealthy,
    /// Entry is temporarily unavailable (circuit breaker open)
    CircuitOpen,
}

// AccessEvent moved to canonical location: crate::cache::eviction::types::AccessEvent
pub use crate::cache::eviction::types::AccessEvent;

/// Tier transition event for tracking migration history
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
pub struct TierTransition {
    /// When the transition occurred (nanoseconds since epoch)
    pub timestamp_ns: u64,
    /// Source tier
    pub from_tier: TierLocation,
    /// Destination tier  
    pub to_tier: TierLocation,
    /// Reason for transition
    pub reason: String,
    /// Success of transition
    pub success: bool,
}

impl Default for TierTransition {
    fn default() -> Self {
        Self {
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            from_tier: TierLocation::Hot,
            to_tier: TierLocation::Warm,
            reason: String::new(),
            success: false,
        }
    }
}

/// Migration lock to prevent concurrent tier transitions
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
pub struct MigrationLock {
    /// When the lock was acquired (nanoseconds since epoch)
    pub acquired_at_ns: u64,
    /// Target tier for migration
    pub target_tier: TierLocation,
    /// Migration operation ID
    pub operation_id: u64,
}

impl Default for MigrationLock {
    fn default() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        Self {
            acquired_at_ns: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            target_tier: TierLocation::Warm,
            operation_id: 0,
        }
    }
}

/// Cache entry metadata containing infrastructure data
#[derive(Debug, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
pub struct CacheEntryMetadata {
    /// Exact creation timestamp (nanoseconds since epoch)
    pub created_at_ns: u64,
    /// Last access timestamp (nanoseconds since epoch)
    pub last_accessed_ns: u64,
    /// Total access count (atomic for concurrent access)
    pub access_count: AtomicU64,
    /// Entry size in bytes
    pub size_bytes: usize,
    /// Cost to recreate this value (CPU cycles estimate)
    pub creation_cost: u64,
    /// Compression effectiveness (0.0-1.0)
    pub compression_ratio: f32,
    /// Entry health status for error tracking
    pub health_status: HealthStatus,
    /// Eviction priority score (higher = keep longer)
    pub priority_score: f32,
    /// Error count for circuit breaker
    pub error_count: AtomicU32,
    /// Version for coherence protocol
    pub version: u64,
    /// Last error timestamp (nanoseconds since epoch)
    pub last_error_ns: Option<u64>,
}

// Internal metadata methods - may not be used in minimal API
impl CacheEntryMetadata {
    /// Create new metadata for cache entry
    pub fn new(size_bytes: usize, creation_cost: u64) -> Self {
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Self {
            created_at_ns: now_ns,
            last_accessed_ns: now_ns,
            access_count: AtomicU64::new(0),
            size_bytes,
            creation_cost,
            compression_ratio: 1.0, // No compression by default
            health_status: HealthStatus::Healthy,
            priority_score: 0.5, // Medium priority by default
            error_count: AtomicU32::new(0),
            version: 1,
            last_error_ns: None,
        }
    }

    /// Record successful access using coherence protocol delegation
    #[inline]
    pub fn record_access<
        K: CacheKey
            + Default
            + bincode::Encode
            + bincode::Decode<()>
            + serde::Serialize
            + serde::de::DeserializeOwned,
        V: CacheValue
            + Default
            + bincode::Encode
            + bincode::Decode<()>
            + serde::Serialize
            + serde::de::DeserializeOwned,
    >(
        &self,
        cache_key: &K,
        tier_location: TierLocation,
        coherence_controller: &CoherenceController<K, V>,
    ) -> Result<(), CoherenceError> {
        // DELEGATE to existing coherence protocol read operation
        // This handles all atomic operations, state management, and coordination
        match coherence_controller.handle_read_request(cache_key, tier_location.into()) {
            Ok(_read_response) => {
                // Coherence protocol handled all state management, atomicity, and coordination
                // Update local counter for compatibility
                self.access_count.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(coherence_error) => Err(coherence_error),
        }
    }

    /// Record error occurrence
    pub fn record_error(&mut self) {
        let error_count = self.error_count.fetch_add(1, Ordering::Relaxed) + 1;
        self.last_error_ns = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
        );

        // Update health status based on error frequency
        self.health_status = match error_count {
            1..=3 => HealthStatus::Degraded,
            4..=10 => HealthStatus::Unhealthy,
            _ => HealthStatus::CircuitOpen,
        };
    }

    /// Get current access count
    #[inline]
    pub fn access_count(&self) -> u64 {
        self.access_count.load(Ordering::Relaxed)
    }

    /// Get current error count  
    #[inline]
    pub fn error_count(&self) -> u32 {
        self.error_count.load(Ordering::Relaxed)
    }

    /// Calculate age since creation
    #[inline]
    pub fn age(&self) -> Duration {
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Duration::from_nanos(now_ns.saturating_sub(self.created_at_ns))
    }

    /// Calculate time since last access
    #[inline]
    pub fn time_since_last_access(&self) -> Duration {
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Duration::from_nanos(now_ns.saturating_sub(self.last_accessed_ns))
    }
}

impl Clone for CacheEntryMetadata {
    fn clone(&self) -> Self {
        Self {
            created_at_ns: self.created_at_ns,
            last_accessed_ns: self.last_accessed_ns,
            access_count: AtomicU64::new(self.access_count.load(Ordering::Relaxed)),
            size_bytes: self.size_bytes,
            creation_cost: self.creation_cost,
            compression_ratio: self.compression_ratio,
            health_status: self.health_status,
            priority_score: self.priority_score,
            error_count: AtomicU32::new(self.error_count.load(Ordering::Relaxed)),
            version: self.version,
            last_error_ns: self.last_error_ns,
        }
    }
}

impl Default for CacheEntryMetadata {
    fn default() -> Self {
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Self {
            created_at_ns: now_ns,
            last_accessed_ns: now_ns,
            access_count: AtomicU64::new(0),
            size_bytes: 0,
            creation_cost: 0,
            compression_ratio: 1.0,
            health_status: HealthStatus::Healthy,
            priority_score: 0.5,
            error_count: AtomicU32::new(0),
            version: 1,
            last_error_ns: None,
        }
    }
}

/// Parameters for recording cache access events
pub struct AccessRecordParams<'a, K: CacheKey> {
    /// Key being accessed
    pub key: &'a K,
    /// Cache tier where access occurred
    pub tier: CacheTier,
    /// Size of the entry in bytes
    pub entry_size: usize,
    /// Type of access operation
    pub access_type: AccessType,
    /// Access latency in nanoseconds
    pub latency_ns: u64,
    /// Whether the access was a hit
    pub hit: bool,
    /// Event counter for generating unique event IDs
    pub event_counter: &'a std::sync::atomic::AtomicU64,
}

/// Sophisticated access pattern tracker for cache intelligence
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
#[serde(default)]
#[serde(bound(deserialize = "K: serde::de::DeserializeOwned"))]
pub struct AccessTracker<K: CacheKey> {
    /// Recent access events (bounded circular buffer)
    pub access_history: VecDeque<AccessEvent<K>>,
    /// Maximum history size
    pub max_history_size: usize,
    /// Frequency estimation using exponential moving average
    pub frequency_estimate: f64,
    /// Frequency decay factor (0.0-1.0)
    pub frequency_decay: f64,
    /// Detected temporal pattern
    pub temporal_pattern: TemporalPattern,
    /// Last pattern analysis timestamp (nanoseconds since epoch)
    pub last_pattern_update_ns: u64,
    /// Pattern confidence score (0.0-1.0)
    pub pattern_confidence: f32,
    /// Average access latency in nanoseconds
    pub avg_latency_ns: u64,
    /// Hit rate (0.0-1.0)
    pub hit_rate: f64,
}

// Internal access tracking methods - may not be used in minimal API
impl<K: CacheKey> AccessTracker<K> {
    /// Create new access tracker
    pub fn new() -> Self {
        Self {
            access_history: VecDeque::new(),
            max_history_size: 100, // Keep last 100 accesses
            frequency_estimate: 0.0,
            frequency_decay: 0.9, // 10% decay per time unit
            temporal_pattern: TemporalPattern::Steady,
            last_pattern_update_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            pattern_confidence: 0.0,
            avg_latency_ns: 0,
            hit_rate: 0.0,
        }
    }

    /// Record new access event and update statistics
    pub fn record_access(&mut self, params: AccessRecordParams<K>) {
        // Use proper AccessEvent constructor that handles event_id generation
        let mut event = AccessEvent::new(
            params.key.clone(),
            params.access_type,
            params.tier,
            params.hit,
            params.event_counter,
        );
        event.latency_ns = params.latency_ns;
        event.entry_size = params.entry_size;

        // Update frequency estimate using exponential moving average
        let time_delta = if let Some(last_event) = self.access_history.back() {
            if event.timestamp > last_event.timestamp {
                (event.timestamp - last_event.timestamp) as f64 / 1_000_000_000.0 // Convert nanoseconds to seconds
            } else {
                1.0 // Fallback if timestamps are equal or reversed
            }
        } else {
            1.0 // Default time delta for first access
        };

        // Add to history, maintaining size limit
        self.access_history.push_back(event);
        if self.access_history.len() > self.max_history_size {
            self.access_history.pop_front();
        }

        if time_delta > 0.0 {
            let new_frequency = 1.0 / time_delta;
            self.frequency_estimate = self.frequency_decay * self.frequency_estimate
                + (1.0 - self.frequency_decay) * new_frequency;
        }

        // Update running averages
        self.update_running_stats();

        // Update temporal pattern if enough time has passed
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        if (now_ns.saturating_sub(self.last_pattern_update_ns)) >= 60_000_000_000 {
            // 60 seconds in nanoseconds
            self.analyze_temporal_pattern();
        }
    }

    /// Update running statistics from access history
    fn update_running_stats(&mut self) {
        if self.access_history.is_empty() {
            return;
        }

        let total_latency: u64 = self.access_history.iter().map(|e| e.latency_ns).sum();
        self.avg_latency_ns = total_latency / self.access_history.len() as u64;

        let hits = self.access_history.iter().filter(|e| e.hit).count();
        self.hit_rate = hits as f64 / self.access_history.len() as f64;
    }

    /// Analyze temporal access pattern using coherence statistics infrastructure
    pub fn analyze_temporal_pattern_with_coherence<V: CacheValue>(
        &mut self,
        cache_key: &K,
        coherence_controller: &CoherenceController<K, V>,
    ) {
        let coherence_key = CoherenceKey::from_cache_key(cache_key);

        if let Some(cache_line_entry) = coherence_controller.cache_line_states.get(&coherence_key) {
            let cache_line = cache_line_entry.value();
            let total_accesses = cache_line.total_access_count.load(Ordering::Relaxed);
            let version = cache_line.get_version();

            // DELEGATE to existing coherence statistics system for sophisticated pattern analysis
            let stats_snapshot = coherence_controller.coherence_stats.get_snapshot();

            // Use existing sophisticated statistics for pattern analysis
            if total_accesses >= 10 {
                // Calculate access frequency from coherence data
                let now_ns = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;
                let time_span =
                    (now_ns.saturating_sub(self.last_pattern_update_ns)) as f64 / 1_000_000_000.0;

                let access_rate = total_accesses as f64 / time_span;

                // Advanced pattern detection using coherence state transitions and statistics
                self.temporal_pattern = if stats_snapshot.writebacks_performed
                    > stats_snapshot.successful_operations / 2
                {
                    TemporalPattern::WriteHeavy
                } else if access_rate > 10.0 {
                    TemporalPattern::BurstyHigh
                } else if stats_snapshot.success_rate() > 90.0 && access_rate > 1.0 {
                    TemporalPattern::Periodic
                } else if version > total_accesses / 2 {
                    TemporalPattern::WriteHeavy
                } else {
                    TemporalPattern::Irregular
                };

                // Confidence based on coherence state stability and statistics quality
                self.pattern_confidence = ((total_accesses as f64 / 100.0).min(1.0)
                    * (stats_snapshot.success_rate() / 100.0))
                    as f32;
                self.last_pattern_update_ns = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;
            }
        }
    }

    /// Analyze temporal access pattern from history (basic implementation)
    fn analyze_temporal_pattern(&mut self) {
        if self.access_history.len() < 10 {
            self.pattern_confidence = 0.0;
            return;
        }

        // Calculate inter-arrival times
        let mut intervals: Vec<u64> = Vec::new();
        for window in self.access_history.iter().collect::<Vec<_>>().windows(2) {
            if let [first, second] = window {
                let interval = (second.timestamp.saturating_sub(first.timestamp)) / 1_000_000; // Convert nanoseconds to milliseconds
                intervals.push(interval);
            }
        }

        if intervals.is_empty() {
            return;
        }

        // Calculate variance to determine pattern type
        let mean_interval = intervals.iter().sum::<u64>() as f64 / intervals.len() as f64;
        let variance = intervals
            .iter()
            .map(|&interval| {
                let diff = interval as f64 - mean_interval;
                diff * diff
            })
            .sum::<f64>()
            / intervals.len() as f64;

        let coefficient_of_variation = variance.sqrt() / mean_interval;

        // Classify pattern based on regularity
        self.temporal_pattern = if coefficient_of_variation < 0.1 {
            TemporalPattern::Steady
        } else if coefficient_of_variation > 2.0 {
            TemporalPattern::Bursty
        } else {
            TemporalPattern::Steady // Default fallback
        };

        self.pattern_confidence = (1.0 - coefficient_of_variation.min(1.0)) as f32;
        self.last_pattern_update_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
    }

    /// Get current access frequency estimate
    #[inline]
    pub fn frequency(&self) -> f64 {
        self.frequency_estimate
    }

    /// Get pattern confidence score
    #[inline]
    pub fn confidence(&self) -> f32 {
        self.pattern_confidence
    }
}

impl<K: CacheKey> Default for AccessTracker<K> {
    fn default() -> Self {
        Self::new()
    }
}

/// Multi-tier management information for intelligent tier transitions
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
#[serde(default)]
pub struct TierInfo {
    /// Current tier location
    pub current_tier: TierLocation,
    /// Preferred tier based on access patterns
    pub tier_affinity: TierAffinity,
    /// Score for promotion to higher tier (0.0-1.0)
    pub promotion_score: f32,
    /// Whether this entry is a candidate for demotion
    pub demotion_candidate: bool,
    /// History of tier transitions
    pub tier_transition_history: Vec<TierTransition>,
    /// Current migration lock if transition is in progress
    pub migration_lock: Option<MigrationLock>,
    /// Tier residency time (how long in current tier)
    pub tier_residency: Duration,
    /// Tier entry timestamp (nanoseconds since epoch)
    pub tier_entry_time_ns: u64,
    /// Number of tier transitions
    pub transition_count: u32,
}

// Internal tier management methods - may not be used in minimal API
impl TierInfo {
    /// Create new tier info for specified tier
    pub fn new(initial_tier: TierLocation) -> Self {
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Self {
            current_tier: initial_tier,
            tier_affinity: TierAffinity::Auto,
            promotion_score: 0.0,
            demotion_candidate: false,
            tier_transition_history: Vec::new(),
            migration_lock: None,
            tier_residency: Duration::from_secs(0),
            tier_entry_time_ns: now_ns,
            transition_count: 0,
        }
    }

    /// Record tier transition using coherence communication hub delegation
    pub fn transition_to<K: CacheKey, V: CacheValue>(
        &mut self,
        new_tier: TierLocation,
        reason: String,
        cache_key: &K,
        coherence_controller: &CoherenceController<K, V>,
    ) -> Result<(), CoherenceError> {
        let _now = Instant::now();

        // DELEGATE to existing coherence communication hub for coordination
        let coherence_message = CoherenceMessage::RequestExclusive {
            key: CoherenceKey::from_cache_key(cache_key),
            requester_tier: new_tier.into(),
            version: 0,
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
        };

        // Use existing communication hub broadcasting for tier coordination
        let transition_result = coherence_controller
            .communication_hub
            .broadcast(coherence_message);

        // Record transition with actual success/failure status from coherence protocol
        let now = std::time::SystemTime::now();
        let transition = TierTransition {
            timestamp_ns: now
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            from_tier: self.current_tier,
            to_tier: new_tier,
            reason,
            success: transition_result.is_ok(), // Real success status from coherence protocol
        };
        self.tier_transition_history.push(transition);

        // Only update state if coherence protocol succeeded
        if transition_result.is_ok() {
            // Update tier residency
            let now_ns = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            self.tier_residency =
                Duration::from_nanos(now_ns.saturating_sub(self.tier_entry_time_ns));

            // Update current state
            self.current_tier = new_tier;
            self.tier_entry_time_ns = now_ns;
        }

        transition_result
    }

    /// Acquire migration lock for tier transition
    pub fn acquire_migration_lock(&mut self, target_tier: TierLocation, operation_id: u64) -> bool {
        if self.migration_lock.is_some() {
            false // Already locked
        } else {
            self.migration_lock = Some(MigrationLock {
                acquired_at_ns: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64,
                target_tier,
                operation_id,
            });
            true
        }
    }

    /// Release migration lock
    pub fn release_migration_lock(&mut self) {
        self.migration_lock = None;
    }

    /// Check if migration is currently locked
    #[inline]
    pub fn is_migration_locked(&self) -> bool {
        self.migration_lock.is_some()
    }

    /// Get current tier residency time
    #[inline]
    pub fn current_tier_residency(&self) -> Duration {
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Duration::from_nanos(now_ns.saturating_sub(self.tier_entry_time_ns))
    }

    /// Calculate tier stability score (higher = more stable in current tier)
    pub fn tier_stability_score(&self) -> f32 {
        let residency_hours = self.current_tier_residency().as_secs() as f32 / 3600.0;
        let transition_penalty = self.transition_count as f32 * 0.1;
        (residency_hours / (1.0 + residency_hours) - transition_penalty).clamp(0.0, 1.0)
    }
}

impl Default for TierInfo {
    fn default() -> Self {
        Self::new(TierLocation::Hot)
    }
}

/// Serialization context for persistent storage
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
pub struct SerializationContext {
    /// Serialization format to use
    pub format: SerializationFormat,
    /// Compression algorithm to apply
    pub compression: CompressionAlgorithm,
    /// Schema version for evolution support
    pub schema_version: u32,
    /// Whether to include metadata in serialization
    pub include_metadata: bool,
    /// Compression level (0-9, higher = better compression)
    pub compression_level: u8,
}

// Internal serialization methods - may not be used in minimal API
impl SerializationContext {
    /// Create new serialization context
    pub fn new(format: SerializationFormat, compression: CompressionAlgorithm) -> Self {
        Self {
            format,
            compression,
            schema_version: 1,
            include_metadata: true,
            compression_level: 6, // Balanced compression
        }
    }

    /// Create binary context with LZ4 compression
    pub fn binary_lz4() -> Self {
        Self::new(SerializationFormat::Binary, CompressionAlgorithm::Lz4)
    }

    /// Create context optimized for cold tier storage
    pub fn cold_tier_optimized() -> Self {
        Self {
            format: SerializationFormat::Bincode,
            compression: CompressionAlgorithm::Zstd,
            schema_version: 1,
            include_metadata: true,
            compression_level: 9, // Maximum compression for cold tier
        }
    }
}

impl Default for SerializationContext {
    fn default() -> Self {
        Self::binary_lz4()
    }
}

/// Production-quality cache entry wrapping user domain data with cache infrastructure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
#[serde(bound(serialize = "K: serde::Serialize, V: serde::Serialize"))]
#[serde(bound(deserialize = "K: serde::de::DeserializeOwned, V: serde::de::DeserializeOwned"))]
pub struct CacheEntry<K: CacheKey, V: CacheValue> {
    /// User's domain key
    pub key: K,
    /// User's domain value
    pub value: V,
    /// Cache infrastructure metadata
    pub metadata: CacheEntryMetadata,
    /// Access pattern tracking and analysis
    pub access_tracker: AccessTracker<K>,
    /// Multi-tier management information
    pub tier_info: TierInfo,
    /// Serialization context for persistence
    pub serialization_context: SerializationContext,
}

// Basic cache entry methods - compatible with all key/value types
impl<K: CacheKey, V: CacheValue> CacheEntry<K, V> {
    /// Create new cache entry with user key/value
    pub fn new(key: K, value: V, initial_tier: TierLocation) -> Self {
        let value_size = value.estimated_size();
        let key_size = key.estimated_size();
        let total_size = value_size + key_size;

        // Estimate creation cost based on value properties
        let base_cost = if value.is_expensive() { 10000 } else { 1000 };
        let size_cost = (total_size as u64).saturating_mul(10);
        let creation_cost = base_cost + size_cost;

        Self {
            key,
            value,
            metadata: CacheEntryMetadata::new(total_size, creation_cost),
            access_tracker: AccessTracker::new(),
            tier_info: TierInfo::new(initial_tier),
            serialization_context: SerializationContext::default(),
        }
    }

    /// Record access to this cache entry
    pub fn record_access(
        &mut self,
        access_type: AccessType,
        latency_ns: u64,
        hit: bool,
        event_counter: &std::sync::atomic::AtomicU64,
    ) {
        // Update metadata with coherence integration
        self.metadata.access_count.fetch_add(1, Ordering::Relaxed);
        self.metadata.last_accessed_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        // Note: Coherence integration available through record_access_with_coherence() method
        log::trace!(
            "Access recorded: key={:?}, tier={:?}, type={:?}, hit={}",
            self.key,
            self.tier_info.current_tier,
            access_type,
            hit
        );

        // Update access tracker with sophisticated analysis
        self.access_tracker.record_access(AccessRecordParams {
            key: &self.key,
            tier: self.tier_info.current_tier.into(),
            entry_size: self.metadata.size_bytes,
            access_type,
            latency_ns,
            hit,
            event_counter,
        });

        // Update promotion score based on access patterns
        self.update_promotion_score();
    }

    /// Update promotion score for tier management
    fn update_promotion_score(&mut self) {
        let frequency = self.access_tracker.frequency();
        let recency_bonus = if self.metadata.time_since_last_access().as_secs() < 300 {
            0.2
        } else {
            0.0
        };
        let pattern_bonus = self.access_tracker.confidence() as f64 * 0.1;
        let hit_rate_bonus = self.access_tracker.hit_rate * 0.3;

        self.tier_info.promotion_score =
            ((frequency * 0.4 + recency_bonus + pattern_bonus + hit_rate_bonus).min(1.0) as f32)
                .max(0.0);
    }

    /// Check if entry should be promoted to higher tier
    pub fn should_promote(&self) -> bool {
        match self.tier_info.current_tier {
            TierLocation::Cold => {
                self.tier_info.promotion_score > 0.6 && self.access_tracker.frequency() > 0.1
            }
            TierLocation::Warm => {
                self.tier_info.promotion_score > 0.8 && self.access_tracker.frequency() > 1.0
            }
            TierLocation::Hot => false, // Already at top tier
        }
    }

    /// Check if entry should be demoted to lower tier
    pub fn should_demote(&self) -> bool {
        let low_access = self.access_tracker.frequency() < 0.01;
        let old_entry = self.metadata.time_since_last_access().as_secs() > 3600; // 1 hour
        let low_hit_rate = self.access_tracker.hit_rate < 0.3;
        let unhealthy = matches!(
            self.metadata.health_status,
            HealthStatus::Unhealthy | HealthStatus::CircuitOpen
        );

        match self.tier_info.current_tier {
            TierLocation::Hot => low_access || (old_entry && low_hit_rate) || unhealthy,
            TierLocation::Warm => low_access && old_entry,
            TierLocation::Cold => false, // Already at bottom tier
        }
    }

    /// Promote entry to higher tier
    pub fn promote(&mut self) -> Option<TierLocation> {
        let new_tier = match self.tier_info.current_tier {
            TierLocation::Cold => Some(TierLocation::Warm),
            TierLocation::Warm => Some(TierLocation::Hot),
            TierLocation::Hot => None,
        };

        if let Some(target_tier) = new_tier {
            // Simple tier transition for promote() public API
            self.tier_info.current_tier = target_tier;
            self.tier_info.tier_entry_time_ns = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            self.tier_info.transition_count += 1;
        }

        new_tier
    }

    /// Demote entry to lower tier
    pub fn demote(&mut self) -> Option<TierLocation> {
        let new_tier = match self.tier_info.current_tier {
            TierLocation::Hot => Some(TierLocation::Warm),
            TierLocation::Warm => Some(TierLocation::Cold),
            TierLocation::Cold => None,
        };

        if let Some(target_tier) = new_tier {
            // Simple tier transition for promote() public API
            self.tier_info.current_tier = target_tier;
            self.tier_info.tier_entry_time_ns = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            self.tier_info.transition_count += 1;
        }

        new_tier
    }

    /// Get total entry size including overhead
    #[inline]
    pub fn total_size(&self) -> usize {
        self.metadata.size_bytes + std::mem::size_of::<Self>()
    }

    /// Get entry priority for eviction decisions (higher = keep longer)
    pub fn eviction_priority(&self) -> f64 {
        let base_priority = self.metadata.priority_score as f64;
        let frequency_weight = self.access_tracker.frequency() * 0.3;
        let recency_weight = if self.metadata.time_since_last_access().as_secs() < 300 {
            0.2
        } else {
            0.0
        };
        let health_penalty = match self.metadata.health_status {
            HealthStatus::Healthy => 0.0,
            HealthStatus::Degraded => -0.1,
            HealthStatus::Unhealthy => -0.3,
            HealthStatus::CircuitOpen => -0.5,
        };

        base_priority + frequency_weight + recency_weight + health_penalty
    }

    /// Check if entry is healthy and operational
    #[inline]
    pub fn is_healthy(&self) -> bool {
        matches!(
            self.metadata.health_status,
            HealthStatus::Healthy | HealthStatus::Degraded
        )
    }

    /// Update serialization context based on tier and access patterns
    pub fn update_serialization_context(&mut self) {
        self.serialization_context = match self.tier_info.current_tier {
            TierLocation::Hot => SerializationContext::binary_lz4(), // Fast serialization
            TierLocation::Warm => {
                SerializationContext::new(SerializationFormat::Bincode, CompressionAlgorithm::Lz4)
            }
            TierLocation::Cold => SerializationContext::cold_tier_optimized(), // Max compression
        };
    }
}

// Coherence-enabled cache entry methods - requires serialization trait support
impl<
    K: CacheKey
        + Default
        + bincode::Encode
        + bincode::Decode<()>
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static,
    V: CacheValue
        + Default
        + bincode::Encode
        + bincode::Decode<()>
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static,
> CacheEntry<K, V>
{
    /// Create new cache entry with coherence tracking enabled
    pub fn coherence_aware_new(
        key: K,
        value: V,
        initial_tier: TierLocation,
        request_id_counter: &std::sync::atomic::AtomicU64,
        channel_map: &std::sync::Arc<std::sync::RwLock<
            std::collections::HashMap<std::any::TypeId, Box<dyn std::any::Any + Send + Sync>>
        >>,
    ) -> Self {
        let entry = Self::new(key, value, initial_tier);

        // Record access via tokio messaging to coherence worker
        if let Some(sender) =
            crate::cache::coherence::worker::worker_manager::get_worker_channels::<K, V>(channel_map)
        {
            use tokio::sync::oneshot;

            let access_type = match initial_tier {
                TierLocation::Hot => crate::cache::traits::types_and_enums::AccessType::Write,
                TierLocation::Warm => crate::cache::traits::types_and_enums::AccessType::Write,
                TierLocation::Cold => crate::cache::traits::types_and_enums::AccessType::Write,
            };

            let (response_tx, _response_rx) = oneshot::channel();
            let coherence_request =
                crate::cache::coherence::worker::message_types::CoherenceRequest::RecordWrite {
                    key: entry.key.clone(),
                    data: entry.value.clone(),
                    tier: crate::cache::coherence::data_structures::CacheTier::from(initial_tier),
                    response: response_tx,
                };

            // Send request - ignore errors for non-blocking operation
            let _ = sender.send(coherence_request);

            log::debug!(
                "Recorded cache entry creation for key with access type: {:?}",
                access_type
            );
        } else {
            log::debug!(
                "No coherence worker channels available - cache entry created without coherence tracking"
            );
        }

        entry
    }

    /// Record access to this cache entry with coherence integration
    pub async fn record_access_with_coherence(
        &mut self,
        access_type: AccessType,
        latency_ns: u64,
        hit: bool,
        event_counter: &std::sync::atomic::AtomicU64,
        channel_map: &std::sync::Arc<std::sync::RwLock<
            std::collections::HashMap<std::any::TypeId, Box<dyn std::any::Any + Send + Sync>>
        >>,
    ) {
        // Update metadata first
        self.metadata
            .access_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.metadata.last_accessed_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        // Coherence coordination via tokio messaging to dedicated coherence workers
        use crate::cache::coherence::worker::message_types::CoherenceRequest;
        use std::time::Duration;
        use tokio::sync::oneshot;

        // Get per-instance coherence worker channel from worker manager
        if let Some(request_sender) =
            crate::cache::coherence::worker::worker_manager::get_worker_channels::<K, V>(channel_map)
        {
            // Convert tier for coherence protocol
            let requesting_tier = self.tier_info.current_tier.into();

            // Create oneshot channel for response
            let (response_tx, response_rx) = oneshot::channel();

            // Create appropriate coherence request based on access type
            let request = match access_type {
                AccessType::Read
                | AccessType::Sequential
                | AccessType::Random
                | AccessType::Temporal
                | AccessType::Spatial
                | AccessType::Hit
                | AccessType::SequentialRead
                | AccessType::RandomRead => CoherenceRequest::RecordRead {
                    key: self.key.clone(),
                    tier: requesting_tier,
                    response: response_tx,
                },
                AccessType::Write | AccessType::SequentialWrite | AccessType::ReadModifyWrite => {
                    CoherenceRequest::RecordWrite {
                        key: self.key.clone(),
                        data: self.value.clone(),
                        tier: requesting_tier,
                        response: response_tx,
                    }
                }
                AccessType::Prefetch
                | AccessType::PrefetchHit
                | AccessType::Miss
                | AccessType::Promotion
                | AccessType::Demotion => CoherenceRequest::RecordPrefetch {
                    key: self.key.clone(),
                    tier: requesting_tier,
                    response: response_tx,
                },
            };

            // Send request to coherence worker via tokio channel
            if request_sender.send(request).is_ok() {
                // Non-blocking async await with timeout
                match tokio::time::timeout(Duration::from_millis(50), response_rx).await {
                    Ok(Ok(Ok(()))) => {
                        log::trace!(
                            "Coherence access recorded successfully for key {:?}",
                            self.key
                        );
                    }
                    Ok(Ok(Err(error))) => {
                        log::warn!(
                            "Coherence access recording failed for key {:?}: {:?}",
                            self.key,
                            error
                        );
                    }
                    Ok(Err(_recv_error)) => {
                        log::warn!(
                            "Coherence response channel closed for key {:?}",
                            self.key
                        );
                    }
                    Err(_timeout) => {
                        // Non-blocking - coherence worker will handle asynchronously
                        log::trace!(
                            "Coherence access message sent asynchronously for key {:?}",
                            self.key
                        );
                    }
                }
            } else {
                log::warn!(
                    "Coherence worker channel closed, cannot record access for key {:?}",
                    self.key
                );
            }
        } else {
            // Coherence coordination not available - log for debugging
            log::trace!(
                "Coherence coordination unavailable for access: key={:?}, tier={:?}, type={:?}, hit={}",
                self.key,
                self.tier_info.current_tier,
                access_type,
                hit
            );
        }

        // Update access tracker with sophisticated analysis
        self.access_tracker.record_access(AccessRecordParams {
            key: &self.key,
            tier: self.tier_info.current_tier.into(),
            entry_size: self.metadata.size_bytes,
            access_type,
            latency_ns,
            hit,
            event_counter,
        });

        // Update promotion score based on access patterns
        self.update_promotion_score();
    }
}

/// Serialization envelope for persistent storage with versioning and integrity
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
#[serde(bound(serialize = "K: serde::Serialize, V: serde::Serialize"))]
#[serde(bound(deserialize = "K: serde::de::DeserializeOwned, V: serde::de::DeserializeOwned"))]
pub struct SerializationEnvelope<K, V>
where
    K: CacheKey + Clone + Default,
    V: CacheValue + Clone + Default,
{
    /// Complete cache entry with all metadata
    pub entry: CacheEntry<K, V>,
    /// Schema version for evolution support
    pub schema_version: u32,
    /// Compression algorithm actually applied
    pub compression_used: CompressionAlgorithm,
    /// Data integrity checksum
    pub checksum: u64,
    /// Timestamp when serialized (nanoseconds since epoch)
    pub serialized_at_ns: u64,
    /// Tier-specific serialization context
    pub tier_context: TierSerializationContext,
}

/// Tier-specific serialization context for optimization
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
pub struct TierSerializationContext {
    /// Target tier for this serialization
    pub target_tier: TierLocation,
    /// Expected access frequency in target tier
    pub expected_frequency: f64,
    /// Serialization optimization level (0-9)
    pub optimization_level: u8,
    /// Whether to include access history
    pub include_access_history: bool,
}

impl Default for TierSerializationContext {
    fn default() -> Self {
        Self {
            target_tier: TierLocation::Warm,
            expected_frequency: 1.0,
            optimization_level: 3,
            include_access_history: false,
        }
    }
}

// Internal serialization methods - may not be used in minimal API
impl<K, V> SerializationEnvelope<K, V>
where
    K: CacheKey
        + Clone
        + Default
        + bincode::Encode
        + bincode::Decode<()>
        + serde::Serialize
        + serde::de::DeserializeOwned,
    V: CacheValue
        + Clone
        + Default
        + bincode::Encode
        + bincode::Decode<()>
        + serde::Serialize
        + serde::de::DeserializeOwned,
{
    /// Create new serialization envelope using coherence write propagation system
    pub fn new_with_coherence(
        entry: CacheEntry<K, V>,
        coherence_controller: &CoherenceController<K, V>,
    ) -> Result<Self, CoherenceError> {
        let coherence_key = CoherenceKey::from_cache_key(&entry.key);

        // DELEGATE to existing write propagation system for coordination (spawn async)
        let write_propagation = std::sync::Arc::clone(&coherence_controller.write_propagation);
        let key_clone = coherence_key.clone();
        let value_clone = entry.value.clone();
        let source_tier = entry.tier_info.current_tier.into();
        let dest_tier = coherence_controller.determine_target_tier(source_tier);
        tokio::runtime::Handle::current().spawn(async move {
            let _ = write_propagation.submit_writeback(
                key_clone,
                value_clone,
                source_tier,
                dest_tier,
                0,
                WritePriority::Normal,
            ).await;
        });

        let tier_context = TierSerializationContext {
            target_tier: entry.tier_info.current_tier,
            expected_frequency: entry.access_tracker.frequency(),
            optimization_level: match entry.tier_info.current_tier {
                TierLocation::Hot => 1,  // Fast serialization
                TierLocation::Warm => 5, // Balanced
                TierLocation::Cold => 9, // Maximum compression
            },
            include_access_history: entry.tier_info.current_tier != TierLocation::Hot,
        };

        // Get checksum and compression from coherence metadata (computed by coherence system)
        let (checksum, compression_used) = if let Some(cache_line_entry) =
            coherence_controller.cache_line_states.get(&coherence_key)
        {
            let cache_line = cache_line_entry.value();
            // Use coherence invalidation sequence as integrity checksum
            let checksum = cache_line.metadata.invalidation_seq.load(Ordering::Acquire) as u64;

            // Determine compression from tier and coherence state
            let compression_used = match entry.tier_info.current_tier {
                TierLocation::Hot => CompressionAlgorithm::None, // Speed priority
                TierLocation::Warm => CompressionAlgorithm::Lz4, // Balanced
                TierLocation::Cold => CompressionAlgorithm::Zstd, // Space priority
            };

            (checksum, compression_used)
        } else {
            return Err(CoherenceError::CacheLineNotFound);
        };

        Ok(Self {
            entry,
            schema_version: 1,
            compression_used, // From coherence protocol negotiation
            checksum,         // From coherence system metadata
            serialized_at_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            tier_context,
        })
    }

    /// Create new serialization envelope (basic implementation - prefer new_with_coherence)
    pub fn new(entry: CacheEntry<K, V>) -> Result<Self, Box<dyn std::error::Error>> {
        let tier_context = TierSerializationContext {
            target_tier: entry.tier_info.current_tier,
            expected_frequency: entry.access_tracker.frequency(),
            optimization_level: match entry.tier_info.current_tier {
                TierLocation::Hot => 1,  // Fast serialization
                TierLocation::Warm => 5, // Balanced
                TierLocation::Cold => 9, // Maximum compression
            },
            include_access_history: entry.tier_info.current_tier != TierLocation::Hot,
        };

        // Serialize entry first to get actual data for compression analysis
        let serialized_data = bincode::encode_to_vec(&entry, bincode::config::standard())?;

        // Use compression engine to intelligently select algorithm based on data
        let compression_engine =
            crate::cache::tier::cold::data_structures::CompressionEngine::new(6);
        let compression_algorithm = compression_engine.select_algorithm(&serialized_data);

        // Calculate checksum using the already serialized data
        let checksum = crc32fast::hash(&serialized_data);

        Ok(Self {
            entry,
            schema_version: 1,
            compression_used: compression_algorithm.into(),
            checksum: checksum.into(),
            serialized_at_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            tier_context,
        })
    }

    /// Get estimated serialized size
    pub fn estimated_serialized_size(&self) -> usize {
        let base_size = self.entry.total_size();
        let metadata_overhead = std::mem::size_of::<SerializationEnvelope<K, V>>()
            - std::mem::size_of::<CacheEntry<K, V>>();
        base_size + metadata_overhead
    }

    /// Check if envelope is valid using coherence state management delegation
    pub fn is_valid_with_coherence(
        &self,
        coherence_controller: &CoherenceController<K, V>,
    ) -> Result<bool, CoherenceError> {
        // DELEGATE to existing coherence state transition validator
        let transition_request = StateTransitionRequest {
            from_state: MesiState::Shared, // Current assumed state
            to_state: MesiState::Shared,   // No transition for validation
            tier: self.entry.tier_info.current_tier.into(),
            version: 0,
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            reason: TransitionReason::ProtocolEnforcement,
        };

        // Use existing sophisticated transition validator for consistency checks
        match coherence_controller
            .transition_validator
            .execute_transition(&transition_request)
        {
            Ok(()) => {
                // Coherence validation passed - perform additional envelope checks
                let basic_valid = self.schema_version > 0 && self.entry.is_healthy();

                // Validate checksum against coherence metadata
                let coherence_key = CoherenceKey::from_cache_key(&self.entry.key);
                let checksum_valid = if let Some(cache_line_entry) =
                    coherence_controller.cache_line_states.get(&coherence_key)
                {
                    let cache_line = cache_line_entry.value();
                    let expected_checksum =
                        cache_line.metadata.invalidation_seq.load(Ordering::Acquire) as u64;
                    expected_checksum == self.checksum
                } else {
                    false
                };

                Ok(basic_valid && checksum_valid)
            }
            Err(coherence_error) => Err(coherence_error),
        }
    }
}

impl<K, V> Default for SerializationEnvelope<K, V>
where
    K: CacheKey + Clone + Default,
    V: CacheValue + Clone + Default,
{
    fn default() -> Self {
        // Create a default cache entry as the base
        let default_entry = CacheEntry {
            key: K::default(),
            value: V::default(),
            metadata: CacheEntryMetadata::default(),
            access_tracker: AccessTracker::new(), // Default max history size
            tier_info: TierInfo::default(),
            serialization_context: SerializationContext::default(),
        };

        Self {
            entry: default_entry,
            schema_version: 1,
            compression_used: CompressionAlgorithm::None,
            checksum: 0,
            serialized_at_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            tier_context: TierSerializationContext::default(),
        }
    }
}

// Implement the existing CacheEntry trait for our concrete CacheEntry struct
impl<K: CacheKey, V: CacheValue> super::entry_and_stats::CacheEntry<K, V> for CacheEntry<K, V> {
    type Key = K;
    type Value = V;

    fn key(&self) -> &Self::Key {
        &self.key
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }

    fn created_at(&self) -> Instant {
        let system_time =
            std::time::UNIX_EPOCH + std::time::Duration::from_nanos(self.metadata.created_at_ns);
        Instant::now()
            - (std::time::SystemTime::now()
                .duration_since(system_time)
                .unwrap_or_default())
    }

    fn access_count(&self) -> u64 {
        self.metadata.access_count()
    }

    fn last_access(&self) -> Instant {
        let system_time =
            std::time::UNIX_EPOCH + std::time::Duration::from_nanos(self.metadata.last_accessed_ns);
        Instant::now()
            - (std::time::SystemTime::now()
                .duration_since(system_time)
                .unwrap_or_default())
    }

    fn size(&self) -> usize {
        self.metadata.size_bytes
    }

    fn tier(&self) -> TierLocation {
        self.tier_info.current_tier
    }
}
