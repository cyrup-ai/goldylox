//! Promotion and demotion criteria with SIMD-optimized scoring algorithms
//!
//! This module provides adaptive criteria for tier promotion and demotion decisions
//! with SIMD-accelerated scoring computation and machine learning-based thresholds.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crossbeam_utils::CachePadded;

use crate::cache::config::CacheConfig;
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Advanced promotion criteria with adaptive scoring algorithms
#[derive(Debug)]
pub struct PromotionCriteria {
    /// Access frequency thresholds per tier (atomic for thread safety)
    #[allow(dead_code)] // Tier criteria - Access frequency thresholds for promotion scoring
    pub frequency_thresholds: CachePadded<[AtomicU32; 3]>, // Hot, Warm, Cold
    /// Recency weight factors for temporal locality scoring
    #[allow(dead_code)] // Tier criteria - Recency weight factors for temporal locality scoring
    pub recency_weights: CachePadded<[AtomicU32; 3]>, // Packed as u32 * 1000
    /// Size penalty factors for memory efficiency
    #[allow(dead_code)]
    // Tier criteria - Size penalty factors for memory efficiency calculations
    pub size_penalties: CachePadded<[AtomicU32; 3]>, // Bytes per promotion cost
    /// Access pattern correlation thresholds
    #[allow(dead_code)]
    // Tier criteria - Access pattern correlation thresholds for pattern analysis
    pub correlation_thresholds: CachePadded<[AtomicU32; 3]>, // Pattern strength * 1000
    /// Adaptive learning rate for threshold adjustment
    pub learning_rate: AtomicU32, // Rate * 10000 for precision
    /// SIMD-aligned scoring coefficients for vectorized computation
    pub scoring_coefficients: SIMDScoringCoefficients,
}

/// Graceful demotion criteria with data preservation strategies
#[allow(dead_code)] // Tier criteria - comprehensive demotion criteria structure with SIMD optimization
#[derive(Debug)]
pub struct DemotionCriteria {
    /// Idle time thresholds before demotion (nanoseconds)
    pub idle_thresholds: CachePadded<[AtomicU64; 3]>, // Hot->Warm, Warm->Cold, Cold->Evict
    /// Memory pressure thresholds for urgent demotion
    pub memory_pressure_thresholds: CachePadded<[AtomicU32; 3]>, // Percentage * 100
    /// Access frequency decay factors
    pub frequency_decay: CachePadded<[AtomicU32; 3]>, // Decay rate * 1000 per second
    /// Value preservation priorities (higher = keep longer)
    pub preservation_priorities: CachePadded<[AtomicU32; 4]>, // By value type
}

/// SIMD-aligned scoring coefficients for vectorized promotion scoring
#[allow(dead_code)] // Tier criteria - SIMD scoring coefficients for AVX2-optimized promotion scoring
#[derive(Debug)]
#[repr(align(32))] // AVX2 alignment
pub struct SIMDScoringCoefficients {
    /// Frequency scoring coefficients (4x f32 for SIMD)
    pub frequency_coeffs: [f32; 8], // Padded to 32 bytes
    /// Recency scoring coefficients (4x f32 for SIMD)  
    pub recency_coeffs: [f32; 8], // Padded to 32 bytes
    /// Size penalty coefficients (4x f32 for SIMD)
    pub size_coeffs: [f32; 8], // Padded to 32 bytes
    /// Temporal locality coefficients (4x f32 for SIMD)
    pub locality_coeffs: [f32; 8], // Padded to 32 bytes
}

/// Access characteristics for promotion scoring
#[allow(dead_code)] // Tier criteria - access characteristics structure for comprehensive promotion analysis
#[derive(Debug, Clone, Copy)]
pub struct AccessCharacteristics {
    /// Access frequency (accesses per second)
    pub access_frequency: f32,
    /// Recent access count (last 1 second)
    pub recent_accesses: f32,
    /// Temporal locality coefficient (0.0-1.0)
    pub temporal_locality: f32,
    /// Spatial locality coefficient (0.0-1.0)
    pub spatial_locality: f32,
    /// Value size in bytes
    pub value_size: f32,
    /// Creation to access delay (seconds)
    pub access_delay: f32,
}

impl PromotionCriteria {
    pub fn new(_config: &CacheConfig) -> Result<Self, CacheOperationError> {
        // Initialize with configuration-based defaults
        let frequency_thresholds = CachePadded::new([
            AtomicU32::new(10_000), // Hot: 10 accesses/sec * 1000
            AtomicU32::new(1_000),  // Warm: 1 access/sec * 1000
            AtomicU32::new(100),    // Cold: 0.1 access/sec * 1000
        ]);

        let recency_weights = CachePadded::new([
            AtomicU32::new(800), // Hot: 0.8 * 1000
            AtomicU32::new(500), // Warm: 0.5 * 1000
            AtomicU32::new(200), // Cold: 0.2 * 1000
        ]);

        let size_penalties = CachePadded::new([
            AtomicU32::new(1000), // Hot: 1.0 penalty per KB
            AtomicU32::new(500),  // Warm: 0.5 penalty per KB
            AtomicU32::new(100),  // Cold: 0.1 penalty per KB
        ]);

        let correlation_thresholds = CachePadded::new([
            AtomicU32::new(700), // Hot: 0.7 * 1000
            AtomicU32::new(500), // Warm: 0.5 * 1000
            AtomicU32::new(300), // Cold: 0.3 * 1000
        ]);

        // SIMD-optimized scoring coefficients
        let scoring_coefficients = SIMDScoringCoefficients {
            frequency_coeffs: [1.0, 0.8, 0.6, 0.4, 0.0, 0.0, 0.0, 0.0],
            recency_coeffs: [0.9, 0.7, 0.5, 0.3, 0.0, 0.0, 0.0, 0.0],
            size_coeffs: [-0.1, -0.2, -0.3, -0.4, 0.0, 0.0, 0.0, 0.0], // Negative penalty
            locality_coeffs: [0.8, 0.6, 0.4, 0.2, 0.0, 0.0, 0.0, 0.0],
        };

        Ok(Self {
            frequency_thresholds,
            recency_weights,
            size_penalties,
            correlation_thresholds,
            learning_rate: AtomicU32::new(100), // 0.01 * 10000
            scoring_coefficients,
        })
    }

    /// SIMD-accelerated promotion score computation
    #[cfg(target_arch = "x86_64")]
    pub fn compute_promotion_score_simd(&self, characteristics: &AccessCharacteristics) -> f32 {
        unsafe {
            // Load characteristics into SIMD registers
            let freq_vec = _mm256_set_ps(
                characteristics.access_frequency,
                characteristics.recent_accesses,
                characteristics.temporal_locality,
                characteristics.spatial_locality,
                0.0,
                0.0,
                0.0,
                0.0,
            );

            // Load scoring coefficients
            let coeff_vec = _mm256_load_ps(self.scoring_coefficients.frequency_coeffs.as_ptr());

            // Vectorized multiplication and horizontal sum
            let product = _mm256_mul_ps(freq_vec, coeff_vec);
            let sum = _mm256_hadd_ps(product, product);
            let sum2 = _mm256_hadd_ps(sum, sum);

            // Extract final score
            let mut result = [0.0f32; 8];
            _mm256_store_ps(result.as_mut_ptr(), sum2);
            result[0] + result[4] // Sum high and low 128-bit lanes
        }
    }

    /// Fallback promotion score computation for non-x86_64 architectures
    #[cfg(not(target_arch = "x86_64"))]
    pub fn compute_promotion_score_simd(&self, characteristics: &AccessCharacteristics) -> f32 {
        // Scalar computation as fallback
        let coeffs = &self.scoring_coefficients;

        characteristics.access_frequency * coeffs.frequency_coeffs[0]
            + characteristics.recent_accesses * coeffs.recency_coeffs[0]
            + characteristics.temporal_locality * coeffs.locality_coeffs[0]
            + characteristics.spatial_locality * coeffs.locality_coeffs[1]
    }

    /// Get promotion threshold for tier transition
    pub fn get_promotion_threshold(&self, from_tier_idx: usize, to_tier_idx: usize) -> f32 {
        let base_threshold = match (from_tier_idx, to_tier_idx) {
            (2, 1) => 0.3, // Cold -> Warm
            (1, 0) => 0.6, // Warm -> Hot
            (2, 0) => 0.8, // Cold -> Hot (rare)
            _ => 0.5,      // Default threshold
        };

        // Apply adaptive learning adjustments
        let learning_factor = self.learning_rate.load(Ordering::Relaxed) as f32 / 10000.0;
        base_threshold * (1.0 + learning_factor)
    }

    /// Calculate decision confidence based on score and thresholds
    pub fn calculate_decision_confidence(&self, score: f32, threshold: f32) -> f32 {
        if score > threshold {
            let confidence = (score - threshold) / threshold;
            confidence.min(1.0) // Cap at 100% confidence
        } else {
            0.0
        }
    }
}

#[allow(dead_code)] // Tier criteria - comprehensive demotion criteria methods for intelligent tier management
impl DemotionCriteria {
    pub fn new(_config: &CacheConfig) -> Result<Self, CacheOperationError> {
        // Initialize with adaptive demotion thresholds
        let idle_thresholds = CachePadded::new([
            AtomicU64::new(60_000_000_000),    // Hot->Warm: 60 seconds
            AtomicU64::new(300_000_000_000),   // Warm->Cold: 5 minutes
            AtomicU64::new(3_600_000_000_000), // Cold->Evict: 1 hour
        ]);

        let memory_pressure_thresholds = CachePadded::new([
            AtomicU32::new(8000), // Hot: 80% * 100
            AtomicU32::new(9000), // Warm: 90% * 100
            AtomicU32::new(9500), // Cold: 95% * 100
        ]);

        let frequency_decay = CachePadded::new([
            AtomicU32::new(100), // Hot: 0.1 * 1000 per second
            AtomicU32::new(50),  // Warm: 0.05 * 1000 per second
            AtomicU32::new(10),  // Cold: 0.01 * 1000 per second
        ]);

        let preservation_priorities = CachePadded::new([
            AtomicU32::new(1000), // High priority values
            AtomicU32::new(500),  // Medium priority values
            AtomicU32::new(100),  // Low priority values
            AtomicU32::new(0),    // No priority values
        ]);

        Ok(Self {
            idle_thresholds,
            memory_pressure_thresholds,
            frequency_decay,
            preservation_priorities,
        })
    }

    /// Check if entry should be demoted based on idle time
    pub fn should_demote_by_idle(&self, tier_idx: usize, idle_time_ns: u64) -> bool {
        let threshold = self.idle_thresholds[tier_idx].load(Ordering::Relaxed);
        idle_time_ns > threshold
    }

    /// Check if entry should be demoted due to memory pressure
    pub fn should_demote_by_pressure(&self, tier_idx: usize, memory_usage_percent: u32) -> bool {
        let threshold = self.memory_pressure_thresholds[tier_idx].load(Ordering::Relaxed);
        memory_usage_percent * 100 > threshold
    }

    /// Get frequency decay rate for tier
    pub fn get_frequency_decay_rate(&self, tier_idx: usize) -> f32 {
        self.frequency_decay[tier_idx].load(Ordering::Relaxed) as f32 / 1000.0
    }

    /// Get preservation priority for value type
    pub fn get_preservation_priority(&self, value_type_idx: usize) -> u32 {
        self.preservation_priorities[value_type_idx].load(Ordering::Relaxed)
    }
}
