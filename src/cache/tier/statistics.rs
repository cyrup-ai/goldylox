#![allow(dead_code)]
// Tier statistics - Complete tier statistics library with promotion/demotion tracking, performance analysis, and optimization metrics

//! Atomic promotion statistics for performance monitoring and analysis
//!
//! This module provides thread-safe statistics collection for tier promotion
//! operations with atomic counters and running averages.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crossbeam_utils::CachePadded;

use super::manager::tier_to_index;
use crate::cache::coherence::CacheTier;

/// Atomic promotion statistics for performance monitoring
#[derive(Debug)]
pub struct PromotionStatistics {
    /// Promotion counts per tier transition
    promotion_counts: CachePadded<[[AtomicU64; 3]; 3]>, // [from][to] tier matrix
    /// Demotion counts per tier transition  
    #[allow(dead_code)]
    // Tier statistics - Demotion count matrix for [from][to] tier transitions
    demotion_counts: CachePadded<[[AtomicU64; 3]; 3]>, // [from][to] tier matrix
    /// Average promotion latency per tier (nanoseconds)
    avg_promotion_latency: CachePadded<[AtomicU64; 3]>,
    /// Average demotion latency per tier (nanoseconds)
    #[allow(dead_code)] // Tier statistics - Average demotion latency tracking per tier
    avg_demotion_latency: CachePadded<[AtomicU64; 3]>,
    /// Successful vs failed promotion ratios
    #[allow(dead_code)]
    // Tier statistics - Promotion success rate tracking (rate * 1000 for precision)
    promotion_success_rate: CachePadded<[AtomicU32; 3]>, // Success rate * 1000
    /// Memory efficiency metrics per tier
    #[allow(dead_code)] // Tier statistics - Memory efficiency tracking (bytes per operation)
    memory_efficiency: CachePadded<[AtomicU32; 3]>, // Bytes per operation
}

/// Snapshot of promotion statistics for reporting
#[derive(Debug, Clone)]
pub struct PromotionStatsSnapshot {
    pub promotion_counts: [[u64; 3]; 3],
    pub demotion_counts: [[u64; 3]; 3],
    pub avg_promotion_latency: [u64; 3],
    pub avg_demotion_latency: [u64; 3],
    pub promotion_success_rate: [f32; 3],
    pub memory_efficiency: [u32; 3],
}

impl PromotionStatistics {
    pub fn new() -> Self {
        // Initialize all atomic counters to zero
        Self {
            promotion_counts: CachePadded::new(Default::default()),
            demotion_counts: CachePadded::new(Default::default()),
            avg_promotion_latency: CachePadded::new([
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ]),
            avg_demotion_latency: CachePadded::new([
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ]),
            promotion_success_rate: CachePadded::new([
                AtomicU32::new(1000), // 100% initial success rate
                AtomicU32::new(1000),
                AtomicU32::new(1000),
            ]),
            memory_efficiency: CachePadded::new([
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
            ]),
        }
    }

    /// Record successful promotion between tiers
    pub fn record_successful_promotion(
        &self,
        from_tier: CacheTier,
        to_tier: CacheTier,
        latency_ns: u64,
    ) {
        let from_idx = tier_to_index(from_tier);
        let to_idx = tier_to_index(to_tier);

        // Update promotion count atomically
        self.promotion_counts[from_idx][to_idx].fetch_add(1, Ordering::Relaxed);

        // Update running average latency
        self.update_running_average_latency(to_idx, latency_ns);
    }

    /// Record failed promotion attempt
    #[allow(dead_code)] // Tier statistics - Failed promotion tracking for success rate analysis
    pub fn record_failed_promotion(
        &self,
        from_tier: CacheTier,
        to_tier: CacheTier,
        latency_ns: u64,
    ) {
        let _from_idx = tier_to_index(from_tier);
        let to_idx = tier_to_index(to_tier);

        // Update failure statistics
        self.update_success_rate(to_idx, false);
        self.update_running_average_latency(to_idx, latency_ns);
    }

    /// Record successful demotion between tiers
    #[allow(dead_code)] // Tier statistics - Successful demotion tracking for tier transition analysis
    pub fn record_successful_demotion(
        &self,
        from_tier: CacheTier,
        to_tier: CacheTier,
        latency_ns: u64,
    ) {
        let from_idx = tier_to_index(from_tier);
        let to_idx = tier_to_index(to_tier);

        // Update demotion count atomically
        self.demotion_counts[from_idx][to_idx].fetch_add(1, Ordering::Relaxed);

        // Update running average demotion latency
        self.update_running_average_demotion_latency(from_idx, latency_ns);
    }

    /// Record decision latency for performance monitoring
    pub fn record_decision_latency(&self, tier: CacheTier, latency_ns: u64) {
        let idx = tier_to_index(tier);
        self.update_running_average_latency(idx, latency_ns);
    }

    /// Update memory efficiency metric
    #[allow(dead_code)] // Tier statistics - Memory efficiency tracking for performance analysis
    pub fn record_memory_efficiency(&self, tier: CacheTier, bytes_per_operation: u32) {
        let idx = tier_to_index(tier);
        self.memory_efficiency[idx].store(bytes_per_operation, Ordering::Relaxed);
    }

    /// Get total promotion count for a tier transition
    #[allow(dead_code)] // Tier statistics - Promotion count retrieval for monitoring and analysis
    pub fn get_promotion_count(&self, from_tier: CacheTier, to_tier: CacheTier) -> u64 {
        let from_idx = tier_to_index(from_tier);
        let to_idx = tier_to_index(to_tier);
        self.promotion_counts[from_idx][to_idx].load(Ordering::Relaxed)
    }

    /// Get total demotion count for a tier transition
    #[allow(dead_code)] // Tier statistics - Demotion count retrieval for monitoring and analysis
    pub fn get_demotion_count(&self, from_tier: CacheTier, to_tier: CacheTier) -> u64 {
        let from_idx = tier_to_index(from_tier);
        let to_idx = tier_to_index(to_tier);
        self.demotion_counts[from_idx][to_idx].load(Ordering::Relaxed)
    }

    /// Get average promotion latency for a tier
    #[allow(dead_code)] // Tier statistics - Average promotion latency retrieval for performance monitoring
    pub fn get_avg_promotion_latency(&self, tier: CacheTier) -> u64 {
        let idx = tier_to_index(tier);
        self.avg_promotion_latency[idx].load(Ordering::Relaxed)
    }

    /// Get promotion success rate for a tier
    #[allow(dead_code)] // Tier statistics - Promotion success rate retrieval for performance analysis
    pub fn get_promotion_success_rate(&self, tier: CacheTier) -> f32 {
        let idx = tier_to_index(tier);
        self.promotion_success_rate[idx].load(Ordering::Relaxed) as f32 / 1000.0
    }

    /// Get memory efficiency for a tier
    #[allow(dead_code)] // Tier statistics - Memory efficiency retrieval for performance analysis
    pub fn get_memory_efficiency(&self, tier: CacheTier) -> u32 {
        let idx = tier_to_index(tier);
        self.memory_efficiency[idx].load(Ordering::Relaxed)
    }

    /// Get complete statistics snapshot
    pub fn snapshot(&self) -> PromotionStatsSnapshot {
        let mut promotion_counts = [[0u64; 3]; 3];
        let mut demotion_counts = [[0u64; 3]; 3];

        // Copy promotion counts
        for from_idx in 0..3 {
            for to_idx in 0..3 {
                promotion_counts[from_idx][to_idx] =
                    self.promotion_counts[from_idx][to_idx].load(Ordering::Relaxed);
                demotion_counts[from_idx][to_idx] =
                    self.demotion_counts[from_idx][to_idx].load(Ordering::Relaxed);
            }
        }

        // Copy other metrics
        let avg_promotion_latency = [
            self.avg_promotion_latency[0].load(Ordering::Relaxed),
            self.avg_promotion_latency[1].load(Ordering::Relaxed),
            self.avg_promotion_latency[2].load(Ordering::Relaxed),
        ];

        let avg_demotion_latency = [
            self.avg_demotion_latency[0].load(Ordering::Relaxed),
            self.avg_demotion_latency[1].load(Ordering::Relaxed),
            self.avg_demotion_latency[2].load(Ordering::Relaxed),
        ];

        let promotion_success_rate = [
            self.promotion_success_rate[0].load(Ordering::Relaxed) as f32 / 1000.0,
            self.promotion_success_rate[1].load(Ordering::Relaxed) as f32 / 1000.0,
            self.promotion_success_rate[2].load(Ordering::Relaxed) as f32 / 1000.0,
        ];

        let memory_efficiency = [
            self.memory_efficiency[0].load(Ordering::Relaxed),
            self.memory_efficiency[1].load(Ordering::Relaxed),
            self.memory_efficiency[2].load(Ordering::Relaxed),
        ];

        PromotionStatsSnapshot {
            promotion_counts,
            demotion_counts,
            avg_promotion_latency,
            avg_demotion_latency,
            promotion_success_rate,
            memory_efficiency,
        }
    }

    /// Reset all statistics to initial values
    pub fn reset(&self) {
        // Reset promotion counts
        for from_idx in 0..3 {
            for to_idx in 0..3 {
                self.promotion_counts[from_idx][to_idx].store(0, Ordering::Relaxed);
                self.demotion_counts[from_idx][to_idx].store(0, Ordering::Relaxed);
            }
        }

        // Reset latencies
        for idx in 0..3 {
            self.avg_promotion_latency[idx].store(0, Ordering::Relaxed);
            self.avg_demotion_latency[idx].store(0, Ordering::Relaxed);
            self.promotion_success_rate[idx].store(1000, Ordering::Relaxed); // 100%
            self.memory_efficiency[idx].store(0, Ordering::Relaxed);
        }
    }

    /// Update running average promotion latency using exponential moving average
    fn update_running_average_latency(&self, tier_idx: usize, new_latency: u64) {
        let current_avg = self.avg_promotion_latency[tier_idx].load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            new_latency
        } else {
            // Exponential moving average with alpha = 0.1
            (current_avg * 9 + new_latency) / 10
        };
        self.avg_promotion_latency[tier_idx].store(new_avg, Ordering::Relaxed);
    }

    /// Update running average demotion latency
    fn update_running_average_demotion_latency(&self, tier_idx: usize, new_latency: u64) {
        let current_avg = self.avg_demotion_latency[tier_idx].load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            new_latency
        } else {
            // Exponential moving average with alpha = 0.1
            (current_avg * 9 + new_latency) / 10
        };
        self.avg_demotion_latency[tier_idx].store(new_avg, Ordering::Relaxed);
    }

    /// Update success rate using exponential moving average
    fn update_success_rate(&self, tier_idx: usize, success: bool) {
        let current_rate = self.promotion_success_rate[tier_idx].load(Ordering::Relaxed);
        let success_value = if success { 1000 } else { 0 }; // 100% or 0%

        let new_rate = if current_rate == 1000 && success {
            1000 // Keep at 100% if still successful
        } else {
            // Exponential moving average with alpha = 0.1
            (current_rate * 9 + success_value) / 10
        };

        self.promotion_success_rate[tier_idx].store(new_rate, Ordering::Relaxed);
    }
}

impl Default for PromotionStatistics {
    fn default() -> Self {
        Self::new()
    }
}
