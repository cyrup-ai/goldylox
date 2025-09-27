//! Cold tier serialization module
//!
//! This module provides serialization and deserialization functionality
//! for the cold tier cache, including binary format handling and storage operations.

pub mod binary_format;
pub mod config;
pub mod header;
pub mod storage_ops;
pub mod utilities;

// Re-export main types and functions
