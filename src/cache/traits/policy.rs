//! Eviction policy and access tracking traits with Higher-Ranked Trait Bounds (HRTBs)
//!
//! This module defines sophisticated eviction policies and access pattern recognition
//! using advanced lifetime management for zero-cost abstractions.

#![allow(dead_code)] // Cache traits - Eviction policy trait definitions with ML optimization

// Internal policy traits - may not be used in minimal API

use std::fmt::Debug;
use std::time::Instant;

use super::core::{CacheKey, CacheValue};
use super::types_and_enums::{
    AccessType, EvictionReason, PerformanceMetrics, SelectionReason, TemporalPattern,
};

/// Higher-Ranked Trait Bound for eviction policy implementations
pub trait EvictionPolicy<K: CacheKey, V: CacheValue>: Send + Sync + Debug {
    /// Access pattern tracker type (GAT)
    type AccessTracker<'a>: AccessTracker<K, V> + 'a
    where
        Self: 'a;

    /// Eviction candidate type (GAT)
    type Candidate<'a>: EvictionCandidate<K, V> + 'a
    where
        Self: 'a;

    /// Select eviction candidate using HRTB for lifetime polymorphism
    fn select_candidate<'a, F>(&'a self, accessor: F) -> Option<Self::Candidate<'a>>
    where
        F: for<'b> FnOnce(&'b Self::AccessTracker<'b>) -> &'b Self::AccessTracker<'b>;

    /// Record access event with advanced pattern recognition
    fn record_access(&mut self, key: &K, access_type: AccessType, timestamp: Instant);

    /// Record eviction event for policy adaptation
    fn record_eviction(&mut self, key: &K, reason: EvictionReason, value_metadata: &V::Metadata);

    /// Get access tracker for analysis (GAT)
    fn access_tracker<'a>(&'a self) -> Self::AccessTracker<'a>;

    /// Optimize policy parameters based on access patterns
    fn optimize(&mut self, performance_metrics: &PerformanceMetrics);
}

/// Access pattern tracker with advanced analytics
pub trait AccessTracker<K: CacheKey, V: CacheValue>: Send + Sync + Debug {
    /// Record access event
    fn record_access(&mut self, key: &K, timestamp: Instant, access_type: AccessType);

    /// Get access frequency for key
    fn access_frequency(&self, key: &K) -> f64;

    /// Get recency score for key
    fn recency_score(&self, key: &K) -> f64;

    /// Predict future access probability
    fn predict_access(&self, key: &K) -> f64;

    /// Get temporal access pattern
    fn temporal_pattern(&self, key: &K) -> TemporalPattern;
}

/// Eviction candidate with rich decision data
pub trait EvictionCandidate<K: CacheKey, V: CacheValue>: Send + Sync + Debug {
    /// Candidate key
    fn key(&self) -> &K;

    /// Eviction score (higher = more likely to evict)
    fn eviction_score(&self) -> f64;

    /// Reason for selection
    fn selection_reason(&self) -> SelectionReason;

    /// Confidence in decision (0.0 to 1.0)
    fn confidence(&self) -> f32;
}

/// Advanced eviction policy with machine learning integration
pub trait AdvancedEvictionPolicy<K: CacheKey, V: CacheValue>: EvictionPolicy<K, V> {
    /// Error type for policy operations
    type Error: std::error::Error + Send + Sync + 'static;

    /// Machine learning model type for predictive analytics
    type MLModel: MachineLearningModel;

    /// Predict future access probability using ML model
    fn predict_access(&self, key: &K) -> f64;

    /// Update ML model with recent access patterns
    fn update_model(&mut self, feedback: &[AccessEvent<K>]);

    /// Get model confidence score (0.0-1.0)
    fn model_confidence(&self) -> f64;

    /// Adaptive threshold adjustment based on performance
    fn adjust_thresholds(&mut self, performance: &PerformanceMetrics);

    /// Check if policy supports machine learning
    fn supports_ml(&self) -> bool {
        true
    }

    /// Adapt policy parameters based on workload patterns
    fn adapt_to_workload(&self, pattern: TemporalPattern) -> Result<(), Self::Error>;

    /// Get policy configuration name
    fn policy_name(&self) -> &'static str;
}

/// Machine learning model trait for cache optimization
pub trait MachineLearningModel: Send + Sync + Debug {
    /// Train model with access pattern data
    fn train(&mut self, data: &[TrainingExample]);

    /// Predict access probability for given features
    fn predict(&self, features: &[f64]) -> f64;

    /// Get model accuracy score
    fn accuracy(&self) -> f64;

    /// Update model with online learning
    fn update(&mut self, example: &TrainingExample);

    /// Get model confidence for a prediction
    fn prediction_confidence(&self, features: &[f64]) -> f64;
}

/// Training example for ML model
#[derive(Debug, Clone)]
pub struct TrainingExample {
    /// Feature vector (recency, frequency, size, etc.)
    pub features: Vec<f64>,
    /// Target value (was accessed = 1.0, not accessed = 0.0)
    pub target: f64,
    /// Example weight for importance
    pub weight: f64,
}

// AccessEvent moved to canonical location: crate::cache::eviction::types::AccessEvent
pub use crate::cache::eviction::types::AccessEvent;

/// Access tracking statistics for monitoring and optimization
#[derive(Debug, Clone)]
pub struct AccessTrackingStats {
    /// Total number of tracked keys
    pub tracked_keys: usize,

    /// Total access records
    pub total_accesses: u64,

    /// Average access frequency
    pub avg_frequency: f64,

    /// Hot key threshold
    pub hot_key_threshold: f64,

    /// Cold key threshold  
    pub cold_key_threshold: f64,

    /// Memory usage for tracking
    pub memory_usage: usize,

    /// Prediction accuracy (if ML is supported)
    pub prediction_accuracy: Option<f64>,

    /// Model training iterations completed
    pub training_iterations: u64,
}
