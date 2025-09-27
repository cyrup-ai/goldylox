//! Compression engine with multiple algorithms and adaptive selection
//!
//! This module provides compression and decompression capabilities with multiple algorithms,
//! performance tracking, and adaptive algorithm selection for optimal storage efficiency.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::time::Instant;

use brotli::{CompressorReader, Decompressor};
use crc32fast;
use crossbeam_utils::atomic::AtomicCell;
use dashmap::DashMap;
use flate2::{Compression as GzipLevel, read::GzDecoder, write::GzEncoder};
use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use snap::raw::{Decoder as SnapDecoder, Encoder as SnapEncoder};

use super::data_structures::{
    AdaptiveThresholds, AlgorithmMetrics, CompressionAlgorithm, CompressionEngine, CompressionStats,
};

/// Workload types for algorithm selection
#[allow(dead_code)] // Cold tier - WorkloadType used in compression algorithm selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkloadType {
    /// Prioritize speed over compression ratio
    LatencyOptimized,
    /// Maximize compression ratio
    StorageOptimized,
    /// Balance speed and compression
    Balanced,
    /// Use current adaptive algorithm
    Adaptive,
}

impl CompressionEngine {
    /// Create new compression engine with default settings
    pub fn new(_compression_level: u8) -> Self {
        Self {
            algorithm: AtomicCell::new(CompressionAlgorithm::Lz4),
            compression_stats: CompressionStats::new(),
            algorithm_metrics: DashMap::new(),
            adaptive_thresholds: AdaptiveThresholds::default(),
            adaptation_counter: AtomicU64::new(0),
            last_adaptation: AtomicU64::new(0),
            compression_level: AtomicU8::new(_compression_level),
            fast_mode: AtomicBool::new(false),
        }
    }

    /// Set compression algorithm (updates current algorithm atomically)
    #[allow(dead_code)] // Cold tier - set_algorithm used in dynamic compression algorithm switching
    pub fn set_algorithm(&self, algorithm: CompressionAlgorithm) {
        self.algorithm.store(algorithm);
    }

    /// Get current compression algorithm
    pub fn get_algorithm(&self) -> CompressionAlgorithm {
        self.algorithm.load()
    }

    /// Update compression thresholds for adaptive behavior
    pub fn update_thresholds(&self, min_size: u32, max_ratio: f32, speed_threshold: f64) {
        self.adaptive_thresholds
            .min_compression_size
            .store(min_size, Ordering::Relaxed);
        self.adaptive_thresholds
            .min_compression_ratio
            .store(max_ratio.to_bits(), Ordering::Relaxed);
        self.adaptive_thresholds
            .speed_threshold
            .store(speed_threshold.to_bits(), Ordering::Relaxed);
    }

    /// Select optimal compression algorithm for given data
    pub fn select_algorithm(&self, data: &[u8]) -> CompressionAlgorithm {
        let data_size = data.len() as u32;

        // Skip compression for small data
        if data_size
            < self
                .adaptive_thresholds
                .min_compression_size
                .load(Ordering::Relaxed)
        {
            return CompressionAlgorithm::None;
        }

        // Use current algorithm as default
        let current = self.algorithm.load();

        // Simple heuristic based on data size and current performance
        if data_size > 4096 {
            // For larger data, prefer better compression - check Brotli first for best ratio
            if let Some(brotli_metrics) = self.algorithm_metrics.get(&CompressionAlgorithm::Brotli)
                && brotli_metrics.avg_compression_ratio < 0.3
            {
                return CompressionAlgorithm::Brotli;
            }
            // Fall back to Zstd for good balance of speed and compression
            if let Some(zstd_metrics) = self.algorithm_metrics.get(&CompressionAlgorithm::Zstd)
                && zstd_metrics.avg_compression_ratio < 0.7
            {
                return CompressionAlgorithm::Zstd;
            }
        }

        // For medium-sized data, balance speed and compression
        if data_size > 1024
            && let Some(lz4_metrics) = self.algorithm_metrics.get(&CompressionAlgorithm::Lz4)
            && lz4_metrics.avg_compression_speed
                > f64::from_bits(
                    self.adaptive_thresholds
                        .speed_threshold
                        .load(Ordering::Relaxed),
                )
        {
            return CompressionAlgorithm::Lz4;
        }

        current
    }

    /// Compress data using specified algorithm
    pub fn compress(
        &self,
        data: &[u8],
        algorithm: CompressionAlgorithm,
    ) -> Result<CompressedData, CompressionError> {
        let start_time = Instant::now();

        let compressed_data = match algorithm {
            CompressionAlgorithm::None => data.to_vec(),
            CompressionAlgorithm::Lz4 => compress_prepend_size(data),
            CompressionAlgorithm::Gzip => {
                let level = GzipLevel::new(
                    self.compression_level
                        .load(std::sync::atomic::Ordering::Relaxed)
                        .min(9) as u32,
                );
                let mut encoder = GzEncoder::new(Vec::new(), level);
                encoder
                    .write_all(data)
                    .map_err(|e| CompressionError::CompressionFailed(e.to_string()))?;
                encoder
                    .finish()
                    .map_err(|e| CompressionError::CompressionFailed(e.to_string()))?
            }
            CompressionAlgorithm::Zstd => zstd::bulk::compress(
                data,
                self.compression_level
                    .load(std::sync::atomic::Ordering::Relaxed)
                    .min(21) as i32,
            )
            .map_err(|e| CompressionError::CompressionFailed(e.to_string()))?,
            CompressionAlgorithm::Snappy => {
                let mut encoder = SnapEncoder::new();
                encoder
                    .compress_vec(data)
                    .map_err(|e| CompressionError::CompressionFailed(e.to_string()))?
            }
            CompressionAlgorithm::Brotli => {
                let mut compressed = Vec::new();
                let mut compressor = CompressorReader::new(
                    data,
                    4096, // buffer size
                    self.compression_level
                        .load(std::sync::atomic::Ordering::Relaxed)
                        .min(11) as u32, // Brotli supports 0-11
                    22,   // window size
                );
                compressor
                    .read_to_end(&mut compressed)
                    .map_err(|e| CompressionError::CompressionFailed(e.to_string()))?;
                compressed
            }
        };

        let elapsed = start_time.elapsed();
        let elapsed_ns = elapsed.as_nanos() as u64;

        // Calculate integrity checksum
        let checksum = crc32fast::hash(data);

        // Update statistics
        self.compression_stats
            .total_uncompressed
            .fetch_add(data.len() as u64, Ordering::Relaxed);
        self.compression_stats
            .total_compressed
            .fetch_add(compressed_data.len() as u64, Ordering::Relaxed);
        self.compression_stats
            .compression_ops
            .fetch_add(1, Ordering::Relaxed);
        self.compression_stats
            .total_compression_time_ns
            .fetch_add(elapsed_ns, Ordering::Relaxed);

        // Update algorithm metrics
        self.update_compression_metrics(algorithm, data.len(), compressed_data.len(), elapsed_ns);

        Ok(CompressedData {
            data: compressed_data,
            original_size: data.len(),
            checksum,
            algorithm,
        })
    }

    /// Decompress data with integrity verification
    pub fn decompress(
        &self,
        compressed_data: &CompressedData,
    ) -> Result<Vec<u8>, CompressionError> {
        let start_time = Instant::now();

        let decompressed_data = match compressed_data.algorithm {
            CompressionAlgorithm::None => compressed_data.data.clone(),
            CompressionAlgorithm::Lz4 => {
                decompress_size_prepended(&compressed_data.data).map_err(|e| {
                    CompressionError::CompressionFailed(format!(
                        "LZ4 decompression failed: {:?}",
                        e
                    ))
                })?
            }
            CompressionAlgorithm::Gzip => {
                let mut decoder = GzDecoder::new(compressed_data.data.as_slice());
                let mut decompressed = Vec::new();
                decoder
                    .read_to_end(&mut decompressed)
                    .map_err(|e| CompressionError::CompressionFailed(e.to_string()))?;
                decompressed
            }
            CompressionAlgorithm::Zstd => {
                zstd::bulk::decompress(&compressed_data.data, compressed_data.original_size)
                    .map_err(|e| CompressionError::CompressionFailed(e.to_string()))?
            }
            CompressionAlgorithm::Snappy => {
                let mut decoder = SnapDecoder::new();
                decoder
                    .decompress_vec(&compressed_data.data)
                    .map_err(|e| CompressionError::CompressionFailed(e.to_string()))?
            }
            CompressionAlgorithm::Brotli => {
                let mut decompressed = Vec::new();
                let mut decompressor = Decompressor::new(compressed_data.data.as_slice(), 4096);
                decompressor.read_to_end(&mut decompressed).map_err(|e| {
                    CompressionError::CompressionFailed(format!(
                        "Brotli decompression failed: {}",
                        e
                    ))
                })?;
                decompressed
            }
        };

        // Verify integrity
        let actual_checksum = crc32fast::hash(&decompressed_data);
        if actual_checksum != compressed_data.checksum {
            return Err(CompressionError::IntegrityCheckFailed {
                expected: compressed_data.checksum,
                actual: actual_checksum,
            });
        }

        let elapsed = start_time.elapsed();
        let elapsed_ns = elapsed.as_nanos() as u64;

        // Update statistics
        self.compression_stats
            .decompression_ops
            .fetch_add(1, Ordering::Relaxed);
        self.compression_stats
            .total_decompression_time_ns
            .fetch_add(elapsed_ns, Ordering::Relaxed);

        // Update algorithm metrics
        self.update_decompression_metrics(
            compressed_data.algorithm,
            decompressed_data.len(),
            elapsed_ns,
        );

        Ok(decompressed_data)
    }

    /// Update compression metrics for algorithm (ENHANCED CONNECTION)
    fn update_compression_metrics(
        &self,
        algorithm: CompressionAlgorithm,
        original_size: usize,
        compressed_size: usize,
        elapsed_ns: u64,
    ) {
        // Connect to existing AlgorithmMetrics DashMap for sophisticated tracking
        let compression_ratio = compressed_size as f32 / original_size as f32;
        let compression_speed = if elapsed_ns > 0 {
            (original_size as f64 * 1_000_000_000.0) / elapsed_ns as f64
        } else {
            f64::MAX
        };

        // Use DashMap concurrent entry API for thread-safe updates
        self.algorithm_metrics
            .entry(algorithm)
            .and_modify(|metrics| {
                // Exponential moving average for sophisticated metric updates
                let alpha = 0.1f32; // Smoothing factor
                metrics.avg_compression_ratio =
                    (1.0 - alpha) * metrics.avg_compression_ratio + alpha * compression_ratio;
                metrics.avg_compression_speed = (1.0 - alpha as f64)
                    * metrics.avg_compression_speed
                    + alpha as f64 * compression_speed;
                metrics.operation_count += 1;
                metrics.compression_ops += 1;
            });

        // Trigger periodic adaptation using sophisticated metrics with race condition prevention
        if self
            .compression_stats
            .compression_ops
            .load(Ordering::Relaxed)
            .is_multiple_of(100)
        {
            // Connect to existing atomic coordination fields for race prevention
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                .as_nanos() as u64;

            // Only one thread can trigger adaptation using compare-and-swap
            if self
                .adaptation_counter
                .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                let last_adaptation = self.last_adaptation.load(Ordering::Relaxed);

                // Minimum 1-second interval between adaptations to prevent thrashing
                if current_time.saturating_sub(last_adaptation) > 1_000_000_000 {
                    self.select_optimal_algorithm_from_metrics();
                    self.last_adaptation.store(current_time, Ordering::Relaxed);
                }

                // Reset adaptation counter
                self.adaptation_counter.store(0, Ordering::Release);
            }
        }
    }

    /// Update decompression metrics for algorithm
    fn update_decompression_metrics(
        &self,
        algorithm: CompressionAlgorithm,
        decompressed_size: usize,
        elapsed_ns: u64,
    ) {
        let decompression_speed = if elapsed_ns > 0 {
            (decompressed_size as f64 * 1_000_000_000.0) / elapsed_ns as f64
        } else {
            f64::MAX
        };

        // Connect to existing AlgorithmMetrics DashMap for decompression tracking
        self.algorithm_metrics
            .entry(algorithm)
            .and_modify(|metrics| {
                // Exponential moving average for sophisticated decompression metric updates
                let alpha = 0.1f64; // Smoothing factor
                metrics.avg_decompression_speed =
                    (1.0 - alpha) * metrics.avg_decompression_speed + alpha * decompression_speed;
                metrics.decompression_ops += 1;
                metrics.operation_count += 1;
            });
    }

    /// Get compression statistics
    pub fn get_stats(&self) -> CompressionStatsSnapshot {
        CompressionStatsSnapshot {
            total_compressed: self
                .compression_stats
                .total_compressed
                .load(Ordering::Relaxed),
            total_uncompressed: self
                .compression_stats
                .total_uncompressed
                .load(Ordering::Relaxed),
            compression_ops: self
                .compression_stats
                .compression_ops
                .load(Ordering::Relaxed),
            decompression_ops: self
                .compression_stats
                .decompression_ops
                .load(Ordering::Relaxed),
            total_compression_time_ns: self
                .compression_stats
                .total_compression_time_ns
                .load(Ordering::Relaxed),
            total_decompression_time_ns: self
                .compression_stats
                .total_decompression_time_ns
                .load(Ordering::Relaxed),
            current_algorithm: self.algorithm.load(),
        }
    }

    /// Adapt compression algorithm based on performance
    pub fn adapt_algorithm(&self) {
        // Connect to existing sophisticated AlgorithmMetrics for optimal selection
        self.select_optimal_algorithm_from_metrics()
    }

    /// Select optimal algorithm using existing sophisticated AlgorithmMetrics
    fn select_optimal_algorithm_from_metrics(&self) {
        let stats = self.get_stats();

        // Need statistical significance before adapting
        if stats.compression_ops < 50 {
            return; // Keep current algorithm until we have enough data
        }

        let mut best_algorithm = self.algorithm.load();
        let mut best_score = 0.0f64;

        // Use existing AlgorithmMetrics DashMap for sophisticated selection
        for entry in &self.algorithm_metrics {
            let algorithm = *entry.key(); // Dereference to get CompressionAlgorithm value
            let metrics = entry.value();
            // Skip algorithms with insufficient data
            if metrics.operation_count < 10 {
                continue;
            }

            // Calculate comprehensive performance score using existing metrics
            let score = self.calculate_algorithm_score(metrics);

            if score > best_score {
                best_score = score;
                best_algorithm = algorithm;
            }
        }

        // Update algorithm if we found a significantly better one
        if best_algorithm != self.algorithm.load() {
            self.algorithm.store(best_algorithm);
        }
    }

    /// Calculate algorithm performance score using existing sophisticated metrics
    fn calculate_algorithm_score(&self, metrics: &AlgorithmMetrics) -> f64 {
        // Use existing adaptive thresholds for decision-making
        let speed_weight = if metrics.avg_compression_speed
            < f64::from_bits(
                self.adaptive_thresholds
                    .speed_threshold
                    .load(Ordering::Relaxed),
            ) {
            0.6 // Prioritize speed when below threshold
        } else {
            0.3 // De-emphasize speed when adequate
        };

        let ratio_weight = 1.0 - speed_weight - 0.1; // Remaining weight for compression ratio
        let reliability_weight = 0.1; // Small weight for operation count

        // Higher compression speed is better (more bytes/second)
        let speed_score = (metrics.avg_compression_speed / 100_000_000.0).min(1.0);

        // Lower compression ratio is better (more compression)
        let ratio_score = (2.0f64 - metrics.avg_compression_ratio as f64).clamp(0.0, 1.0);

        // More operations indicate reliability
        let reliability_score = (metrics.operation_count as f64 / 1000.0).min(1.0);

        // Weighted combination using existing metrics
        speed_score * speed_weight
            + ratio_score * ratio_weight
            + reliability_score * reliability_weight
    }

    /// Connect to existing metrics for workload-aware selection
    pub fn select_algorithm_for_workload(
        &self,
        data: &[u8],
        workload_type: WorkloadType,
    ) -> CompressionAlgorithm {
        // Use existing size thresholds
        if (data.len() as u32)
            < self
                .adaptive_thresholds
                .min_compression_size
                .load(Ordering::Relaxed)
        {
            return CompressionAlgorithm::None;
        }

        match workload_type {
            WorkloadType::LatencyOptimized => {
                // Use existing metrics to find fastest algorithm
                self.find_fastest_algorithm_from_metrics()
            }
            WorkloadType::StorageOptimized => {
                // Use existing metrics to find best compression ratio
                self.find_best_ratio_algorithm_from_metrics()
            }
            WorkloadType::Balanced => {
                // Use existing sophisticated scoring
                self.find_balanced_algorithm_from_metrics()
            }
            WorkloadType::Adaptive => {
                // Use current sophisticated algorithm selection
                self.algorithm.load()
            }
        }
    }

    /// Find fastest algorithm using existing AlgorithmMetrics
    fn find_fastest_algorithm_from_metrics(&self) -> CompressionAlgorithm {
        let mut fastest_algorithm = CompressionAlgorithm::Lz4; // Safe default
        let mut best_speed = 0.0f64;

        for entry in &self.algorithm_metrics {
            let algorithm = *entry.key(); // Dereference to get CompressionAlgorithm value
            let metrics = entry.value();
            if metrics.operation_count >= 5 && metrics.avg_compression_speed > best_speed {
                best_speed = metrics.avg_compression_speed;
                fastest_algorithm = algorithm;
            }
        }

        fastest_algorithm
    }

    /// Find best compression ratio using existing AlgorithmMetrics
    fn find_best_ratio_algorithm_from_metrics(&self) -> CompressionAlgorithm {
        let mut best_algorithm = CompressionAlgorithm::Zstd; // Safe default
        let mut best_ratio = 1.0f32;

        for entry in &self.algorithm_metrics {
            let algorithm = *entry.key(); // Dereference to get CompressionAlgorithm value
            let metrics = entry.value();
            if metrics.operation_count >= 5 && metrics.avg_compression_ratio < best_ratio {
                best_ratio = metrics.avg_compression_ratio;
                best_algorithm = algorithm;
            }
        }

        best_algorithm
    }

    /// Find balanced algorithm using existing sophisticated scoring
    fn find_balanced_algorithm_from_metrics(&self) -> CompressionAlgorithm {
        let mut best_algorithm = CompressionAlgorithm::Lz4; // Safe default
        let mut best_score = 0.0f64;

        for entry in &self.algorithm_metrics {
            let algorithm = *entry.key(); // Dereference to get CompressionAlgorithm value
            let metrics = entry.value();
            if metrics.operation_count >= 5 {
                let score = self.calculate_algorithm_score(metrics);
                if score > best_score {
                    best_score = score;
                    best_algorithm = algorithm;
                }
            }
        }

        best_algorithm
    }
}

impl CompressionStats {
    pub fn new() -> Self {
        Self {
            total_compressed: AtomicU64::new(0),
            total_uncompressed: AtomicU64::new(0),
            compression_ops: AtomicU64::new(0),
            decompression_ops: AtomicU64::new(0),
            total_compression_time_ns: AtomicU64::new(0),
            total_decompression_time_ns: AtomicU64::new(0),
        }
    }
}

/// Compression statistics snapshot
#[allow(dead_code)] // Cold tier - CompressionStatsSnapshot used in compression statistics reporting
#[derive(Debug, Clone)]
pub struct CompressionStatsSnapshot {
    pub total_compressed: u64,
    pub total_uncompressed: u64,
    pub compression_ops: u64,
    pub decompression_ops: u64,
    pub total_compression_time_ns: u64,
    pub total_decompression_time_ns: u64,
    pub current_algorithm: CompressionAlgorithm,
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
        self.total_uncompressed
            .saturating_sub(self.total_compressed)
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
            (self.total_uncompressed as f64 * 1_000_000_000.0)
                / self.total_compression_time_ns as f64
        } else {
            0.0
        }
    }

    /// Get decompression throughput (bytes per second)
    pub fn decompression_throughput(&self) -> f64 {
        if self.total_decompression_time_ns > 0 {
            (self.total_compressed as f64 * 1_000_000_000.0)
                / self.total_decompression_time_ns as f64
        } else {
            0.0
        }
    }
}

/// Enhanced compressed data structure with integrity verification
#[derive(Debug, Clone)]
pub struct CompressedData {
    pub data: Vec<u8>,
    pub original_size: usize,
    pub checksum: u32,
    pub algorithm: CompressionAlgorithm,
}

impl CompressedData {
    /// Get the compressed data length
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if compressed data is empty
    #[allow(dead_code)] // Cold tier - is_empty used in compressed data validation
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Compression error types
#[allow(dead_code)] // Cold tier - CompressionError used in compression error handling
#[derive(Debug, Clone)]
pub enum CompressionError {
    // Existing
    DecompressionFailed,
    UnsupportedAlgorithm,
    InvalidData,

    // NEW: Detailed error variants
    CompressionFailed(String),
    IntegrityCheckFailed { expected: u32, actual: u32 },
    InvalidCompressionLevel(i32),
    BackendNotAvailable(String),
    InsufficientBuffer { needed: usize, available: usize },
    CorruptedData(String),
}

impl std::fmt::Display for CompressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CompressionFailed(msg) => write!(f, "Compression failed: {}", msg),
            Self::IntegrityCheckFailed { expected, actual } => {
                write!(
                    f,
                    "Integrity check failed: expected {}, got {}",
                    expected, actual
                )
            }
            Self::InvalidCompressionLevel(level) => {
                write!(f, "Invalid compression level: {}", level)
            }
            Self::BackendNotAvailable(backend) => {
                write!(f, "Compression backend not available: {}", backend)
            }
            Self::InsufficientBuffer { needed, available } => {
                write!(
                    f,
                    "Insufficient buffer: need {} bytes, have {}",
                    needed, available
                )
            }
            Self::CorruptedData(msg) => write!(f, "Corrupted data: {}", msg),
            _ => write!(f, "{:?}", self),
        }
    }
}

impl std::error::Error for CompressionError {}
