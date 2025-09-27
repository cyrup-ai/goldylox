//! Cache management modules
//!
//! This module contains management functionality for cache operations
//! including error recovery, performance monitoring, and system coordination.

pub mod background;
pub mod error_recovery;
pub mod performance;
pub mod policy;
pub mod strategy;

pub use error_recovery::{ComponentCircuitBreaker, ErrorRecoveryProvider, FallbackErrorProvider};
