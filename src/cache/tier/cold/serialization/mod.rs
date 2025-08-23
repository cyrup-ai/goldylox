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
pub use binary_format::{deserialize_cache_value, serialize_cache_value};
pub use config::{
    DeserializationResult, SerializationConfig, SerializationEngine, SerializationResult,
    MAGIC_BYTES, SERIALIZATION_VERSION,
};
pub use header::StorageHeader;
pub use utilities::calculate_checksum;
