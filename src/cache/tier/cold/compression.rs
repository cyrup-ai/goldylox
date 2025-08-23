//! Compression algorithms for cold tier storage
//!
//! This module provides efficient compression to reduce storage footprint
//! for serialized cache value data in the cold tier cache.

use lz4_flex::{compress_prepend_size, decompress_size_prepended};

use crate::cache::traits::types_and_enums::CacheOperationError;

/// Compression configuration
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

/// Available compression algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    /// No compression
    None,
    /// LZ4 fast compression
    Lz4,
}

/// Compression result with metadata
#[derive(Debug)]
pub struct CompressionResult {
    /// Compressed data
    pub data: Vec<u8>,
    /// Original size before compression
    pub original_size: usize,
    /// Compressed size
    pub compressed_size: usize,
    /// Compression ratio (compressed/original)
    pub compression_ratio: f64,
    /// Algorithm used
    pub algorithm: CompressionAlgorithm,
}

/// Decompression result
#[derive(Debug)]
pub struct DecompressionResult {
    /// Decompressed data
    pub data: Vec<u8>,
    /// Actual size after decompression
    pub actual_size: usize,
}

/// Compression engine
#[derive(Debug)]
pub struct CompressionEngine {
    /// Configuration
    config: CompressionConfig,
    /// Statistics
    stats: CompressionStats,
}

/// Compression statistics
#[derive(Debug, Default)]
pub struct CompressionStats {
    /// Total compressions performed
    pub total_compressions: u64,
    /// Total decompressions performed
    pub total_decompressions: u64,
    /// Total bytes compressed
    pub total_bytes_compressed: u64,
    /// Total bytes decompressed
    pub total_bytes_decompressed: u64,
    /// Average compression ratio
    pub avg_compression_ratio: f64,
    /// Compression failures
    pub compression_failures: u64,
    /// Decompression failures
    pub decompression_failures: u64,
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
    /// Create new compression engine with configuration
    pub fn new(config: CompressionConfig) -> Self {
        Self {
            config,
            stats: CompressionStats::default(),
        }
    }

    /// Compress data using configured algorithm
    pub fn compress(&mut self, data: &[u8]) -> Result<CompressionResult, CacheOperationError> {
        let original_size = data.len();

        // Skip compression for small data
        if original_size < self.config.min_compress_size {
            return Ok(CompressionResult {
                data: data.to_vec(),
                original_size,
                compressed_size: original_size,
                compression_ratio: 1.0,
                algorithm: CompressionAlgorithm::None,
            });
        }

        let compressed_data = match self.config.algorithm {
            CompressionAlgorithm::None => data.to_vec(),
            CompressionAlgorithm::Lz4 => compress_prepend_size(data),
        };

        let compressed_size = compressed_data.len();
        let compression_ratio = compressed_size as f64 / original_size as f64;

        // Check if compression is effective
        if compression_ratio > self.config.max_compression_ratio {
            return Ok(CompressionResult {
                data: data.to_vec(),
                original_size,
                compressed_size: original_size,
                compression_ratio: 1.0,
                algorithm: CompressionAlgorithm::None,
            });
        }

        // Update statistics
        self.stats.total_compressions += 1;
        self.stats.total_bytes_compressed += original_size as u64;
        self.stats.avg_compression_ratio = (self.stats.avg_compression_ratio
            * (self.stats.total_compressions - 1) as f64
            + compression_ratio)
            / self.stats.total_compressions as f64;

        Ok(CompressionResult {
            data: compressed_data,
            original_size,
            compressed_size,
            compression_ratio,
            algorithm: self.config.algorithm,
        })
    }

    /// Decompress data using the specified algorithm
    pub fn decompress(
        &mut self,
        data: &[u8],
        algorithm: CompressionAlgorithm,
        expected_size: usize,
    ) -> Result<DecompressionResult, CacheOperationError> {
        let decompressed_data = match algorithm {
            CompressionAlgorithm::None => data.to_vec(),
            CompressionAlgorithm::Lz4 => decompress_size_prepended(data).map_err(|e| {
                CacheOperationError::resource_exhausted(&format!("LZ4 compression failed: {}", e))
            })?,
        };

        let actual_size = decompressed_data.len();

        // Validate decompressed size
        if actual_size != expected_size && algorithm != CompressionAlgorithm::None {
            self.stats.decompression_failures += 1;
            return Err(CacheOperationError::resource_exhausted(&format!(
                "Decompression size mismatch: expected {}, got {}",
                expected_size, actual_size
            )));
        }

        // Update statistics
        self.stats.total_decompressions += 1;
        self.stats.total_bytes_decompressed += actual_size as u64;

        Ok(DecompressionResult {
            data: decompressed_data,
            actual_size,
        })
    }

    /// Get compression statistics
    pub fn get_stats(&self) -> &CompressionStats {
        &self.stats
    }

    /// Reset compression statistics
    pub fn reset_stats(&mut self) {
        self.stats = CompressionStats::default();
    }

    /// Update compression configuration
    pub fn update_config(&mut self, config: CompressionConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn get_config(&self) -> &CompressionConfig {
        &self.config
    }

    /// Estimate compression ratio for data
    pub fn estimate_compression_ratio(&self, data: &[u8]) -> f64 {
        if data.len() < self.config.min_compress_size {
            return 1.0;
        }

        // Simple entropy-based estimation
        let mut byte_counts = [0u32; 256];
        for &byte in data {
            byte_counts[byte as usize] += 1;
        }

        let mut entropy = 0.0;
        let len = data.len() as f64;
        for &count in &byte_counts {
            if count > 0 {
                let p = count as f64 / len;
                entropy -= p * p.log2();
            }
        }

        // Estimate compression ratio based on entropy
        let max_entropy = 8.0; // Maximum possible entropy for byte data
        let compression_estimate = 0.3 + (entropy / max_entropy) * 0.7;

        compression_estimate.clamp(0.1, 1.0)
    }

    /// Check if data should be compressed
    pub fn should_compress(&self, data: &[u8]) -> bool {
        if data.len() < self.config.min_compress_size {
            return false;
        }

        if self.config.algorithm == CompressionAlgorithm::None {
            return false;
        }

        // Estimate compression effectiveness
        let estimated_ratio = self.estimate_compression_ratio(data);
        estimated_ratio < self.config.max_compression_ratio
    }
}

impl CompressionStats {
    /// Get average compression time per operation
    pub fn avg_compression_ratio(&self) -> f64 {
        self.avg_compression_ratio
    }

    /// Get total space saved through compression
    pub fn total_space_saved(&self) -> u64 {
        if self.avg_compression_ratio > 0.0 {
            let original_size =
                (self.total_bytes_compressed as f64 / self.avg_compression_ratio) as u64;
            original_size.saturating_sub(self.total_bytes_compressed)
        } else {
            0
        }
    }

    /// Get compression effectiveness
    pub fn compression_effectiveness(&self) -> f64 {
        if self.total_compressions > 0 {
            1.0 - self.avg_compression_ratio
        } else {
            0.0
        }
    }
}
