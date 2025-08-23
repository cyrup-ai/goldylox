//! Tier promotion management system
//!
//! This module provides intelligent tier promotion and demotion with SIMD optimization,
//! lock-free queues, and adaptive criteria for cache performance optimization.

pub mod demotion_logic;
pub mod manager;
pub mod promotion_logic;
pub mod statistics;
pub mod types;

// Re-export main types and structs
// Re-export manager implementation

pub use types::{
    DemotionCriteria, PromotionCriteria, PromotionPriority, PromotionQueue, PromotionStatistics,
    PromotionTask, PromotionTaskType, TierLocation, TierPromotionManager,
};
