//! Cache configuration system with production-ready defaults
//!
//! This module provides comprehensive configuration for the hierarchical cache system
//! with lock-free data structures and atomic metadata tracking.

// Internal config architecture - components may not be used in minimal API

pub mod global;
pub mod types;
// pub mod validator; // Removed - not used by simple public API

// Export only types actually used by external code
pub use types::CacheConfig;
// ConfigError and global config functions removed - not used by simple public API
