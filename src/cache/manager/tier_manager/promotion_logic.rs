//! SIMD-optimized promotion evaluation logic
//!
//! This module implements the core promotion evaluation algorithms using
//! AVX2 intrinsics for parallel threshold comparison and scoring.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use crossbeam_utils::CachePadded;

use super::types::PromotionCriteria;
use crate::cache::traits::TierLocation;

impl PromotionCriteria {
    /// Create new SIMD-optimized promotion criteria
    #[inline]
    pub fn new() -> Self {
        Self {
            frequency_thresholds: CachePadded::new([
                AtomicU32::new(10),    // Threshold 0: Very low frequency
                AtomicU32::new(50),    // Threshold 1: Low frequency
                AtomicU32::new(100),   // Threshold 2: Medium frequency
                AtomicU32::new(500),   // Threshold 3: High frequency
                AtomicU32::new(1000),  // Threshold 4: Very high frequency
                AtomicU32::new(2000),  // Threshold 5: Ultra high frequency
                AtomicU32::new(5000),  // Threshold 6: Emergency promotion
                AtomicU32::new(10000), // Threshold 7: Maximum threshold
            ]),
            recency_weights: [1.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.3, 0.1],
            min_access_count: AtomicU32::new(5),
            frequency_multiplier: AtomicU32::new(100),
            simd_score_buffer: [0; 8],
        }
    }

    /// SIMD-optimized promotion evaluation using AVX2 intrinsics
    #[inline(always)]
    #[cfg(target_arch = "x86_64")]
    pub fn evaluate_promotion_simd(
        &self,
        access_count: u32,
        recency_score: f32,
        current_tier: TierLocation,
    ) -> bool {
        unsafe {
            // Load frequency thresholds into SIMD register
            let thresholds_ptr = self.frequency_thresholds.as_ptr() as *const u32;
            let thresholds = _mm256_load_si256(thresholds_ptr as *const __m256i);

            // Broadcast access count for parallel comparison
            let access_counts = _mm256_set1_epi32(access_count as i32);

            // Compare access count against all thresholds simultaneously
            let comparison = _mm256_cmpgt_epi32(access_counts, thresholds);

            // Extract comparison mask
            let mask = _mm256_movemask_epi8(comparison);

            // Calculate promotion score based on comparison results and recency
            let threshold_score = mask.count_ones() as u32;
            let frequency_mult = self.frequency_multiplier.load(Ordering::Relaxed);
            let min_access = self.min_access_count.load(Ordering::Relaxed);

            // Apply recency weighting and tier-specific thresholds
            let weighted_score = (threshold_score * frequency_mult) as f32 * recency_score;

            match current_tier {
                TierLocation::Cold => weighted_score > 300.0 && access_count >= min_access,
                TierLocation::Warm => weighted_score > 600.0 && access_count >= min_access * 2,
                TierLocation::Hot => false, // Already at highest tier
            }
        }
    }

    /// Non-SIMD fallback promotion evaluation
    #[inline(always)]
    #[cfg(not(target_arch = "x86_64"))]
    pub fn evaluate_promotion_simd(
        &self,
        access_count: u32,
        recency_score: f32,
        current_tier: TierLocation,
    ) -> bool {
        self.evaluate_promotion_scalar(access_count, recency_score, current_tier)
    }

    /// Scalar promotion evaluation for non-x86_64 targets
    #[inline(always)]
    pub fn evaluate_promotion_scalar(
        &self,
        access_count: u32,
        recency_score: f32,
        current_tier: TierLocation,
    ) -> bool {
        let min_access = self.min_access_count.load(Ordering::Relaxed);
        if access_count < min_access {
            return false;
        }

        let frequency_mult = self.frequency_multiplier.load(Ordering::Relaxed);
        let mut promotion_score = 0.0f32;

        // Calculate threshold crossings
        for (i, threshold) in self.frequency_thresholds.iter().enumerate() {
            if access_count > threshold.load(Ordering::Relaxed) {
                promotion_score += self.recency_weights[i] * frequency_mult as f32;
            }
        }

        // Apply recency weighting
        promotion_score *= recency_score;

        // Tier-specific promotion thresholds
        match current_tier {
            TierLocation::Cold => promotion_score > 300.0,
            TierLocation::Warm => promotion_score > 600.0,
            TierLocation::Hot => false,
        }
    }

    /// Update frequency thresholds based on observed access patterns
    #[inline]
    pub fn update_thresholds(&self, tier_stats: &[u32; 3]) {
        // Adaptive threshold adjustment based on tier utilization
        let total_entries = tier_stats.iter().sum::<u32>();
        if total_entries == 0 {
            return;
        }

        let hot_ratio = tier_stats[0] as f32 / total_entries as f32;

        // Adjust thresholds to maintain optimal tier ratios (Hot: 20%, Warm: 30%, Cold: 50%)
        if hot_ratio > 0.25 {
            // Too many entries in hot tier, increase thresholds
            for threshold in self.frequency_thresholds.iter() {
                let current = threshold.load(Ordering::Relaxed);
                threshold.store((current as f32 * 1.1) as u32, Ordering::Relaxed);
            }
        } else if hot_ratio < 0.15 {
            // Too few entries in hot tier, decrease thresholds
            for threshold in self.frequency_thresholds.iter() {
                let current = threshold.load(Ordering::Relaxed);
                threshold.store((current as f32 * 0.9) as u32, Ordering::Relaxed);
            }
        }
    }
}
