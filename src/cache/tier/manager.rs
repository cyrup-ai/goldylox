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

    /// Process next promotion task from queue (work-stealing compatible)
    /// NOTE: This method is now generic and requires explicit type parameters
    pub fn process_next_promotion<
        V: CacheValue
            + Default
            + serde::Serialize
            + serde::de::DeserializeOwned
            + bincode::Encode
            + bincode::Decode<()>
            + 'static,
    >(
        &self,
        key: &K,
    ) -> Result<Option<PromotionTask<K>>, CacheOperationError> {
        if let Some(task) = self.promotion_queue.get_next_task()? {
            // Execute promotion with atomic coordination
            self.execute_promotion::<V>(&task, key)?;

            // Update statistics atomically
            self.promotion_stats.record_successful_promotion(
                task.from_tier,
                task.to_tier,
                task.created_at.elapsed().as_nanos() as u64,
            );

            return Ok(Some(task));
        }

        Ok(None)
    }

    /// Execute promotion with coherence protocol coordination
    fn execute_promotion<
        V: CacheValue
            + Default
            + serde::Serialize
            + serde::de::DeserializeOwned
            + bincode::Encode
            + bincode::Decode<()>
            + 'static,
    >(
        &self,
        task: &PromotionTask<K>,
        key: &K,
    ) -> Result<(), CacheOperationError> {
        // Read value from source tier first
        let value = self.read_from_tier::<V>(key, task.from_tier)?;

        if let Some(cached_value) = value {
            // Write to destination tier with coherence protocol
            self.write_to_tier(key.clone(), cached_value, task.to_tier)?;

            // Remove from source tier to avoid duplication and maintain consistency - FIXED: Now properly generic
            let _ = self.remove_from_tier::<V>(key, task.from_tier);

            // Update promotion statistics
            self.promotion_stats.record_successful_promotion(
                task.from_tier,
                task.to_tier,
                task.created_at.elapsed().as_nanos() as u64,
            );

            Ok(())
        } else {
            // Value not found in source tier - this could happen if it was evicted
            // between scheduling and execution
            Err(CacheOperationError::KeyNotFound)
        }
    }

    /// Read value from specific tier with error handling
    fn read_from_tier<
        V: CacheValue
            + Default
            + serde::Serialize
            + serde::de::DeserializeOwned
            + bincode::Encode
            + bincode::Decode<()>
            + 'static,
    >(
        &self,
        key: &K,
        tier: CacheTier,
    ) -> Result<Option<V>, CacheOperationError> {
        match tier {
            CacheTier::Hot => {
                // For hot tier, use the worker-based routing system
                Ok(super::hot::simd_hot_get::<K, V>(key))
            }
            CacheTier::Warm => {
                // For warm tier, use direct access
                Ok(super::warm::warm_get::<K, V>(key))
            }
            CacheTier::Cold => {
                // For cold tier, use the Result-based API and convert to Option
                match super::cold::cold_get::<K, V>(key) {
                    Ok(value_opt) => Ok(value_opt),
                    Err(e) => Err(e),
                }
            }
        }
    }

    /// Write value to specific tier with coherence protocol
    fn write_to_tier<
        V: CacheValue
            + Default
            + serde::Serialize
            + serde::de::DeserializeOwned
            + bincode::Encode
            + bincode::Decode<()>
            + 'static,
    >(
        &self,
        key: K,
        value: V,
        tier: CacheTier,
    ) -> Result<(), CacheOperationError> {
        match tier {
            CacheTier::Hot => {
                // For hot tier, use the worker-based routing system
                super::hot::simd_hot_put(key, value)
            }
            CacheTier::Warm => {
                // For warm tier, use direct access
                super::warm::warm_put(key, value)
            }
            CacheTier::Cold => {
                // For cold tier, use the Result-based API
                super::cold::insert_demoted(key, value)
            }
        }
    }

    /// Remove value from specific tier - FIXED: Now properly generic over both K and V
    pub fn remove_from_tier<
        V: CacheValue
            + Default
            + serde::Serialize
            + serde::de::DeserializeOwned
            + bincode::Encode
            + bincode::Decode<()>
            + 'static,
    >(
        &self,
        key: &K,
        tier: CacheTier,
    ) -> Result<bool, CacheOperationError> {
        match tier {
            CacheTier::Hot => {
                // For hot tier, use the worker-based routing system - FIXED: Now properly generic
                match super::hot::simd_hot_remove::<K, V>(key) {
                    Ok(Some(_)) => Ok(true),
                    Ok(None) => Ok(false),
                    Err(e) => Err(e),
                }
            }
            CacheTier::Warm => {
                // For warm tier, use direct access - FIXED: Now properly generic
                match super::warm::warm_remove::<K, V>(key) {
                    Some(_) => Ok(true),
                    None => Ok(false),
                }
            }
            CacheTier::Cold => {
                // For cold tier, use the Result-based API
                super::cold::remove_entry::<K, V>(key)
            }
        }
    }

    /// Extract access characteristics from key, value, and access path
    fn extract_access_characteristics<V: CacheValue>(
        &self,
        _key: &K,
        _value: &V,
        access_path: &AccessPath,
    ) -> AccessCharacteristics {
        // Create a default frequency estimator for access frequency calculation
        let frequency_estimator = crate::cache::tier::warm::access_tracking::frequency_estimator::FrequencyEstimator::<K>::new();

        // Extract characteristics based on access patterns and value properties
        // Note: Using simplified approach for characteristics that require external dependencies
        AccessCharacteristics {
            access_frequency: access_path.frequency_estimate(&frequency_estimator),
            recent_accesses: 1.0, // Default - could be enhanced with actual statistics
            temporal_locality: 0.5, // Default temporal locality score
            spatial_locality: 0.5, // Default spatial locality score
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
}

/// Convert cache tier to array index for statistics
pub fn tier_to_index(tier: CacheTier) -> usize {
    match tier {
        CacheTier::Hot => 0,
        CacheTier::Warm => 1,
        CacheTier::Cold => 2,
    }
}
