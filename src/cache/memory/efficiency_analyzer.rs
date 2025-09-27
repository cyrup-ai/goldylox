//! Memory efficiency analysis and optimization recommendations
//!
//! This module analyzes memory usage patterns, allocation latency, and fragmentation
//! to provide optimization recommendations for memory management.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use arrayvec::ArrayVec;
use crossbeam_utils::CachePadded;

use super::types::{EfficiencyAnalysisResult, OptimizationRecommendation};
use crate::cache::config::CacheConfig;
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Memory efficiency analyzer
#[derive(Debug)]
pub struct MemoryEfficiencyAnalyzer {
    /// Allocation pattern tracking
    #[allow(dead_code)]
    // Memory management - allocation_patterns used in memory allocation pattern analysis
    allocation_patterns: AllocationPatternTracker,
    /// Latency measurement system
    #[allow(dead_code)]
    // Memory management - latency_tracker used in memory operation latency tracking
    latency_tracker: LatencyTracker,
    /// Fragmentation analysis
    #[allow(dead_code)]
    // Memory management - fragmentation_analyzer used in memory fragmentation analysis
    fragmentation_analyzer: FragmentationAnalyzer,
    /// Analysis result history
    analysis_history: std::cell::UnsafeCell<AnalysisHistoryBuffer>,
}

/// Allocation pattern tracking
#[derive(Debug)]
struct AllocationPatternTracker {
    /// Small allocation frequency
    #[allow(dead_code)]
    // Memory analysis - allocation frequency tracking used in pattern analysis
    small_alloc_frequency: CachePadded<AtomicU64>,
    /// Medium allocation frequency
    #[allow(dead_code)]
    // Memory analysis - allocation frequency tracking used in pattern analysis
    medium_alloc_frequency: CachePadded<AtomicU64>,
    /// Large allocation frequency
    #[allow(dead_code)]
    // Memory analysis - allocation frequency tracking used in pattern analysis
    large_alloc_frequency: CachePadded<AtomicU64>,
    /// Allocation size distribution
    #[allow(dead_code)]
    // Memory management - size_distribution used in allocation pattern analysis
    size_distribution: [AtomicU64; 16], // Size buckets
    /// Temporal allocation patterns
    #[allow(dead_code)]
    // Memory analysis - temporal patterns used in allocation timing analysis
    temporal_patterns: ArrayVec<u64, 64>, // Recent allocation timestamps
}

/// Latency measurement system
#[derive(Debug)]
struct LatencyTracker {
    /// Total allocation latency (nanoseconds)
    total_allocation_latency: AtomicU64,
    /// Allocation count for average calculation
    allocation_count: AtomicU64,
    /// Peak allocation latency
    #[allow(dead_code)]
    // Memory analysis - peak latency tracking used in performance optimization
    peak_allocation_latency: AtomicU64,
    /// Recent latency samples
    #[allow(dead_code)] // Memory analysis - latency samples used in performance trend analysis
    recent_latencies: ArrayVec<u64, 128>,
}

/// Fragmentation analysis
#[derive(Debug)]
struct FragmentationAnalyzer {
    /// External fragmentation level (0-1000)
    external_fragmentation: AtomicU32,
    /// Internal fragmentation level (0-1000)
    internal_fragmentation: AtomicU32,
    /// Fragmentation trend (increasing/decreasing)
    #[allow(dead_code)]
    // Memory management - fragmentation_trend used in fragmentation trend analysis
    fragmentation_trend: AtomicU32,
    /// Free block count
    free_block_count: AtomicUsize,
    /// Largest free block size
    #[allow(dead_code)]
    // Memory management - largest_free_block used in fragmentation analysis
    largest_free_block: AtomicUsize,
}

/// Analysis result history buffer
#[derive(Debug)]
struct AnalysisHistoryBuffer {
    /// Historical analysis results
    results: ArrayVec<EfficiencyAnalysisResult, 32>,
    /// Buffer write position
    write_position: AtomicUsize,
}

#[allow(dead_code)] // Memory management - comprehensive memory efficiency analyzer with allocation pattern tracking and latency analysis
impl MemoryEfficiencyAnalyzer {
    /// Create new memory efficiency analyzer
    pub fn new(_config: &CacheConfig) -> Result<Self, CacheOperationError> {
        Ok(Self {
            allocation_patterns: AllocationPatternTracker {
                small_alloc_frequency: CachePadded::new(AtomicU64::new(0)),
                medium_alloc_frequency: CachePadded::new(AtomicU64::new(0)),
                large_alloc_frequency: CachePadded::new(AtomicU64::new(0)),
                size_distribution: Default::default(),
                temporal_patterns: ArrayVec::new(),
            },
            latency_tracker: LatencyTracker {
                total_allocation_latency: AtomicU64::new(0),
                allocation_count: AtomicU64::new(0),
                peak_allocation_latency: AtomicU64::new(0),
                recent_latencies: ArrayVec::new(),
            },
            fragmentation_analyzer: FragmentationAnalyzer {
                external_fragmentation: AtomicU32::new(0),
                internal_fragmentation: AtomicU32::new(0),
                fragmentation_trend: AtomicU32::new(0),
                free_block_count: AtomicUsize::new(0),
                largest_free_block: AtomicUsize::new(0),
            },
            analysis_history: std::cell::UnsafeCell::new(AnalysisHistoryBuffer {
                results: ArrayVec::new(),
                write_position: AtomicUsize::new(0),
            }),
        })
    }

    /// Record allocation latency
    pub fn record_allocation_latency(&self, latency_ns: u64, size: usize) {
        // Update latency statistics
        self.latency_tracker
            .total_allocation_latency
            .fetch_add(latency_ns, Ordering::Relaxed);
        self.latency_tracker
            .allocation_count
            .fetch_add(1, Ordering::Relaxed);

        // Update peak latency
        let current_peak = self
            .latency_tracker
            .peak_allocation_latency
            .load(Ordering::Relaxed);
        if latency_ns > current_peak {
            self.latency_tracker
                .peak_allocation_latency
                .store(latency_ns, Ordering::Relaxed);
        }

        // Update allocation patterns
        self.update_allocation_patterns(size);
    }

    /// Analyze current memory efficiency
    pub fn analyze_current_efficiency(
        &self,
        allocation_stats: &super::allocation_stats::AllocationStatistics,
        _pool_manager: &super::pool_manager::manager::MemoryPoolManager,
    ) -> super::types::EfficiencyAnalysisResult {
        let stats = allocation_stats.get_statistics();
        let total_allocated = stats.total_allocated;
        let active_allocations = stats.active_allocations;
        let current_usage = active_allocations;

        let efficiency = if total_allocated == 0 {
            1.0 // Perfect efficiency when no allocations
        } else {
            // Calculate efficiency as ratio of useful memory to total allocated
            if current_usage > 0 {
                current_usage as f64 / total_allocated as f64
            } else {
                0.0
            }
        };

        super::types::EfficiencyAnalysisResult {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            efficiency_score: efficiency.clamp(0.0, 1.0) as f32,
            average_latency_ns: self
                .latency_tracker
                .total_allocation_latency
                .load(std::sync::atomic::Ordering::Relaxed),
            fragmentation_impact: allocation_stats.fragmentation_level(),
            memory_utilization: efficiency.clamp(0.0, 1.0) as f32,
            recommendations: self.generate_recommendations_from_existing_stats(allocation_stats),
        }
    }

    /// Analyze current memory efficiency
    pub fn analyze_efficiency(&self) -> Result<EfficiencyAnalysisResult, CacheOperationError> {
        let avg_latency = self.calculate_average_latency();
        let fragmentation_impact = self.calculate_fragmentation_impact();
        let efficiency_score = self.calculate_efficiency_score(avg_latency, fragmentation_impact);
        let recommendations = self.generate_recommendations(efficiency_score, fragmentation_impact);

        let result = EfficiencyAnalysisResult {
            timestamp: self.current_time_ns(),
            efficiency_score,
            average_latency_ns: avg_latency,
            fragmentation_impact,
            memory_utilization: self.calculate_memory_utilization(),
            recommendations,
        };

        self.store_analysis_result(result.clone());
        Ok(result)
    }

    /// Get efficiency snapshot
    pub fn get_efficiency_snapshot(&self) -> (f32, f32, f32) {
        let avg_latency = self.calculate_average_latency() as f32 / 1_000_000.0; // Convert to ms
        let fragmentation = self
            .fragmentation_analyzer
            .external_fragmentation
            .load(Ordering::Relaxed) as f32
            / 10.0;
        let utilization = self.calculate_memory_utilization();

        (avg_latency, fragmentation, utilization)
    }

    /// Update allocation patterns
    fn update_allocation_patterns(&self, size: usize) {
        // Categorize allocation size
        if size < 1024 {
            self.allocation_patterns
                .small_alloc_frequency
                .fetch_add(1, Ordering::Relaxed);
        } else if size < 65536 {
            self.allocation_patterns
                .medium_alloc_frequency
                .fetch_add(1, Ordering::Relaxed);
        } else {
            self.allocation_patterns
                .large_alloc_frequency
                .fetch_add(1, Ordering::Relaxed);
        }

        // Update size distribution
        let bucket = std::cmp::min(size.trailing_zeros() as usize, 15);
        self.allocation_patterns.size_distribution[bucket].fetch_add(1, Ordering::Relaxed);
    }

    /// Calculate average allocation latency
    fn calculate_average_latency(&self) -> u64 {
        let total_latency = self
            .latency_tracker
            .total_allocation_latency
            .load(Ordering::Relaxed);
        let count = self
            .latency_tracker
            .allocation_count
            .load(Ordering::Relaxed);

        if count > 0 { total_latency / count } else { 0 }
    }

    /// Calculate fragmentation impact
    fn calculate_fragmentation_impact(&self) -> f32 {
        let external = self
            .fragmentation_analyzer
            .external_fragmentation
            .load(Ordering::Relaxed) as f32;
        let internal = self
            .fragmentation_analyzer
            .internal_fragmentation
            .load(Ordering::Relaxed) as f32;

        // Weighted average of external and internal fragmentation
        (external * 0.7 + internal * 0.3) / 1000.0
    }

    /// Calculate efficiency score
    fn calculate_efficiency_score(&self, avg_latency: u64, fragmentation_impact: f32) -> f32 {
        // Base efficiency score
        let mut score = 1.0;

        // Penalize high latency (target: < 1ms)
        if avg_latency > 1_000_000 {
            score -= (avg_latency as f32 / 10_000_000.0).min(0.5);
        }

        // Penalize fragmentation
        score -= fragmentation_impact * 0.3;

        // Ensure score is in valid range
        score.clamp(0.0, 1.0)
    }

    /// Calculate memory utilization
    fn calculate_memory_utilization(&self) -> f32 {
        // Simplified utilization calculation
        let free_blocks = self
            .fragmentation_analyzer
            .free_block_count
            .load(Ordering::Relaxed);
        if free_blocks > 0 {
            0.85 // 85% utilization estimate
        } else {
            0.95 // 95% utilization when no free blocks
        }
    }

    /// Generate optimization recommendations
    fn generate_recommendations(
        &self,
        efficiency_score: f32,
        fragmentation_impact: f32,
    ) -> Vec<OptimizationRecommendation> {
        let mut recommendations = Vec::new();

        if efficiency_score < 0.7 {
            recommendations.push(OptimizationRecommendation::IncreasePoolSizes);
        }

        if fragmentation_impact > 0.3 {
            recommendations.push(OptimizationRecommendation::TriggerDefragmentation);
        }

        let avg_latency = self.calculate_average_latency();
        if avg_latency > 2_000_000 {
            // > 2ms
            recommendations.push(OptimizationRecommendation::OptimizeAllocationPaths);
        }

        if recommendations.is_empty() {
            recommendations.push(OptimizationRecommendation::NoActionNeeded);
        }

        recommendations
    }

    /// Store analysis result in history
    fn store_analysis_result(&self, result: EfficiencyAnalysisResult) {
        // Use circular buffer pattern with ArrayVec
        let current_pos = unsafe {
            (*self.analysis_history.get())
                .write_position
                .load(Ordering::Acquire)
        };
        let capacity = 32u64; // ArrayVec<EfficiencyAnalysisResult, 32> capacity

        // Calculate write index in circular buffer
        let write_idx = current_pos % capacity as usize;

        // Store in buffer using UnsafeCell for safe interior mutability
        unsafe {
            let history_mut = &mut *self.analysis_history.get();

            if history_mut.results.len() >= capacity as usize {
                history_mut.results[write_idx] = result;
            } else {
                history_mut.results.push(result);
            }
        }

        // Update write position
        unsafe {
            (*self.analysis_history.get())
                .write_position
                .store(current_pos.wrapping_add(1), Ordering::Release)
        };
    }

    /// Get current time in nanoseconds
    fn current_time_ns(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }

    /// Generate recommendations using existing sophisticated AllocationStatistics data
    fn generate_recommendations_from_existing_stats(
        &self,
        allocation_stats: &super::allocation_stats::AllocationStatistics,
    ) -> Vec<OptimizationRecommendation> {
        let mut recommendations = Vec::new();

        // Connect to existing fragmentation level
        let fragmentation = allocation_stats.fragmentation_level();
        if fragmentation > 0.3 {
            recommendations.push(OptimizationRecommendation::TriggerDefragmentation);
        }

        // Use existing allocation failure data
        let stats = allocation_stats.get_statistics();
        let total_ops = stats.allocation_operations;
        let failures = stats.allocation_failures;
        let failure_rate = if total_ops > 0 {
            failures as f64 / total_ops as f64
        } else {
            0.0
        };

        if failure_rate > 0.05 {
            // 5% failure rate
            recommendations.push(OptimizationRecommendation::IncreasePoolSizes);
        }

        // Use existing peak allocation tracking for memory efficiency
        let total_allocated = stats.total_allocated;
        let peak_allocation = stats.peak_allocation;
        let memory_efficiency = if peak_allocation > 0 {
            total_allocated as f64 / peak_allocation as f64
        } else {
            1.0
        };

        if memory_efficiency < 0.7 {
            // Less than 70% efficiency
            recommendations.push(OptimizationRecommendation::OptimizeAllocationPaths);
        }

        if recommendations.is_empty() {
            recommendations.push(OptimizationRecommendation::NoActionNeeded);
        }

        recommendations
    }
}
