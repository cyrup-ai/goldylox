//! Trend analysis for memory usage patterns
//!
//! This module implements statistical analysis of memory usage trends to predict
//! future behavior and detect anomalies.

#![allow(dead_code)] // Warm tier monitoring - Complete trend analysis library for memory usage patterns

use std::sync::atomic::Ordering;

use crate::cache::tier::warm::atomic_float::AtomicF64;
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Trend analysis for memory usage patterns
#[derive(Debug)]
pub struct TrendAnalysis {
    /// Current trend direction (-1.0 to 1.0)
    pub trend_direction: AtomicF64,
    /// Trend strength (0.0-1.0)
    pub trend_strength: AtomicF64,

    /// Rate of change (bytes per second)
    pub change_rate: AtomicF64,
    /// Predicted time to threshold breach
    pub time_to_breach_sec: AtomicF64,
    /// Trend analysis confidence
    pub analysis_confidence: AtomicF64,
}

impl Default for TrendAnalysis {
    fn default() -> Self {
        Self::new()
    }
}

impl TrendAnalysis {
    pub fn new() -> Self {
        Self::new_with_config(true)
    }

    pub fn new_with_config(_leak_detection_active: bool) -> Self {
        Self {
            trend_direction: AtomicF64::new(0.0),
            trend_strength: AtomicF64::new(0.0),

            change_rate: AtomicF64::new(0.0),
            time_to_breach_sec: AtomicF64::new(f64::INFINITY),
            analysis_confidence: AtomicF64::new(0.0),
        }
    }

    /// Perform linear regression analysis on sample data
    pub fn analyze_samples(
        &self,
        samples: &[u64],
        timestamps: &[u64],
    ) -> Result<(), CacheOperationError> {
        if samples.len() != timestamps.len() || samples.len() < 8 {
            return Err(CacheOperationError::configuration_error(
                "Insufficient data for trend analysis",
            ));
        }

        // Simple linear regression for trend analysis
        let n = samples.len() as f64;
        let sum_x: f64 = timestamps.iter().map(|&t| t as f64).sum();
        let sum_y: f64 = samples.iter().map(|&u| u as f64).sum();
        let sum_xy: f64 = timestamps
            .iter()
            .zip(samples.iter())
            .map(|(&t, &u)| t as f64 * u as f64)
            .sum();
        let sum_x2: f64 = timestamps.iter().map(|&t| (t as f64).powi(2)).sum();

        // Calculate slope (change rate)
        let denominator = n * sum_x2 - sum_x.powi(2);
        if denominator.abs() < f64::EPSILON {
            return Ok(()); // Avoid division by zero
        }

        let slope = (n * sum_xy - sum_x * sum_y) / denominator;

        // Calculate correlation coefficient (trend strength)
        let mean_x = sum_x / n;
        let mean_y = sum_y / n;
        let var_x: f64 = timestamps
            .iter()
            .map(|&t| (t as f64 - mean_x).powi(2))
            .sum::<f64>()
            / n;
        let var_y: f64 = samples
            .iter()
            .map(|&u| (u as f64 - mean_y).powi(2))
            .sum::<f64>()
            / n;
        let covar_xy: f64 = timestamps
            .iter()
            .zip(samples.iter())
            .map(|(&t, &u)| (t as f64 - mean_x) * (u as f64 - mean_y))
            .sum::<f64>()
            / n;

        let correlation = if var_x > 0.0 && var_y > 0.0 {
            covar_xy / (var_x.sqrt() * var_y.sqrt())
        } else {
            0.0
        };

        // Update trend analysis atomically
        self.change_rate.store(slope / 1e9, Ordering::Relaxed); // Convert to bytes/second
        self.trend_strength
            .store(correlation.abs(), Ordering::Relaxed);
        self.trend_direction.store(correlation, Ordering::Relaxed);
        self.analysis_confidence.store(0.8, Ordering::Relaxed); // Simplified

        Ok(())
    }

    /// Get current trend metrics
    pub fn get_trend_metrics(&self) -> (f64, f64, f64, f64) {
        (
            self.trend_direction.load(Ordering::Relaxed),
            self.trend_strength.load(Ordering::Relaxed),
            self.change_rate.load(Ordering::Relaxed),
            self.analysis_confidence.load(Ordering::Relaxed),
        )
    }

    /// Check if trend indicates potential memory leak
    pub fn indicates_memory_leak(&self) -> bool {
        let trend_direction = self.trend_direction.load(Ordering::Relaxed);
        let trend_strength = self.trend_strength.load(Ordering::Relaxed);
        let change_rate = self.change_rate.load(Ordering::Relaxed);

        // Detect sustained memory growth
        trend_direction > 0.8 && trend_strength > 0.7 && change_rate > 0.0
    }

    /// Calculate predicted time to reach a threshold
    pub fn time_to_threshold(&self, current_usage: u64, threshold: u64) -> f64 {
        let change_rate = self.change_rate.load(Ordering::Relaxed);

        if change_rate <= 0.0 || current_usage >= threshold {
            return f64::INFINITY;
        }

        let remaining_bytes = threshold.saturating_sub(current_usage);
        remaining_bytes as f64 / change_rate
    }
}
