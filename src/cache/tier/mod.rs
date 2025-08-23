//! Intelligent tier management with SIMD-optimized promotion/demotion system
//!
//! This module implements sophisticated tier promotion and demotion algorithms
//! using lock-free data structures and SIMD-accelerated scoring for optimal performance.

// Declare submodules
pub mod cold;
pub mod criteria;
pub mod hot;
pub mod manager;
pub mod queue;
pub mod statistics;
pub mod warm;

// Re-export key types for public API
pub use criteria::{
    AccessCharacteristics, DemotionCriteria, PromotionCriteria, SIMDScoringCoefficients,
};
pub use manager::{tier_to_index, PromotionDecision, TierPromotionManager};
pub use queue::{PromotionPriority, PromotionQueue, PromotionTask, QueueStatistics};
pub use statistics::{PromotionStatistics, PromotionStatsSnapshot};
