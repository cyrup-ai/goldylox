//! CANONICAL Maintenance task types for cache background operations
//!
//! This module defines the CANONICAL MaintenanceTask enum that serves as the
//! unified source of truth for all maintenance tasks across the entire cache system.
//! Other modules should re-export this canonical implementation.
//!
//! ARCHITECTURAL DECISION: This is the most sophisticated implementation with
//! comprehensive task types, rich parameter sets, and production-quality methods.

use std::time::Duration;

/// Maintenance task for background operations
#[allow(dead_code)] // Warm tier maintenance - comprehensive maintenance task enum with all operation types
#[derive(Debug, Clone)]
pub enum MaintenanceTask {
    /// Cleanup expired entries
    CleanupExpired { ttl: Duration, batch_size: usize },
    /// Perform eviction to reduce memory pressure
    PerformEviction {
        target_pressure: f64,
        max_evictions: usize,
    },
    /// Simple eviction task
    Evict {
        target_count: usize,
        policy_hint: String,
    },
    /// Update cache statistics and metrics
    UpdateStatistics { include_detailed_analysis: bool },
    /// Optimize cache data structure layout
    OptimizeStructure {
        optimization_level: OptimizationLevel,
    },
    /// Compact storage to reduce fragmentation
    CompactStorage { compaction_threshold: f64 },
    /// Analyze access patterns for prediction
    AnalyzePatterns {
        analysis_depth: AnalysisDepth,
        prediction_horizon_sec: u64,
    },
    /// Synchronize with other cache tiers
    SyncTiers {
        sync_direction: SyncDirection,
        consistency_level: ConsistencyLevel,
    },
    /// Validate cache integrity
    ValidateIntegrity { check_level: ValidationLevel },
    /// Perform memory defragmentation
    DefragmentMemory { target_fragmentation: f64 },
    /// Update machine learning models
    UpdateMLModels {
        training_data_size: usize,
        model_complexity: ModelComplexity,
    },
}

/// Optimization level for structure optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationLevel {
    /// Basic optimization (fast)
    Basic,
    /// Standard optimization (balanced)
    Standard,
    /// Aggressive optimization (thorough but slower)
    Aggressive,
    /// Full optimization (maximum thoroughness)
    Full,
}

/// Analysis depth for pattern analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisDepth {
    /// Surface-level analysis
    Surface,
    /// Standard depth analysis
    Standard,
    /// Deep analysis with correlation detection
    Deep,
    /// Comprehensive analysis with prediction
    Comprehensive,
}

/// Synchronization direction between tiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDirection {
    /// Sync from warm to hot tier
    WarmToHot,
    /// Sync from warm to cold tier
    WarmToCold,
    /// Bidirectional sync
    Bidirectional,
    /// Full tier rebalancing
    FullRebalance,
}

/// Consistency level for tier synchronization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsistencyLevel {
    /// Eventually consistent (best performance)
    Eventual,
    /// Session consistent (balanced)
    Session,
    /// Strong consistency (highest correctness)
    Strong,
    /// Linearizable consistency (strictest)
    Linearizable,
}

/// Validation level for integrity checks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationLevel {
    /// Basic key-value consistency
    Basic,
    /// Standard metadata validation
    Standard,
    /// Deep structural validation
    Deep,
    /// Comprehensive validation with statistics
    Comprehensive,
}

/// Machine learning model complexity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelComplexity {
    /// Linear models (fast training/inference)
    Linear,
    /// Polynomial models (moderate complexity)
    Polynomial,
    /// Neural network models (high complexity)
    Neural,
    /// Ensemble models (highest accuracy)
    Ensemble,
}

impl MaintenanceTask {
    /// Get the estimated duration for this maintenance task
    #[inline]
    pub fn estimated_duration(&self) -> Duration {
        match self {
            MaintenanceTask::CleanupExpired { batch_size, .. } => {
                Duration::from_millis(*batch_size as u64 / 10)
            }
            MaintenanceTask::PerformEviction { max_evictions, .. } => {
                Duration::from_millis(*max_evictions as u64 / 5)
            }
            MaintenanceTask::Evict { target_count, .. } => {
                Duration::from_millis(*target_count as u64 / 10)
            }
            MaintenanceTask::UpdateStatistics {
                include_detailed_analysis,
            } => {
                if *include_detailed_analysis {
                    Duration::from_millis(100)
                } else {
                    Duration::from_millis(10)
                }
            }
            MaintenanceTask::OptimizeStructure {
                optimization_level, ..
            } => match optimization_level {
                OptimizationLevel::Basic => Duration::from_millis(50),
                OptimizationLevel::Standard => Duration::from_millis(200),
                OptimizationLevel::Aggressive => Duration::from_millis(500),
                OptimizationLevel::Full => Duration::from_secs(2),
            },
            MaintenanceTask::CompactStorage { .. } => Duration::from_millis(300),
            MaintenanceTask::AnalyzePatterns { analysis_depth, .. } => match analysis_depth {
                AnalysisDepth::Surface => Duration::from_millis(25),
                AnalysisDepth::Standard => Duration::from_millis(100),
                AnalysisDepth::Deep => Duration::from_millis(400),
                AnalysisDepth::Comprehensive => Duration::from_secs(1),
            },
            MaintenanceTask::SyncTiers {
                consistency_level, ..
            } => match consistency_level {
                ConsistencyLevel::Eventual => Duration::from_millis(50),
                ConsistencyLevel::Session => Duration::from_millis(150),
                ConsistencyLevel::Strong => Duration::from_millis(300),
                ConsistencyLevel::Linearizable => Duration::from_millis(600),
            },
            MaintenanceTask::ValidateIntegrity { check_level } => match check_level {
                ValidationLevel::Basic => Duration::from_millis(20),
                ValidationLevel::Standard => Duration::from_millis(80),
                ValidationLevel::Deep => Duration::from_millis(250),
                ValidationLevel::Comprehensive => Duration::from_millis(800),
            },
            MaintenanceTask::DefragmentMemory { .. } => Duration::from_millis(400),
            MaintenanceTask::UpdateMLModels {
                model_complexity, ..
            } => match model_complexity {
                ModelComplexity::Linear => Duration::from_millis(100),
                ModelComplexity::Polynomial => Duration::from_millis(300),
                ModelComplexity::Neural => Duration::from_secs(2),
                ModelComplexity::Ensemble => Duration::from_secs(5),
            },
        }
    }

    /// Get the priority level for this maintenance task
    #[inline]
    pub fn priority(&self) -> TaskPriority {
        match self {
            MaintenanceTask::CleanupExpired { .. } => TaskPriority::Medium,
            MaintenanceTask::PerformEviction { .. } => TaskPriority::High,
            MaintenanceTask::Evict { .. } => TaskPriority::High,
            MaintenanceTask::UpdateStatistics { .. } => TaskPriority::Low,
            MaintenanceTask::OptimizeStructure { .. } => TaskPriority::Medium,
            MaintenanceTask::CompactStorage { .. } => TaskPriority::Medium,
            MaintenanceTask::AnalyzePatterns { .. } => TaskPriority::Low,
            MaintenanceTask::SyncTiers { .. } => TaskPriority::High,
            MaintenanceTask::ValidateIntegrity { .. } => TaskPriority::Low,
            MaintenanceTask::DefragmentMemory { .. } => TaskPriority::Medium,
            MaintenanceTask::UpdateMLModels { .. } => TaskPriority::Low,
        }
    }

    /// Check if this task can be interrupted safely
    #[inline]
    pub fn is_interruptible(&self) -> bool {
        match self {
            MaintenanceTask::CleanupExpired { .. } => true,
            MaintenanceTask::PerformEviction { .. } => false,
            MaintenanceTask::Evict { .. } => false,
            MaintenanceTask::UpdateStatistics { .. } => true,
            MaintenanceTask::OptimizeStructure { .. } => true,
            MaintenanceTask::CompactStorage { .. } => false,
            MaintenanceTask::AnalyzePatterns { .. } => true,
            MaintenanceTask::SyncTiers { .. } => false,
            MaintenanceTask::ValidateIntegrity { .. } => true,
            MaintenanceTask::DefragmentMemory { .. } => false,
            MaintenanceTask::UpdateMLModels { .. } => true,
        }
    }
}

/// Task priority levels for scheduling
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Low = 1,
    Medium = 2,
    High = 3,
}

impl Default for OptimizationLevel {
    #[inline]
    fn default() -> Self {
        Self::Standard
    }
}

impl Default for AnalysisDepth {
    #[inline]
    fn default() -> Self {
        Self::Standard
    }
}

impl Default for SyncDirection {
    #[inline]
    fn default() -> Self {
        Self::Bidirectional
    }
}

impl Default for ConsistencyLevel {
    #[inline]
    fn default() -> Self {
        Self::Session
    }
}

impl Default for ValidationLevel {
    #[inline]
    fn default() -> Self {
        Self::Standard
    }
}

impl Default for ModelComplexity {
    #[inline]
    fn default() -> Self {
        Self::Polynomial
    }
}
