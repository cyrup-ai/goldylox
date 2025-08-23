//! High-precision timing and performance measurement
//!
//! This module provides comprehensive performance measurement tools
//! including RDTSC-based timing and statistical analysis.

pub mod analysis;
pub mod session;
pub mod timer;

// Re-export main types
pub use analysis::{OperationTiers, PerformanceAnalysis, PerformanceReport, PerformanceSummary};
pub use session::{BatchTiming, OperationHandle, OperationTiming, PerformanceSession};
pub use timer::{timestamp_nanos, PrecisionTimer, Stopwatch, TimerCollection};
