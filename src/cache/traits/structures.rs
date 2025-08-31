//! Data structures for cache implementation types
//!
//! This module provides re-exports of canonical implementations from types_and_enums
//! for backward compatibility.

use std::fmt::Debug;


// Re-export canonical implementations for backward compatibility
pub use super::types_and_enums::{CapacityInfo, MaintenanceReport, PerformanceMetrics};

/// Latency performance categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LatencyCategory {
    UltraLow,
    Low,
    Medium,
    High,
}

/// Throughput performance categories  
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThroughputCategory {
    Low,
    Medium,
    High,
}
