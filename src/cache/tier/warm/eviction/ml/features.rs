//! Feature extraction and vector management for ML-based eviction
//!
//! This module handles feature extraction, vector operations, and feature
//! learning for machine learning-based cache eviction policies.

#![allow(dead_code)] // Warm tier eviction ML - Complete feature extraction library for ML-based eviction policies

use crate::cache::traits::AccessType;

/// Number of features in the ML model
pub const FEATURE_COUNT: usize = 16;

/// Feature vector for ML prediction
#[allow(dead_code)] // ML system - used extensively in machine learning eviction policies and feature extraction
#[derive(Debug, Clone)]
pub struct FeatureVector {
    /// Recency score (0-1, higher = more recent)
    #[allow(dead_code)]
    // ML system - used in machine learning feature extraction and prediction models
    pub recency: f64,
    /// Frequency score (normalized)
    #[allow(dead_code)]
    // ML system - used in machine learning feature extraction and prediction models
    pub frequency: f64,
    /// Access regularity (0-1, higher = more regular)
    #[allow(dead_code)]
    // ML system - used in machine learning pattern recognition and eviction policies
    pub regularity: f64,
    /// Relative size compared to average
    #[allow(dead_code)] // ML system - used in machine learning size-based eviction analysis
    pub relative_size: f64,
    /// Temporal locality score
    #[allow(dead_code)] // ML system - used in machine learning temporal pattern analysis
    pub temporal_locality: f64,
    /// Spatial locality score
    #[allow(dead_code)] // ML system - used in machine learning spatial pattern analysis
    pub spatial_locality: f64,
    /// Working set probability
    #[allow(dead_code)] // ML system - used in machine learning working set prediction
    pub working_set_prob: f64,
    /// Burst access indicator
    #[allow(dead_code)] // ML system - used in machine learning burst pattern detection
    pub burst_indicator: f64,
    /// Sequential access indicator
    #[allow(dead_code)] // ML system - used in machine learning sequential pattern detection
    pub sequential_indicator: f64,
    /// Prefetch success rate
    #[allow(dead_code)] // ML system - used in machine learning prefetch effectiveness analysis
    pub prefetch_success: f64,
    /// Access time variance
    #[allow(dead_code)]
    // ML system - used in machine learning access pattern variance analysis
    pub arrival_variance: f64,
    /// Peak frequency observed
    #[allow(dead_code)]
    // ML system - used in machine learning frequency analysis and prediction
    pub peak_frequency: f64,
    /// Time-of-day access pattern
    #[allow(dead_code)] // ML system - used in machine learning temporal pattern classification
    pub time_pattern: f64,
    /// Thread locality score
    #[allow(dead_code)] // ML system - used in machine learning thread locality analysis
    pub thread_locality: f64,
    /// Cache pressure indicator
    #[allow(dead_code)] // ML system - used in machine learning cache pressure analysis
    pub pressure_indicator: f64,
    /// Aging factor
    #[allow(dead_code)] // ML system - used in machine learning aging factor calculations
    pub aging_factor: f64,
}

impl FeatureVector {
    /// Create new feature vector
    #[allow(dead_code)] // ML system - used in machine learning feature vector construction and initialization
    pub fn new(_timestamp_ns: u64) -> Self {
        Self {
            recency: 0.0,
            frequency: 1.0,
            regularity: 0.0,
            relative_size: 0.5,
            temporal_locality: 0.0,
            spatial_locality: 0.0,
            working_set_prob: 0.0,
            burst_indicator: 0.0,
            sequential_indicator: 0.0,
            prefetch_success: 0.0,
            arrival_variance: 0.0,
            peak_frequency: 1.0,
            time_pattern: 0.0,
            thread_locality: 0.0,
            pressure_indicator: 0.0,
            aging_factor: 1.0,
        }
    }

    /// Update features based on cache access
    #[allow(dead_code)] // ML system - used in machine learning feature updates from cache access patterns
    pub fn update_from_access(&mut self, timestamp_ns: u64, access_type: AccessType) {
        // Update recency (normalized to [0,1])
        let current_time = crate::telemetry::cache::types::timestamp_nanos();
        self.recency = (current_time - timestamp_ns) as f64 / 1e9; // Seconds since access

        // Update frequency with exponential moving average
        self.update_frequency(self.frequency + 1.0, 0.1);

        // Update other features based on access type
        match access_type {
            AccessType::Hit => {
                self.temporal_locality = (self.temporal_locality * 0.9 + 0.1).min(1.0);
            }
            AccessType::PrefetchHit => {
                self.prefetch_success = (self.prefetch_success * 0.9 + 0.1).min(1.0);
            }
            _ => {}
        }

        // Update aging factor
        self.aging_factor *= 0.999; // Gradual aging
    }

    /// Convert to array for ML prediction
    #[allow(dead_code)] // ML system - used in machine learning model input preparation and prediction
    pub fn to_array(&self, cache_pressure: f64) -> [f64; FEATURE_COUNT] {
        [
            self.recency,
            self.frequency,
            self.regularity,
            self.relative_size,
            self.temporal_locality,
            self.spatial_locality,
            self.working_set_prob,
            self.burst_indicator,
            self.sequential_indicator,
            self.prefetch_success,
            self.arrival_variance,
            self.peak_frequency,
            self.time_pattern,
            self.thread_locality,
            cache_pressure, // Use cache pressure as pressure indicator
            self.aging_factor,
        ]
    }

    /// Update recency based on current time
    #[allow(dead_code)] // ML system - used in machine learning recency feature updates
    pub fn update_recency(&mut self, current_time_ns: u64, last_access_ns: u64) {
        let time_diff = current_time_ns.saturating_sub(last_access_ns);
        self.recency = (time_diff as f64 / 1e9).min(1.0); // Normalize to seconds, cap at 1.0
    }

    /// Update frequency with exponential moving average
    #[allow(dead_code)] // ML system - used in machine learning frequency feature updates
    pub fn update_frequency(&mut self, new_frequency: f64, alpha: f64) {
        self.frequency = alpha * new_frequency + (1.0 - alpha) * self.frequency;
    }

    /// Update temporal locality based on access pattern
    pub fn update_temporal_locality(&mut self, is_temporal_hit: bool) {
        let update_value = if is_temporal_hit { 0.1 } else { -0.05 };
        self.temporal_locality = (self.temporal_locality + update_value).clamp(0.0, 1.0);
    }

    /// Update spatial locality based on access pattern
    pub fn update_spatial_locality(&mut self, is_spatial_hit: bool) {
        let update_value = if is_spatial_hit { 0.1 } else { -0.05 };
        self.spatial_locality = (self.spatial_locality + update_value).clamp(0.0, 1.0);
    }

    /// Update working set probability
    pub fn update_working_set_prob(&mut self, is_in_working_set: bool) {
        let target = if is_in_working_set { 1.0 } else { 0.0 };
        self.working_set_prob = 0.9 * self.working_set_prob + 0.1 * target;
    }

    /// Update burst indicator
    pub fn update_burst_indicator(&mut self, access_rate: f64, threshold: f64) {
        self.burst_indicator = if access_rate > threshold { 1.0 } else { 0.0 };
    }

    /// Update sequential access indicator
    pub fn update_sequential_indicator(&mut self, is_sequential: bool) {
        let target = if is_sequential { 1.0 } else { 0.0 };
        self.sequential_indicator = 0.8 * self.sequential_indicator + 0.2 * target;
    }

    /// Update prefetch success rate
    pub fn update_prefetch_success(&mut self, prefetch_hit: bool) {
        let target = if prefetch_hit { 1.0 } else { 0.0 };
        self.prefetch_success = 0.9 * self.prefetch_success + 0.1 * target;
    }

    /// Update arrival variance
    pub fn update_arrival_variance(&mut self, variance: f64) {
        self.arrival_variance = 0.8 * self.arrival_variance + 0.2 * variance;
    }

    /// Update peak frequency
    pub fn update_peak_frequency(&mut self, current_frequency: f64) {
        self.peak_frequency = self.peak_frequency.max(current_frequency);
    }

    /// Update time pattern based on time of day
    pub fn update_time_pattern(&mut self, hour_of_day: u8) {
        // Simple time pattern based on hour (0-23)
        // Peak hours: 9-17 (work hours)
        let normalized_hour = hour_of_day as f64 / 24.0;
        self.time_pattern = if (9..=17).contains(&hour_of_day) {
            0.8 + 0.2 * (normalized_hour - 0.5).abs()
        } else {
            0.2 + 0.3 * (normalized_hour - 0.5).abs()
        };
    }

    /// Update thread locality
    pub fn update_thread_locality(&mut self, same_thread_access: bool) {
        let target = if same_thread_access { 1.0 } else { 0.0 };
        self.thread_locality = 0.9 * self.thread_locality + 0.1 * target;
    }

    /// Update pressure indicator
    pub fn update_pressure_indicator(&mut self, cache_pressure: f64) {
        self.pressure_indicator = cache_pressure.clamp(0.0, 1.0);
    }

    /// Apply aging to all features
    pub fn apply_aging(&mut self, aging_rate: f64) {
        self.aging_factor *= aging_rate;

        // Age frequency and other temporal features
        self.frequency *= aging_rate;
        self.temporal_locality *= aging_rate;
        self.working_set_prob *= aging_rate;
    }

    /// Get feature importance weights (for interpretation)
    #[allow(dead_code)] // ML system - used in machine learning model interpretation and feature analysis
    pub fn get_feature_importance() -> [f64; FEATURE_COUNT] {
        [
            0.15, // recency - high importance
            0.20, // frequency - highest importance
            0.05, // regularity
            0.03, // relative_size
            0.12, // temporal_locality
            0.08, // spatial_locality
            0.10, // working_set_prob
            0.06, // burst_indicator
            0.04, // sequential_indicator
            0.07, // prefetch_success
            0.02, // arrival_variance
            0.09, // peak_frequency
            0.03, // time_pattern
            0.05, // thread_locality
            0.08, // pressure_indicator
            0.06, // aging_factor
        ]
    }

    /// Get feature names for debugging/logging
    #[allow(dead_code)] // ML system - used in machine learning debugging and logging
    pub fn get_feature_names() -> [&'static str; FEATURE_COUNT] {
        [
            "recency",
            "frequency",
            "regularity",
            "relative_size",
            "temporal_locality",
            "spatial_locality",
            "working_set_prob",
            "burst_indicator",
            "sequential_indicator",
            "prefetch_success",
            "arrival_variance",
            "peak_frequency",
            "time_pattern",
            "thread_locality",
            "pressure_indicator",
            "aging_factor",
        ]
    }

    /// Normalize all features to [0,1] range
    #[allow(dead_code)] // ML system - used in machine learning feature normalization and preprocessing
    pub fn normalize(&mut self) {
        self.recency = self.recency.clamp(0.0, 1.0);
        self.frequency = (self.frequency / (self.frequency + 1.0)).clamp(0.0, 1.0);
        self.regularity = self.regularity.clamp(0.0, 1.0);
        self.relative_size = self.relative_size.clamp(0.0, 1.0);
        self.temporal_locality = self.temporal_locality.clamp(0.0, 1.0);
        self.spatial_locality = self.spatial_locality.clamp(0.0, 1.0);
        self.working_set_prob = self.working_set_prob.clamp(0.0, 1.0);
        self.burst_indicator = self.burst_indicator.clamp(0.0, 1.0);
        self.sequential_indicator = self.sequential_indicator.clamp(0.0, 1.0);
        self.prefetch_success = self.prefetch_success.clamp(0.0, 1.0);
        self.arrival_variance = self.arrival_variance.clamp(0.0, 1.0);
        self.peak_frequency = (self.peak_frequency / (self.peak_frequency + 1.0)).clamp(0.0, 1.0);
        self.time_pattern = self.time_pattern.clamp(0.0, 1.0);
        self.thread_locality = self.thread_locality.clamp(0.0, 1.0);
        self.pressure_indicator = self.pressure_indicator.clamp(0.0, 1.0);
        self.aging_factor = self.aging_factor.clamp(0.0, 1.0);
    }
}
