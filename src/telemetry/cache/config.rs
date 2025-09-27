#![allow(dead_code)]
// Telemetry System - Complete cache measurement configuration with pattern analyzer and eviction settings

//! Cache configuration for measurement system

/// Configuration for measurement cache
#[allow(dead_code)] // Configuration system - used in measurement and telemetry configuration
#[derive(Debug, Clone)]
pub struct MeasurementCacheConfig {
    pub max_entries: usize,
    pub max_memory_bytes: usize,
    pub eviction_threshold: f64,
}

impl Default for MeasurementCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 10000,
            max_memory_bytes: 64 * 1024 * 1024, // 64MB
            eviction_threshold: 0.8,
        }
    }
}

/// Configuration for pattern analyzer
#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    pub pattern_detection_active: bool,
    pub max_pattern_history: usize,
    pub analysis_window_size: usize,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            pattern_detection_active: true,
            max_pattern_history: 1000,
            analysis_window_size: 100,
        }
    }
}
