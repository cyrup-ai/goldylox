//! Compression algorithms for cold tier storage
//!
//! This module provides compression functionality by re-exporting the canonical
//! CompressionEngine implementation from data_structures.rs with additional utilities.

// Re-export canonical implementations
pub use super::compression_engine::CompressionStatsSnapshot;
pub use super::data_structures::{CompressionAlgorithm, CompressionEngine};

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

impl CompressionEngine {}

// Helper methods moved to compression_engine.rs canonical implementation
