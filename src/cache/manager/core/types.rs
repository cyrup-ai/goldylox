//! Core types and data structures for unified cache manager
//!
//! This module defines the fundamental types used throughout the unified
//! cache management system.

// Removed unused import

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

// Removed re-export to eliminate type identity conflicts - use canonical imports instead

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

