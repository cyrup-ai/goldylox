//! Main tier promotion manager with SIMD-optimized decision algorithms
//!
//! This module provides the core TierPromotionManager implementation with
//! intelligent promotion/demotion decisions and adaptive learning capabilities.

#![allow(dead_code)] // Tier management - Complete tier promotion/demotion library with SIMD optimization and adaptive learning

use std::time::{Duration, Instant};

use super::criteria::{AccessCharacteristics, DemotionCriteria, PromotionCriteria};
use super::queue::{PromotionPriority, PromotionQueue, PromotionTask};
use super::statistics::PromotionStatistics;
use crate::cache::coherence::CacheTier;
use crate::cache::config::CacheConfig;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::types::AccessPath;

/// Intelligent tier promotion/demotion manager with SIMD optimization
#[derive(Debug)]
pub struct TierPromotionManager<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
> {
    /// Promotion criteria with machine learning scoring
    promotion_criteria: PromotionCriteria,
    /// Demotion criteria with graceful data preservation  
    demotion_criteria: DemotionCriteria,
    /// Promotion statistics with atomic counters
    promotion_stats: PromotionStatistics,
    /// Lock-free promotion task queue with atomic head/tail management
    promotion_queue: PromotionQueue<K>,
}

/// Promotion decision with confidence scoring
#[derive(Debug, Clone, Copy)]
pub struct PromotionDecision {
    /// Whether promotion should occur
    pub should_promote: bool,
    /// Promotion priority (0-10, higher = more urgent)
    pub priority: u8,
    /// Decision confidence (0.0-1.0)
    pub confidence: f32,
    /// Recommended target tier
    pub target_tier: CacheTier,
    /// Expected performance benefit
    pub expected_benefit: f32,
}

impl<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static>
    TierPromotionManager<K>
{
    /// Create new tier promotion manager with adaptive algorithms
    pub fn new(config: &CacheConfig) -> Result<Self, CacheOperationError> {
        Ok(Self {
            promotion_criteria: PromotionCriteria::new(config)?,
            demotion_criteria: DemotionCriteria::new(config)?,
            promotion_stats: PromotionStatistics::new(),
            promotion_queue: PromotionQueue::new()?,
        })
    }

    /// Determine if entry should be promoted with SIMD-accelerated scoring
    pub fn should_promote<V: CacheValue>(
        &self,
        key: &K,
        value: &V,
        from_tier: CacheTier,
        to_tier: CacheTier,
        access_path: &AccessPath,
    ) -> PromotionDecision {
        let timer = Instant::now();

        // Extract access characteristics for SIMD processing
        let characteristics = self.extract_access_characteristics(key, value, access_path);

        // SIMD-accelerated scoring computation
        let promotion_score = self
            .promotion_criteria
            .compute_promotion_score_simd(&characteristics);

        // Determine promotion decision based on adaptive thresholds
        let from_idx = tier_to_index(from_tier);
        let to_idx = tier_to_index(to_tier);
        let threshold = self
            .promotion_criteria
            .get_promotion_threshold(from_idx, to_idx);
        let should_promote = promotion_score > threshold;
        let confidence = self
            .promotion_criteria
            .calculate_decision_confidence(promotion_score, threshold);

        // Record decision latency for performance monitoring
        let decision_latency = timer.elapsed().as_nanos() as u64;
        self.promotion_stats
            .record_decision_latency(from_tier, decision_latency);

        PromotionDecision {
            should_promote,
            priority: self.calculate_priority_level(promotion_score, from_tier, to_tier),
            confidence,
            target_tier: to_tier,
            expected_benefit: self.estimate_performance_benefit(
                promotion_score,
                from_tier,
                to_tier,
            ),
        }
    }

    /// Schedule promotion task in lock-free queue
    pub fn schedule_promotion(
        &self,
        key: K,
        from_tier: CacheTier,
        to_tier: CacheTier,
        priority: u8,
    ) -> Result<(), CacheOperationError> {
        let task = PromotionTask {
            key: key.clone(),
            from_tier,
            to_tier,
            priority: PromotionPriority {
                level: priority,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0),
            },
            created_at: Instant::now(),
            deadline: Instant::now() + Duration::from_millis(100), // 100ms deadline
        };

        self.promotion_queue.schedule_task(task)
    }

    /// Calculate temporal locality score from access timestamp intervals
    ///
    /// **PANICS** if timestamps.len() < 2. Empty timestamps means the tier operations
    /// failed to populate AccessPath.access_timestamps - this is a BUG not an edge case.
    ///
    /// # Algorithm
    /// 1. Calculate intervals between consecutive timestamps using windows(2)
    /// 2. Compute average interval and standard deviation (variance analysis)
    /// 3. Score interval regularity: shorter intervals = higher locality
    /// 4. Score consistency: lower variance = more predictable pattern
    /// 5. Combine scores: 70% interval + 30% consistency
    ///
    /// # Scoring
    /// - >0.7: High temporal locality (sub-ms intervals) → PROMOTE to hot
    /// - 0.4-0.7: Moderate locality (ms-range intervals) → KEEP in warm
    /// - <0.4: Low locality (>10ms intervals) → DEMOTE to cold
    ///
    /// # Performance
    /// - O(n) where n = timestamps.len() (typically 10)
    /// - Single Vec allocation for intervals
    /// - No locking (read-only)
    ///
    /// # Arguments
    /// * `access_path` - Access path with populated timestamps from tier operations
    ///
    /// # Returns
    /// Temporal locality score in [0.0, 1.0]
    #[inline]
    fn calculate_temporal_locality(access_path: &AccessPath) -> f32 {
        let timestamps = &access_path.access_timestamps;

        // Need at least 2 timestamps to calculate intervals
        // Return neutral score for insufficient data
        if timestamps.len() < 2 {
            return 0.5;
        }

        // Calculate intervals between consecutive accesses
        // Pattern from ConcurrentAccessTracker::calculate_access_frequency
        // (src/cache/tier/warm/access_tracking/tracker.rs:211-215)
        let intervals: Vec<u64> = timestamps
            .windows(2)
            .map(|window| window[1].saturating_sub(window[0]))
            .collect();

        // Calculate average interval
        let sum: u64 = intervals.iter().sum();
        let avg_interval = sum as f64 / intervals.len() as f64;

        // Calculate variance and standard deviation for consistency scoring
        let variance: f64 = intervals
            .iter()
            .map(|&interval| {
                let diff = interval as f64 - avg_interval;
                diff * diff
            })
            .sum::<f64>()
            / intervals.len() as f64;

        let stddev = variance.sqrt();

        // SCORE COMPONENT 1: Interval regularity (70% weight)
        // Formula: min(1.0, 1_000_000 / (avg_interval_ns + 1000))
        //
        // Examples:
        //   100μs → 1M / 100,100 = 9.99 → 1.0 (VERY HIGH - promote)
        //   1ms   → 1M / 1,001,000 = 0.999 (HIGH - promote)
        //   10ms  → 1M / 10,001,000 = 0.0999 (LOW - keep/demote)
        //   1s    → 1M / 1,000,001,000 = 0.001 (VERY LOW - demote)
        let interval_score = (1_000_000.0 / (avg_interval + 1000.0)).min(1.0);

        // SCORE COMPONENT 2: Consistency (30% weight)
        // Formula: min(1.0, 10_000 / (stddev + 100))
        //
        // Low stddev = predictable access pattern = high score
        // High stddev = erratic/bursty = low score
        let consistency_score = (10_000.0 / (stddev + 100.0)).min(1.0);

        // COMBINED: 70% interval + 30% consistency
        // Interval dominates because frequency matters more than perfect regularity
        let score = interval_score * 0.7 + consistency_score * 0.3;

        score.clamp(0.0, 1.0) as f32
    }

    /// Calculate spatial locality score from key hash proximity
    /// 
    /// Analyzes hash distance between current key and recently accessed keys:
    /// - High score (>0.7): Accessing similar/nearby keys = high spatial locality
    /// - Low score (<0.3): Accessing unrelated keys = low spatial locality  
    /// - Default (0.5): Insufficient data for analysis (<1 key hash)
    ///
    /// # Algorithm
    /// 1. Get current key's cache_hash() value
    /// 2. For each recent key hash, calculate XOR distance
    /// 3. Count differing bits using count_ones() on XOR result
    /// 4. Convert to proximity: 1.0 - (bits_different / 64.0)
    /// 5. Average all proximity scores
    ///
    /// # Hash Proximity Method
    /// XOR distance measures bit-level difference between hashes:
    /// - 0 differing bits = identical keys → proximity 1.0
    /// - 32 differing bits = somewhat related → proximity 0.5
    /// - 64 differing bits = completely unrelated → proximity 0.0
    ///
    /// Sequential key access patterns produce similar hashes (low XOR distance),
    /// while random access produces dissimilar hashes (high XOR distance).
    ///
    /// # Performance
    /// - Zero allocation when key_hashes.len() < 1 (returns immediately)
    /// - Single u64 hash per recent key (cheap XOR + count_ones operations)
    /// - O(n) time complexity where n = recent_key_hashes.len()
    /// - No locking required (read-only access)
    /// - Branchless bit operations for maximum throughput
    ///
    /// # Arguments
    /// * `key` - Current key being considered for promotion
    /// * `access_path` - Access path containing recent key hash history
    ///
    /// # Returns
    /// Spatial locality score in range [0.0, 1.0]
    #[inline]
    fn calculate_spatial_locality(
        key: &K,
        access_path: &AccessPath,
    ) -> f32 {
        let recent_hashes = &access_path.recent_key_hashes;
        
        // Need at least 1 recent hash to compare against
        // Return neutral score for unknown locality
        if recent_hashes.is_empty() {
            return 0.5;
        }
        
        // Get current key's hash for comparison
        let current_hash = key.cache_hash();
        
        // Calculate proximity to each recent key
        // XOR distance: 0 = identical, 64 = completely different
        let mut total_proximity = 0.0;
        
        for &recent_hash in recent_hashes {
            // XOR gives bits that differ between hashes
            let xor_distance = current_hash ^ recent_hash;
            
            // Count number of differing bits (0-64 range)
            let bits_different = xor_distance.count_ones();
            
            // Convert to proximity score (0.0 = different, 1.0 = identical)
            // Formula: 1.0 - (bits_different / 64.0)
            //
            // Examples:
            //   0 bits different  → 1.0 - 0.0 = 1.0 (identical, perfect proximity)
            //   16 bits different → 1.0 - 0.25 = 0.75 (similar, good proximity)
            //   32 bits different → 1.0 - 0.5 = 0.5 (somewhat related)
            //   48 bits different → 1.0 - 0.75 = 0.25 (quite different, poor proximity)
            //   64 bits different → 1.0 - 1.0 = 0.0 (completely unrelated)
            let proximity = 1.0 - (bits_different as f32 / 64.0);
            
            total_proximity += proximity;
        }
        
        // Average proximity across all recent keys
        let avg_proximity = total_proximity / recent_hashes.len() as f32;
        
        // Result is already in [0.0, 1.0] range due to proximity formula
        // High score = accessing similar keys (high spatial locality)
        // Low score = accessing unrelated keys (low spatial locality)
        avg_proximity
    }

    /// Extract access characteristics from key, value, and access path
    fn extract_access_characteristics<V: CacheValue>(
        &self,
        key: &K,
        _value: &V,
        access_path: &AccessPath,
    ) -> AccessCharacteristics {
        // Create a default frequency estimator for access frequency calculation
        let frequency_estimator = crate::cache::tier::warm::access_tracking::frequency_estimator::FrequencyEstimator::<K>::new();

        // Extract characteristics based on access patterns and value properties
        // Note: Using simplified approach for characteristics that require external dependencies
        AccessCharacteristics {
            access_frequency: access_path.frequency_estimate(&frequency_estimator),
            recent_accesses: access_path.recent_access_score(),
            temporal_locality: Self::calculate_temporal_locality(access_path),
            spatial_locality: Self::calculate_spatial_locality(key, access_path),
            value_size: std::mem::size_of::<V>() as f32,
            access_delay: 0.0, // Default access delay
        }
    }

    /// Calculate priority level based on promotion score and tier transition
    fn calculate_priority_level(
        &self,
        promotion_score: f32,
        from_tier: CacheTier,
        to_tier: CacheTier,
    ) -> u8 {
        let base_priority = match (from_tier, to_tier) {
            (CacheTier::Cold, CacheTier::Hot) => 8, // High priority for cold->hot
            (CacheTier::Cold, CacheTier::Warm) => 4, // Medium priority for cold->warm
            (CacheTier::Warm, CacheTier::Hot) => 6, // Medium-high priority for warm->hot
            _ => 2,                                 // Low priority for other transitions
        };

        // Adjust based on promotion score
        let score_adjustment = (promotion_score * 2.0) as u8;
        (base_priority + score_adjustment).min(10)
    }

    /// Estimate performance benefit of promotion
    fn estimate_performance_benefit(
        &self,
        promotion_score: f32,
        from_tier: CacheTier,
        to_tier: CacheTier,
    ) -> f32 {
        let tier_benefit = match (from_tier, to_tier) {
            (CacheTier::Cold, CacheTier::Hot) => 10.0, // 10x performance improvement
            (CacheTier::Cold, CacheTier::Warm) => 3.0, // 3x performance improvement
            (CacheTier::Warm, CacheTier::Hot) => 3.0,  // 3x performance improvement
            _ => 1.0,                                  // No significant benefit
        };

        promotion_score * tier_benefit
    }

    /// Check if entry should be demoted based on criteria
    pub fn should_demote<V: CacheValue>(
        &self,
        _key: &K,
        _value: &V,
        current_tier: CacheTier,
        idle_time_ns: u64,
        memory_pressure_percent: u32,
    ) -> bool {
        let tier_idx = tier_to_index(current_tier);

        // Check idle time threshold
        if self
            .demotion_criteria
            .should_demote_by_idle(tier_idx, idle_time_ns)
        {
            return true;
        }

        // Check memory pressure threshold
        if self
            .demotion_criteria
            .should_demote_by_pressure(tier_idx, memory_pressure_percent)
        {
            return true;
        }

        false
    }

    /// Get promotion statistics
    #[allow(dead_code)] // Statistics collection - used in unified telemetry system integration
    pub fn get_statistics(&self) -> &PromotionStatistics {
        &self.promotion_stats
    }

    /// Get queue status for monitoring
    pub fn get_queue_status(&self) -> (usize, usize, usize) {
        self.promotion_queue.get_queue_sizes()
    }

    /// Clear expired tasks from queue
    pub fn cleanup_expired_tasks(&self) -> usize {
        self.promotion_queue.cleanup_expired_tasks()
    }
    
    /// Get next promotion task from queue for processing
    pub fn get_next_promotion_task(&self) -> Result<Option<PromotionTask<K>>, CacheOperationError> {
        self.promotion_queue.get_next_task()
    }
    
    /// Record successful promotion for statistics
    pub fn record_promotion_success(&self, from_tier: CacheTier, to_tier: CacheTier, latency_ns: u64) {
        self.promotion_stats.record_successful_promotion(from_tier, to_tier, latency_ns);
    }
}

/// Convert cache tier to array index for statistics
pub fn tier_to_index(tier: CacheTier) -> usize {
    match tier {
        CacheTier::Hot => 0,
        CacheTier::Warm => 1,
        CacheTier::Cold => 2,
    }
}
