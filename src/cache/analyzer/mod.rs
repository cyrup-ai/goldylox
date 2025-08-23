//! Access pattern analyzer for cache optimization
//!
//! This module provides comprehensive access pattern analysis for cache keys,
//! enabling intelligent cache management decisions based on usage patterns.

pub mod access_record;
pub mod analyzer_core;
pub mod pattern_detection;
pub mod types;
pub mod utilities;

// REMOVED: Backwards compatibility re-exports that hide canonical API paths
// Users must now import from canonical module paths:
// - Use analyzer::access_record::AccessRecord
// - Use analyzer::analyzer_core::AccessPatternAnalyzer
// - Use analyzer::pattern_detection::PatternDetector
// - Use analyzer::types::{AccessPattern, AccessPatternType, AnalyzerError, AnalyzerStatistics}
