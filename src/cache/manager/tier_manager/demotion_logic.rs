//! SIMD-optimized demotion evaluation logic
//!
//! This module implements the core demotion evaluation algorithms using
//! SSE intrinsics for parallel pressure threshold comparison and scoring.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;
use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicU32, AtomicU64};

use crossbeam_utils::CachePadded;

use super::types::DemotionCriteria;
use crate::cache::traits::TierLocation;

impl DemotionCriteria {
    /// Create new SIMD-optimized demotion criteria
    #[inline]
    pub fn new() -> Self {
        Self {
            pressure_thresholds: CachePadded::new([
                AtomicU32::new(70), // Low pressure threshold (70% capacity)
                AtomicU32::new(80), // Medium pressure threshold
                AtomicU32::new(90), // High pressure threshold
                AtomicU32::new(95), // Critical pressure threshold
            ]),
            age_weights: [1.0, 0.8, 0.6, 0.4, 0.2, 0.1, 0.05, 0.01],
            max_idle_time: AtomicU64::new(300_000_000_000), // 5 minutes in nanoseconds
            memory_pressure_threshold: AtomicU32::new(85),  // 85% memory usage
            simd_pressure_buffer: [0; 8],
        }
    }

    /// SIMD-optimized demotion evaluation
    #[inline(always)]
    #[cfg(target_arch = "x86_64")]
    pub fn evaluate_demotion_simd(
        &self,
        idle_time_ns: u64,
        memory_pressure: u32,
        tier_utilization: u32,
        current_tier: TierLocation,
    ) -> bool {
        unsafe {
            // Load pressure thresholds into SIMD register
            let pressure_ptr = self.pressure_thresholds.as_ptr() as *const u32;
            let thresholds = _mm_load_si128(pressure_ptr as *const __m128i);

            // Broadcast current utilization for comparison
            let utilization = _mm_set1_epi32(tier_utilization as i32);

            // Compare utilization against pressure thresholds
            let pressure_comparison = _mm_cmpgt_epi32(utilization, thresholds);
            let pressure_mask = _mm_movemask_epi8(pressure_comparison);

            // Calculate pressure score
            let pressure_score = pressure_mask.count_ones();

            // Check age-based demotion criteria
            let max_idle = self.max_idle_time.load(Ordering::Relaxed);
            let memory_threshold = self.memory_pressure_threshold.load(Ordering::Relaxed);

            let idle_factor = if idle_time_ns > max_idle { 2.0 } else { 1.0 };
            let memory_factor = if memory_pressure > memory_threshold {
                1.5
            } else {
                1.0
            };

            let demotion_score = pressure_score as f32 * idle_factor * memory_factor;

            match current_tier {
                TierLocation::Hot => demotion_score > 2.0,
                TierLocation::Warm => demotion_score > 3.0,
                TierLocation::Cold => false, // Already at lowest tier
            }
        }
    }

    /// Non-SIMD fallback demotion evaluation
    #[inline(always)]
    #[cfg(not(target_arch = "x86_64"))]
    pub fn evaluate_demotion_simd(
        &self,
        idle_time_ns: u64,
        memory_pressure: u32,
        tier_utilization: u32,
        current_tier: TierLocation,
    ) -> bool {
        let max_idle = self.max_idle_time.load(Ordering::Relaxed);
        let memory_threshold = self.memory_pressure_threshold.load(Ordering::Relaxed);

        let idle_factor = if idle_time_ns > max_idle { 2.0 } else { 1.0 };
        let memory_factor = if memory_pressure > memory_threshold {
            1.5
        } else {
            1.0
        };

        // Count pressure threshold crossings
        let mut pressure_score = 0;
        for threshold in self.pressure_thresholds.iter() {
            if tier_utilization > threshold.load(Ordering::Relaxed) {
                pressure_score += 1;
            }
        }

        let demotion_score = pressure_score as f32 * idle_factor * memory_factor;

        match current_tier {
            TierLocation::Hot => demotion_score > 2.0,
            TierLocation::Warm => demotion_score > 3.0,
            TierLocation::Cold => false,
        }
    }
}
