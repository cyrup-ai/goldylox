//! Compression algorithms for cold tier storage
//!
//! This module provides compression functionality by re-exporting the canonical
//! CompressionEngine implementation from data_structures.rs with additional utilities.


// Re-export canonical implementations from data_structures.rs
pub use super::data_structures::{
    CompressionEngine,
    CompressionAlgorithm,
    CompressionStatsSnapshot,
};



/// Compression configuration for backward compatibility
#[allow(dead_code)] // Cold tier - CompressionConfig used in compression configuration management
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







}

impl CompressionStatsSnapshot {
    /// Get average compression ratio
    #[allow(dead_code)] // Cold tier - avg_compression_ratio used in compression ratio analysis
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