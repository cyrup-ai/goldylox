//! Error recovery system for fault tolerance
//!
//! This module provides comprehensive error detection, recovery strategies,
//! and circuit breaker patterns for cache fault tolerance.

pub mod circuit_breaker;
pub mod coordinator;
pub mod core;
pub mod detection;
pub mod provider;
pub mod statistics;
pub mod strategies;
pub mod types;

// Re-export only actually used types
pub use core::ErrorRecoverySystem;
pub use provider::{ErrorRecoveryProvider, FallbackErrorProvider};
