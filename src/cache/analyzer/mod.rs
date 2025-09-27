//! Access pattern analyzer for cache optimization
//!
//! This module provides comprehensive access pattern analysis for cache keys,
//! enabling intelligent cache management decisions based on usage patterns.

pub mod access_record;
pub mod analyzer_core;
pub mod types;

// HIGHLANDER CANONICALIZATION: PatternDetector eliminated from this module
// PatternDetector now uses canonical implementation at:
// crate::cache::tier::hot::prefetch::pattern_detection::PatternDetector
//
// Users must import from canonical module paths:
// - Use analyzer::access_record::AccessRecord
// - Use analyzer::analyzer_core::AccessPatternAnalyzer
// - Use tier::hot::prefetch::pattern_detection::PatternDetector (CANONICAL)
// - Use analyzer::types::{AccessPattern, AccessPatternType, AnalyzerError, AnalyzerStatistics}
