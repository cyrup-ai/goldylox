//! Compression algorithms for cold tier storage
//!
//! This module provides compression functionality by re-exporting the canonical
//! CompressionEngine implementation from data_structures.rs with additional utilities.

// Re-export canonical implementations from data_structures.rs
pub use super::data_structures::{
    CompressionEngine,
    CompressionResult,
    DecompressionResult,
    CompressionAlgorithm,
    CompressionStats,
    CompressionStatsSnapshot,
    AlgorithmMetrics,
    AdaptiveThresholds,
};



/// Compression configuration for backward compatibility
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// Compression algorithm to use
    pub algorithm: CompressionAlgorithm,
    /// Minimum data size to trigger compression (bytes)
    pub min_compress_size: usize,
    /// Maximum compression ratio before fallback to uncompressed
    pub max_compression_ratio: f64,
    /// Enable fast compression mode
    pub fast_mode: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            algorithm: CompressionAlgorithm::Lz4,
            min_compress_size: 512,
            max_compression_ratio: 1.1,
            fast_mode: true,
        }
    }
}

impl CompressionEngine {
    /// Create new compression engine with legacy config compatibility
    pub fn with_config(config: CompressionConfig) -> Self {
        let engine = Self::new(6); // Default compression level
        engine.set_algorithm(config.algorithm);
        engine.update_thresholds(
            config.min_compress_size as u32,
            config.max_compression_ratio as f32,
            100_000_000.0, // Default speed threshold
        );
        engine.fast_mode.store(config.fast_mode, std::sync::atomic::Ordering::Relaxed);
        engine
    }

    /// Reset compression statistics (legacy compatibility)
    pub fn reset_stats(&self) {
        // Reset atomic counters to zero
        self.compression_stats.total_compressed.store(0, std::sync::atomic::Ordering::Relaxed);
        self.compression_stats.total_uncompressed.store(0, std::sync::atomic::Ordering::Relaxed);
        self.compression_stats.compression_ops.store(0, std::sync::atomic::Ordering::Relaxed);
        self.compression_stats.decompression_ops.store(0, std::sync::atomic::Ordering::Relaxed);
        self.compression_stats.total_compression_time_ns.store(0, std::sync::atomic::Ordering::Relaxed);
        self.compression_stats.total_decompression_time_ns.store(0, std::sync::atomic::Ordering::Relaxed);

        // Reset per-algorithm metrics
        for mut entry in self.algorithm_metrics.iter_mut() {
            entry.value_mut().avg_compression_ratio = 1.0;
            entry.value_mut().avg_compression_speed = 0.0;
            entry.value_mut().avg_decompression_speed = 0.0;
            entry.value_mut().operation_count = 0;
            entry.value_mut().compression_ops = 0;
            entry.value_mut().decompression_ops = 0;
        }
    }

    /// Update compression configuration (legacy compatibility)
    pub fn update_config(&self, config: CompressionConfig) {
        self.set_algorithm(config.algorithm);
        self.update_thresholds(
            config.min_compress_size as u32,
            config.max_compression_ratio as f32,
            100_000_000.0,
        );
        self.fast_mode.store(config.fast_mode, std::sync::atomic::Ordering::Relaxed);
    }

    /// Get current configuration (legacy compatibility)
    pub fn get_config(&self) -> CompressionConfig {
        CompressionConfig {
            algorithm: self.get_algorithm(),
            min_compress_size: self.adaptive_thresholds.min_compression_size as usize,
            max_compression_ratio: self.adaptive_thresholds.min_compression_ratio as f64,
            fast_mode: self.fast_mode.load(std::sync::atomic::Ordering::Relaxed),
        }
    }
}

impl CompressionStatsSnapshot {
    /// Get average compression ratio
    pub fn avg_compression_ratio(&self) -> f64 {
        if self.compression_ops > 0 && self.total_uncompressed > 0 {
            self.total_compressed as f64 / self.total_uncompressed as f64
        } else {
            1.0
        }
    }

    /// Get total space saved through compression
    pub fn total_space_saved(&self) -> u64 {
        self.total_uncompressed.saturating_sub(self.total_compressed)
    }

    /// Get compression effectiveness (0.0 to 1.0)
    pub fn compression_effectiveness(&self) -> f64 {
        if self.compression_ops > 0 {
            1.0 - self.avg_compression_ratio()
        } else {
            0.0
        }
    }

    /// Get average compression time per operation (nanoseconds)
    pub fn avg_compression_time_ns(&self) -> u64 {
        if self.compression_ops > 0 {
            self.total_compression_time_ns / self.compression_ops
        } else {
            0
        }
    }

    /// Get average decompression time per operation (nanoseconds)
    pub fn avg_decompression_time_ns(&self) -> u64 {
        if self.decompression_ops > 0 {
            self.total_decompression_time_ns / self.decompression_ops
        } else {
            0
        }
    }

    /// Get compression throughput (bytes per second)
    pub fn compression_throughput(&self) -> f64 {
        if self.total_compression_time_ns > 0 {
            (self.total_uncompressed as f64 * 1_000_000_000.0) / self.total_compression_time_ns as f64
        } else {
            0.0
        }
    }

    /// Get decompression throughput (bytes per second)
    pub fn decompression_throughput(&self) -> f64 {
        if self.total_decompression_time_ns > 0 {
            (self.total_compressed as f64 * 1_000_000_000.0) / self.total_decompression_time_ns as f64
        } else {
            0.0
        }
    }
}