//! Frequency estimator with exponential moving averages
//!
//! This module provides frequency estimation capabilities using exponential
//! moving averages for tracking access frequency patterns.

#![allow(dead_code)] // Warm tier access tracking - Complete frequency estimation library for access pattern analysis

use crossbeam_utils::atomic::AtomicCell;

use crate::cache::tier::warm::core::WarmCacheKey;
use crate::cache::traits::CacheKey;

/// Frequency estimator with exponential moving averages
#[derive(Debug)]
pub struct FrequencyEstimator<K: CacheKey> {
    /// Alpha parameter for EMA
    alpha: f64,
    /// Global frequency estimate
    global_frequency: AtomicCell<f64>,
    /// Update interval
    update_interval_ns: u64,
    /// Phantom data for generic parameter
    _phantom: std::marker::PhantomData<K>,
}

impl<K: CacheKey> Default for FrequencyEstimator<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: CacheKey> FrequencyEstimator<K> {
    /// Create new frequency estimator
    pub fn new() -> Self {
        Self {
            alpha: 0.1, // EMA smoothing factor
            global_frequency: AtomicCell::new(1.0),
            update_interval_ns: 1_000_000_000, // 1 second
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create frequency estimator with custom parameters
    pub fn with_params(alpha: f64, initial_frequency: f64, update_interval_ns: u64) -> Self {
        Self {
            alpha,
            global_frequency: AtomicCell::new(initial_frequency),
            update_interval_ns,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Update frequency estimate for key
    pub fn update_frequency(&self, _key: &WarmCacheKey<K>, new_frequency: f64) {
        let current = self.global_frequency.load();
        let updated = self.alpha * new_frequency + (1.0 - self.alpha) * current;
        self.global_frequency.store(updated);
    }

    /// Get current global frequency estimate
    pub fn global_frequency(&self) -> f64 {
        self.global_frequency.load()
    }

    /// Set alpha parameter for EMA smoothing
    pub fn set_alpha(&mut self, alpha: f64) {
        self.alpha = alpha.clamp(0.0, 1.0);
    }

    /// Get current alpha parameter
    pub fn get_alpha(&self) -> f64 {
        self.alpha
    }

    /// Set update interval
    pub fn set_update_interval(&mut self, interval_ns: u64) {
        self.update_interval_ns = interval_ns;
    }

    /// Get current update interval
    pub fn get_update_interval(&self) -> u64 {
        self.update_interval_ns
    }

    /// Reset frequency estimate to initial value
    pub fn reset(&self, initial_frequency: f64) {
        self.global_frequency.store(initial_frequency);
    }
}
