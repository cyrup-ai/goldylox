//! Data structures for cache implementation types
//!
//! This module provides re-exports of canonical implementations from types_and_enums
//! for backward compatibility.

#![allow(dead_code)] // Cache traits - Data structure definitions for cache implementation types

// Internal structure traits - may not be used in minimal API

use std::fmt::Debug;

// Canonical implementations available at: super::types_and_enums::{CapacityInfo, MaintenanceReport, PerformanceMetrics}

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
