//! Core access pattern analyzer implementation
//!
//! This module provides the main AccessPatternAnalyzer struct with lock-free
//! concurrent tracking and pattern analysis capabilities.

use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;

use super::access_record::AccessRecord;
use super::pattern_detection::PatternDetector;
use super::types::{AccessPattern, AccessPatternType, AnalyzerError, AnalyzerStatistics};
use crate::cache::config::types::AnalyzerConfig;
use crate::cache::traits::core::CacheKey;
use crate::cache::traits::types_and_enums::CacheOperationError;

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

        Ok(Self {
            access_history: DashMap::with_capacity(config.max_tracked_keys),
            global_clock: AtomicU64::new(1),
            cleanup_counter: AtomicU64::new(0),
            pattern_detector: PatternDetector::new(&config),
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
            Some(record) => self.compute_pattern(&*record, now_ns),
            None => return Self::default_pattern(),
        }
    }

    #[inline(always)]
    fn compute_pattern(&self, record: &AccessRecord, now_ns: u64) -> AccessPattern {

        let frequency = self.calculate_frequency(&record, now_ns);
        let recency = self.calculate_recency(&record, now_ns);
        let pattern_type =
            self.pattern_detector
                .detect_pattern_type(key, now_ns, &self.pattern_window);

        AccessPattern {
            frequency,
            recency,
            temporal_locality: self.calculate_temporal_locality(&record, now_ns),
            pattern_type,
        }
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

        recency.max(0.0).min(1.0)
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
        locality_score.max(0.0).min(1.0)
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
}
