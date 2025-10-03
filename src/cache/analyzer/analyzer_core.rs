//! Core access pattern analyzer implementation
//!
//! This module provides the main AccessPatternAnalyzer struct with lock-free
//! concurrent tracking and pattern analysis capabilities.

// Internal analyzer architecture - components may not be used in minimal API

use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;

use std::collections::VecDeque;
use std::time::Duration;

use super::access_record::AccessRecord;
use super::types::{AccessPattern, AccessPatternType, AnalyzerError, AnalyzerStatistics};
use crate::cache::config::types::AnalyzerConfig;
use crate::cache::traits::core::CacheKey;
use crate::cache::traits::types_and_enums::CacheOperationError;

// HIGHLANDER CANONICALIZATION: Use canonical PatternDetector directly
use crate::cache::tier::hot::prefetch::pattern_detection::PatternDetector;
use crate::cache::tier::hot::prefetch::types::{
    AccessPattern as PrefetchAccessPattern, AccessSequence, PrefetchConfig,
};

/// Access pattern analyzer with lock-free concurrent tracking
#[derive(Debug)]
pub struct AccessPatternAnalyzer<K: CacheKey> {
    /// Map of access records indexed by cache key
    pub access_history: DashMap<K, AccessRecord>,
    /// Global logical clock for ordering
    pub global_clock: AtomicU64,
    /// Cleanup operation counter
    pub cleanup_counter: AtomicU64,
    /// Configuration parameters
    pub config: AnalyzerConfig,
    /// Pattern analysis window for nearby key tracking
    pub pattern_window: DashMap<u64, Vec<(u64, AccessPatternType)>>, /* hash -> (timestamp, pattern) */
    /// Pattern detection engine
    pub pattern_detector: PatternDetector,
}

impl<K: CacheKey> AccessPatternAnalyzer<K> {
    /// Create new access pattern analyzer with configuration
    pub fn new(config: AnalyzerConfig) -> Result<Self, AnalyzerError> {
        // Validate configuration
        Self::validate_config(&config)?;

        // HIGHLANDER: Convert AnalyzerConfig to PrefetchConfig for canonical detector
        let prefetch_config = PrefetchConfig {
            history_size: config.max_tracked_keys.min(1000),
            max_prefetch_distance: 5, // Reasonable default
            min_confidence_threshold: 0.7,
            pattern_detection_window: Duration::from_nanos(config.time_bucket_duration_ns * 10),
            max_patterns: config.max_tracked_keys.min(100),
            prefetch_queue_size: 50, // Reasonable default
        };

        Ok(Self {
            access_history: DashMap::with_capacity(config.max_tracked_keys),
            global_clock: AtomicU64::new(1),
            cleanup_counter: AtomicU64::new(0),
            pattern_detector: PatternDetector::new(prefetch_config),
            config,
            pattern_window: DashMap::new(),
        })
    }

    /// Validate analyzer configuration
    fn validate_config(config: &AnalyzerConfig) -> Result<(), AnalyzerError> {
        if config.max_tracked_keys == 0 {
            return Err(AnalyzerError::InvalidConfiguration(
                "max_tracked_keys must be > 0".to_string(),
            ));
        }
        if config.frequency_decay_constant <= 0.0 {
            return Err(AnalyzerError::InvalidConfiguration(
                "frequency_decay_constant must be > 0".to_string(),
            ));
        }
        if config.recency_half_life <= 0.0 {
            return Err(AnalyzerError::InvalidConfiguration(
                "recency_half_life must be > 0".to_string(),
            ));
        }
        if config.time_bucket_count == 0 {
            return Err(AnalyzerError::InvalidConfiguration(
                "time_bucket_count must be > 0".to_string(),
            ));
        }
        if config.time_bucket_duration_ns == 0 {
            return Err(AnalyzerError::InvalidConfiguration(
                "time_bucket_duration_ns must be > 0".to_string(),
            ));
        }
        Ok(())
    }

    /// Record cache access for pattern analysis
    #[inline]
    pub fn record_access(&self, key: &K) -> Result<(), CacheOperationError> {
        let now_ns = self.current_timestamp_ns()?;
        let bucket_index = self.time_bucket_index(now_ns);

        // Get existing or create new record
        match self.access_history.get(key) {
            Some(existing) => {
                // Update existing record atomically
                existing.record_access(now_ns, bucket_index);
            }
            None => {
                // Check if we're at capacity
                if self.access_history.len() >= self.config.max_tracked_keys {
                    self.evict_oldest_entry()?;
                }

                // Create new record
                let new_record = AccessRecord::new(now_ns, self.config.time_bucket_count);

                // Insert or update if racing
                self.access_history
                    .entry(key.clone())
                    .and_modify(|existing| existing.record_access(now_ns, bucket_index))
                    .or_insert(new_record);
            }
        }

        // Update pattern analysis window
        self.update_pattern_window(key, now_ns)?;

        // Maybe perform cleanup
        self.maybe_cleanup()?;

        Ok(())
    }

    /// Record cache miss for pattern analysis
    pub fn record_miss(&self, key: &K) -> Result<(), CacheOperationError> {
        // Misses are recorded the same way as accesses for pattern analysis
        self.record_access(key)
    }

    /// Analyze access pattern for cache key
    #[inline]
    pub fn analyze_access_pattern(&self, key: &K) -> AccessPattern {
        let now_ns = match self.current_timestamp_ns() {
            Ok(ts) => ts,
            Err(_) => return Self::default_pattern(),
        };

        match self.access_history.get(key) {
            Some(record) => self.compute_pattern(key, &record, now_ns),
            None => Self::default_pattern(),
        }
    }

    #[inline(always)]
    fn compute_pattern(&self, key: &K, record: &AccessRecord, now_ns: u64) -> AccessPattern {
        let frequency = self.calculate_frequency(record, now_ns);
        let recency = self.calculate_recency(record, now_ns);

        // Use cached pattern hint if available for performance, otherwise detect fresh
        let cached_pattern = record.pattern_hint();
        let pattern_type = if record.total_accesses() > 10
            && matches!(
                cached_pattern,
                AccessPatternType::Sequential
                    | AccessPatternType::Temporal
                    | AccessPatternType::Spatial
            ) {
            // Trust established patterns for frequently accessed keys
            cached_pattern
        } else {
            // Fresh detection for new keys or random patterns
            let detected_pattern = self.detect_pattern_type_canonical(key, now_ns);
            record.update_pattern_hint(detected_pattern);
            detected_pattern
        };

        // Consider entry age in pattern analysis (using creation time)
        let age_ns = now_ns.saturating_sub(record.creation_ns());
        let age_factor = if age_ns > 0 {
            // Adjust frequency based on age - older entries should show more established patterns
            1.0 + (age_ns as f64 / 1_000_000_000.0).ln().max(0.0) * 0.1
        } else {
            1.0
        };

        // Incorporate memory usage into temporal locality calculation
        let memory_adjusted_locality = {
            let base_locality = self.calculate_temporal_locality(record, now_ns);
            let memory_factor = record.memory_usage() as f64 / 1024.0; // KB scale
            base_locality * (1.0 + memory_factor * 0.01) // Small adjustment based on memory usage
        };

        AccessPattern {
            frequency: frequency * age_factor,
            recency,
            temporal_locality: memory_adjusted_locality,
            pattern_type,
        }
    }

    /// HIGHLANDER: Direct pattern detection using canonical implementation
    fn detect_pattern_type_canonical(&self, key: &K, now_ns: u64) -> AccessPatternType {
        let key_hash = key.cache_hash();

        // Get nearby key access history from DashMap for pattern analysis
        let nearby_accesses = self.get_nearby_key_accesses(key_hash, now_ns);

        if nearby_accesses.len() < 3 {
            return AccessPatternType::Random; // Not enough data
        }

        // Convert DashMap format to canonical detector's expected format
        let mut access_history = VecDeque::new();
        for (timestamp, _pattern_type) in &nearby_accesses {
            access_history.push_back(AccessSequence {
                key: key.clone(),
                timestamp: *timestamp,
                context_hash: key_hash,
            });
        }

        // Use canonical detector for sophisticated analysis
        match self.pattern_detector.detect_patterns(&access_history) {
            Ok(patterns) => {
                // Find highest confidence pattern and map to AccessPatternType
                if let Some(best_pattern) = patterns.iter().max_by(|a, b| {
                    a.confidence
                        .partial_cmp(&b.confidence)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }) {
                    match best_pattern.pattern_type {
                        PrefetchAccessPattern::Sequential => AccessPatternType::Sequential,
                        PrefetchAccessPattern::Temporal => AccessPatternType::Temporal,
                        PrefetchAccessPattern::Spatial => AccessPatternType::Spatial,
                        PrefetchAccessPattern::Periodic => AccessPatternType::Temporal, // Map periodic to temporal
                        PrefetchAccessPattern::Contextual => AccessPatternType::Spatial, // Map contextual to spatial
                        PrefetchAccessPattern::Random => AccessPatternType::Random,
                    }
                } else {
                    AccessPatternType::Random
                }
            }
            Err(_) => {
                // Fallback to simple analysis if canonical detector fails
                self.fallback_pattern_analysis(&nearby_accesses, key_hash, now_ns)
            }
        }
    }

    /// Get nearby key access patterns for analysis (using pattern_window)
    fn get_nearby_key_accesses(&self, key_hash: u64, now_ns: u64) -> Vec<(u64, AccessPatternType)> {
        let window_key = key_hash >> 20; // Group by high-order bits

        if let Some(entry) = self.pattern_window.get(&window_key) {
            // Filter recent accesses within analysis window
            let cutoff_time = now_ns.saturating_sub(self.config.time_bucket_duration_ns * 10);
            entry
                .value()
                .iter()
                .filter(|(timestamp, _)| *timestamp >= cutoff_time)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Fallback pattern analysis using simple algorithms
    fn fallback_pattern_analysis(
        &self,
        accesses: &[(u64, AccessPatternType)],
        base_hash: u64,
        now_ns: u64,
    ) -> AccessPatternType {
        if self.is_sequential_pattern_fallback(accesses) {
            AccessPatternType::Sequential
        } else if self.is_temporal_pattern_fallback(accesses, now_ns) {
            AccessPatternType::Temporal
        } else if self.is_spatial_pattern_fallback(accesses, base_hash) {
            AccessPatternType::Spatial
        } else {
            AccessPatternType::Random
        }
    }

    /// Fallback sequential pattern detection
    fn is_sequential_pattern_fallback(&self, accesses: &[(u64, AccessPatternType)]) -> bool {
        if accesses.len() < 3 {
            return false;
        }

        // Look for monotonically increasing hash values
        let mut increasing_count = 0;
        for window in accesses.windows(2) {
            if window[1].0 > window[0].0 {
                increasing_count += 1;
            }
        }

        // Consider sequential if >70% of accesses are in increasing order
        let sequential_ratio = increasing_count as f64 / (accesses.len() - 1) as f64;
        sequential_ratio > 0.7
    }

    /// Fallback temporal pattern detection
    fn is_temporal_pattern_fallback(
        &self,
        accesses: &[(u64, AccessPatternType)],
        _now_ns: u64,
    ) -> bool {
        if accesses.len() < 4 {
            return false;
        }

        // Calculate intervals between accesses
        let mut intervals = Vec::new();
        for window in accesses.windows(2) {
            let interval = window[1].0.saturating_sub(window[0].0);
            if interval > 0 {
                intervals.push(interval);
            }
        }

        if intervals.len() < 3 {
            return false;
        }

        // Calculate coefficient of variation for intervals
        let mean_interval: f64 =
            intervals.iter().map(|&x| x as f64).sum::<f64>() / intervals.len() as f64;
        if mean_interval == 0.0 {
            return false;
        }

        let variance: f64 = intervals
            .iter()
            .map(|&x| (x as f64 - mean_interval).powi(2))
            .sum::<f64>()
            / intervals.len() as f64;
        let std_dev = variance.sqrt();
        let cv = std_dev / mean_interval;

        // Regular temporal pattern has low coefficient of variation
        cv < 0.3
    }

    /// Fallback spatial pattern detection
    fn is_spatial_pattern_fallback(
        &self,
        accesses: &[(u64, AccessPatternType)],
        base_hash: u64,
    ) -> bool {
        if accesses.len() < 3 {
            return false;
        }

        // Calculate how many accesses are within spatial locality range
        let locality_threshold = 0x1000000; // Hash distance threshold
        let nearby_count = accesses
            .iter()
            .filter(|(hash, _)| hash.abs_diff(base_hash) < locality_threshold)
            .count();

        // Consider spatial if >60% of accesses are nearby
        let spatial_ratio = nearby_count as f64 / accesses.len() as f64;
        spatial_ratio > 0.6
    }

    /// Calculate access frequency with exponential decay
    #[inline(always)]
    fn calculate_frequency(&self, record: &AccessRecord, now_ns: u64) -> f64 {
        let total_accesses = record.total_accesses();
        let last_access_ns = record.last_access_ns();

        if total_accesses == 0 {
            return 0.0;
        }

        // Exponential decay: frequency = accesses * e^(-λt)
        let time_since_last = now_ns.saturating_sub(last_access_ns) as f64;
        let decay_factor = (-time_since_last / self.config.frequency_decay_constant).exp();

        // Also consider sliding window frequency from time buckets
        let window_frequency = self.calculate_window_frequency(record, now_ns);

        // Combine long-term (with decay) and short-term (window) frequencies
        let long_term_frequency = (total_accesses as f64) * decay_factor;
        let combined_frequency = (long_term_frequency * 0.7) + (window_frequency * 0.3);

        combined_frequency.max(0.0)
    }

    /// Calculate sliding window frequency from time buckets
    #[inline(always)]
    fn calculate_window_frequency(&self, record: &AccessRecord, now_ns: u64) -> f64 {
        let mut window_accesses = 0;
        let current_bucket = self.time_bucket_index(now_ns);

        // Sum accesses from recent time buckets (sliding window)
        let window_size = (self.config.time_bucket_count / 2).min(10); // Use up to 10 recent buckets
        for i in 0..window_size {
            let bucket_idx = (current_bucket + self.config.time_bucket_count - i)
                % self.config.time_bucket_count;
            window_accesses += record.access_buckets()[bucket_idx].load(Ordering::Relaxed);
        }

        // Calculate frequency per second
        let window_duration_s =
            (window_size as f64) * (self.config.time_bucket_duration_ns as f64) / 1_000_000_000.0;
        if window_duration_s > 0.0 {
            (window_accesses as f64) / window_duration_s
        } else {
            0.0
        }
    }

    /// Calculate access recency with configurable half-life decay  
    #[inline(always)]
    fn calculate_recency(&self, record: &AccessRecord, now_ns: u64) -> f64 {
        let last_access_ns = record.last_access_ns();
        let time_since = now_ns.saturating_sub(last_access_ns) as f64;

        if time_since < 0.0 {
            return 1.0; // Future timestamp, consider as recent
        }

        // Recency score: 1.0 = just accessed, approaches 0.0 over time
        // Uses exponential decay: recency = e^(-t * ln(2) / half_life)
        let decay_rate = std::f64::consts::LN_2 / self.config.recency_half_life;
        let recency = (-time_since * decay_rate).exp();

        recency.clamp(0.0, 1.0)
    }

    /// Calculate temporal locality score based on access pattern regularity
    #[inline(always)]
    fn calculate_temporal_locality(&self, record: &AccessRecord, _now_ns: u64) -> f64 {
        let total_accesses = record.total_accesses();
        if total_accesses < 2 {
            return 0.5; // Not enough data for pattern detection
        }

        // Analyze time bucket distribution for temporal patterns
        let mut bucket_variance = 0.0;
        let mut active_buckets = 0;
        let expected_per_bucket = total_accesses as f64 / self.config.time_bucket_count as f64;

        for bucket in record.access_buckets().iter() {
            let count = bucket.load(Ordering::Relaxed) as f64;
            if count > 0.0 {
                active_buckets += 1;
                bucket_variance += (count - expected_per_bucket).powi(2);
            }
        }

        if active_buckets == 0 {
            return 0.5;
        }

        // Calculate coefficient of variation for temporal distribution
        let mean_bucket_access = total_accesses as f64 / active_buckets as f64;
        let std_dev = (bucket_variance / active_buckets as f64).sqrt();
        let coefficient_of_variation = if mean_bucket_access > 0.0 {
            std_dev / mean_bucket_access
        } else {
            1.0
        };

        // Convert to locality score: low variation = high temporal locality
        // High coefficient of variation = random/scattered access = low temporal locality
        let locality_score = 1.0 / (1.0 + coefficient_of_variation);
        locality_score.clamp(0.0, 1.0)
    }

    /// Get default access pattern for untracked keys
    fn default_pattern() -> AccessPattern {
        AccessPattern {
            frequency: 1.0,
            recency: 0.5,
            temporal_locality: 0.5, // Default neutral temporal locality
            pattern_type: AccessPatternType::Random,
        }
    }

    /// Get analyzer statistics
    pub fn stats(&self) -> AnalyzerStatistics {
        AnalyzerStatistics {
            tracked_keys: self.access_history.len() as u64,
            total_cleanup_cycles: self.cleanup_counter.load(Ordering::Relaxed)
                / self.config.cleanup_interval,
            memory_usage_bytes: self.estimate_memory_usage(),
        }
    }

    // HIGHLANDER: Methods moved from utilities.rs with canonical pattern detection

    /// Update pattern analysis window with new access (CANONICAL IMPLEMENTATION)
    fn update_pattern_window(&self, key: &K, now_ns: u64) -> Result<(), CacheOperationError> {
        let key_hash = self.hash_key(key);
        let window_key = key_hash >> 20; // Group by high-order bits

        // HIGHLANDER: Use canonical pattern detection
        let pattern_type = self.detect_pattern_type_canonical(key, now_ns);

        // Add to pattern window
        match self.pattern_window.entry(window_key) {
            dashmap::mapref::entry::Entry::Occupied(mut entry) => {
                let accesses = entry.get_mut();
                accesses.push((key_hash, pattern_type));

                // Keep window bounded
                if accesses.len() > self.config.pattern_analysis_window {
                    accesses.remove(0);
                }
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                entry.insert(vec![(key_hash, pattern_type)]);
            }
        }

        Ok(())
    }

    /// Hash cache key for pattern analysis
    #[inline(always)]
    fn hash_key(&self, key: &K) -> u64 {
        // Use generic CacheKey hash method
        key.cache_hash()
    }

    /// Calculate time bucket index for sliding window
    #[inline(always)]
    fn time_bucket_index(&self, timestamp_ns: u64) -> usize {
        ((timestamp_ns / self.config.time_bucket_duration_ns)
            % self.config.time_bucket_count as u64) as usize
    }

    /// Get current timestamp in nanoseconds
    #[inline(always)]
    fn current_timestamp_ns(&self) -> Result<u64, CacheOperationError> {
        use crate::cache::types::performance::timer::timestamp_nanos;
        
        // Use absolute UNIX epoch timestamp for wall time
        let wall_time_ns = timestamp_nanos(std::time::Instant::now());
        
        // Increment per-instance logical clock for consistent ordering
        let logical_clock = self.global_clock.fetch_add(1, Ordering::SeqCst);
        
        // Combine wall clock time with logical clock for hybrid timestamp
        // High-order bits: wall time, low-order bits: logical sequence
        let combined_timestamp = wall_time_ns.wrapping_add(logical_clock & 0xFFFF);
        
        Ok(combined_timestamp)
    }

    /// Maybe perform periodic cleanup
    fn maybe_cleanup(&self) -> Result<(), CacheOperationError> {
        let cleanup_counter = self.cleanup_counter.fetch_add(1, Ordering::Relaxed);

        // Cleanup every N operations
        if cleanup_counter.is_multiple_of(self.config.cleanup_interval) {
            self.perform_cleanup()?;
        }

        Ok(())
    }

    /// Perform cleanup of expired entries
    fn perform_cleanup(&self) -> Result<(), CacheOperationError> {
        let now_ns = self.current_timestamp_ns()?;
        let cutoff_time = now_ns.saturating_sub(self.config.cleanup_age_threshold_ns);

        // Remove entries older than threshold
        self.access_history
            .retain(|_key, record| record.last_access_ns() > cutoff_time);

        // Clean pattern window
        self.pattern_window.retain(|_window_key, accesses| {
            accesses.retain(|(timestamp, _)| *timestamp > cutoff_time);
            !accesses.is_empty()
        });

        Ok(())
    }

    /// Evict oldest entry when at capacity (LRU eviction)
    fn evict_oldest_entry(&self) -> Result<(), CacheOperationError> {
        let mut oldest_key: Option<K> = None;
        let mut oldest_time = u64::MAX;

        // Find oldest entry (this is O(n) but only done at capacity)
        for entry in self.access_history.iter() {
            let last_access = entry.value().last_access_ns();
            if last_access < oldest_time {
                oldest_time = last_access;
                oldest_key = Some(entry.key().clone());
            }
        }

        // Remove oldest entry
        if let Some(key) = oldest_key {
            self.access_history.remove(&key);
        }

        Ok(())
    }

    /// Estimate current memory usage
    fn estimate_memory_usage(&self) -> u64 {
        let base_size = std::mem::size_of::<Self>() as u64;
        let map_overhead = self.access_history.len() as u64 * 64; // Estimate per-entry overhead
        let record_size = self
            .access_history
            .iter()
            .map(|entry| entry.value().memory_usage() as u64)
            .sum::<u64>();

        base_size + map_overhead + record_size
    }
}
