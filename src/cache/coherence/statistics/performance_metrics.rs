//! Performance metrics aggregation for coherence operations
//!
//! This module provides high-level performance metrics that aggregate
//! operation-specific metrics for comprehensive performance analysis.

use super::operation_metrics::OperationMetrics;

/// Performance metrics for coherence operations
#[derive(Debug)]
pub struct CoherencePerformanceMetrics {
    /// Read operation statistics
    pub read_operations: OperationMetrics,
    /// Write operation statistics
    pub write_operations: OperationMetrics,
    /// Invalidation operation statistics
    pub invalidation_operations: OperationMetrics,
    /// State transition statistics
    pub transition_operations: OperationMetrics,
}
