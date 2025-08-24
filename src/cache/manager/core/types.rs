//! Core types and data structures for unified cache manager
//!
//! This module defines the fundamental types used throughout the unified
//! cache management system.

use std::time::Instant;

use crate::cache::coherence::CacheTier;
use crate::cache::config::CacheConfig;
use crate::cache::traits::core::{CacheKey, CacheValue};
use crate::cache::coordinator::background_coordinator::BackgroundCoordinator;
use crate::cache::manager::error_recovery::ErrorRecoverySystem;
use crate::cache::manager::performance::PerformanceMonitor;
use crate::cache::eviction::policy_engine::CachePolicyEngine;
use crate::cache::manager::UnifiedCacheStatistics;
use crate::cache::coordinator::strategy_selector::CacheStrategySelector;
use crate::cache::tier::manager::TierPromotionManager;

/// Unified cache manager coordinating all tiers
#[derive(Debug)]
pub struct UnifiedCacheManager<K: CacheKey, V: CacheValue> {
    /// Cache configuration
    pub config: CacheConfig,
    /// Cache strategy selector
    pub strategy_selector: CacheStrategySelector,
    /// Tier promotion/demotion manager
    pub tier_manager: TierPromotionManager<K>,
    /// Unified cache statistics
    pub unified_stats: UnifiedCacheStatistics,
    /// Background operation coordinator
    pub background_coordinator: BackgroundCoordinator<K, V>,
    /// Cache policy engine
    pub policy_engine: CachePolicyEngine<K, V>,
    /// Performance monitor
    pub performance_monitor: PerformanceMonitor,
    /// Error recovery system
    pub error_recovery: ErrorRecoverySystem,
    /// Phantom data for generic types
    pub _phantom: std::marker::PhantomData<(K, V)>,
}

/// Tracks which tiers have been accessed during a cache operation
#[derive(Debug)]
pub struct AccessPath {
    pub tried_hot: bool,
    pub tried_warm: bool,
    pub tried_cold: bool,
    pub start_time: Instant,
}

impl AccessPath {
    /// Estimate access frequency based on tier access pattern
    pub fn frequency_estimate(&self) -> f32 {
        let mut frequency = 1.0;
        if self.tried_hot {
            frequency += 2.0;
        }
        if self.tried_warm {
            frequency += 1.0;
        }
        if self.tried_cold {
            frequency += 0.5;
        }
        frequency
    }

    /// Get recent access count (placeholder implementation)
    pub fn recent_access_count(&self) -> u32 {
        let mut count = 0;
        if self.tried_hot {
            count += 3;
        }
        if self.tried_warm {
            count += 2;
        }
        if self.tried_cold {
            count += 1;
        }
        count
    }

    /// Calculate temporal locality score
    pub fn temporal_locality_score(&self) -> f32 {
        let elapsed = self.start_time.elapsed().as_nanos() as f32;
        // Higher score for more recent access
        1.0 / (1.0 + elapsed / 1_000_000.0) // Normalize to milliseconds
    }

    /// Calculate spatial locality score
    pub fn spatial_locality_score(&self) -> f32 {
        // Simple heuristic based on tier access pattern
        if self.tried_hot && self.tried_warm {
            0.8
        } else if self.tried_hot {
            0.6
        } else if self.tried_warm {
            0.4
        } else {
            0.2
        }
    }

    /// Calculate average access delay
    pub fn average_access_delay(&self) -> f32 {
        let elapsed_ns = self.start_time.elapsed().as_nanos() as f32;
        let tier_count = [self.tried_hot, self.tried_warm, self.tried_cold]
            .iter()
            .filter(|&&x| x)
            .count() as f32;

        if tier_count > 0.0 {
            elapsed_ns / tier_count
        } else {
            0.0
        }
    }
}

/// Decision about where to place a value in the cache hierarchy
#[derive(Debug)]
pub struct PlacementDecision {
    pub primary_tier: CacheTier,
    pub replication_tiers: Vec<CacheTier>,
    pub confidence: f32,
}

/// Characteristics of a cached value for placement decisions
#[derive(Debug)]
pub struct ValueCharacteristics {
    pub size: usize,
    pub complexity: usize,
    pub creation_cost: u64,
}

/// Decision about whether to promote a value between tiers
#[derive(Debug)]
pub struct PromotionDecision {
    pub should_promote: bool,
    pub priority: u8,
    pub confidence: f32,
}

/// Unified statistics snapshot for monitoring
#[derive(Debug, Clone)]
pub struct UnifiedStats {
    pub total_operations: u64,
    pub overall_hit_rate: f64,
    pub hot_tier_hits: u64,
    pub warm_tier_hits: u64,
    pub cold_tier_hits: u64,
    pub total_misses: u64,
    pub avg_access_latency_ns: u64,
    pub promotions_performed: u64,
    pub demotions_performed: u64,
    pub total_memory_usage: u64,
}

/// Background tasks for asynchronous cache operations
#[derive(Debug)]
pub enum BackgroundTask<K: CacheKey, V: CacheValue> {
    Promote {
        key: K,
        value: V,
        from_tier: CacheTier,
        to_tier: CacheTier,
        priority: u8,
    },
    Demote {
        key: K,
        from_tier: CacheTier,
        to_tier: CacheTier,
    },
    Maintenance {
        tier: CacheTier,
        operation: MaintenanceOperation,
    },
    Statistics {
        operation: StatisticsOperation,
    },
}

/// Types of maintenance operations for cache tiers
#[derive(Debug)]
pub enum MaintenanceOperation {
    Compact,
    Cleanup,
    Optimize,
}

/// Types of statistics operations
#[derive(Debug)]
pub enum StatisticsOperation {
    Collect,
    Analyze,
    Report,
}

impl AccessPath {
    /// Create new access path tracker
    pub fn new() -> Self {
        Self {
            tried_hot: false,
            tried_warm: false,
            tried_cold: false,
            start_time: Instant::now(),
        }
    }
}
