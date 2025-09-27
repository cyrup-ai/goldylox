//! Advanced cache eviction policies with machine learning-based decision making
//!
//! This module implements sophisticated cache replacement algorithms including
//! adaptive LRU/LFU, machine learning-based predictions, and intelligent prefetching.

pub mod ml_policies;
pub mod policy_engine;
pub mod prefetch;
pub mod traditional_policies;
pub mod types;
pub mod write_policies;

// Re-export core types
// Re-export main policy engine
pub use policy_engine::{CachePolicyEngine, PolicyStats};
// Remove unused traditional policy exports
pub(crate) use types::PolicyType;
