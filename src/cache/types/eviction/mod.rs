//! Eviction candidate management and selection
//!
//! This module provides comprehensive eviction candidate management
//! with sophisticated selection algorithms and metadata tracking.

pub mod candidate;
pub mod selector;

// Re-export main types
pub use candidate::{CandidateMetadata, EvictionCandidate};
pub use selector::{CandidateAnalysis, EvictionSelector, SelectorConfig};
