//! Configuration and types for cold tier serialization
//!
//! This module contains configuration structures, constants, and the
//! serialization engine for cold tier cache operations.

use crate::cache::traits::CacheValue;

/// Serialization engine configuration
#[derive(Debug, Clone)]
#[allow(dead_code)] // Cold tier serialization - Configuration structure for serialization engine settings
pub struct SerializationConfig {
    /// Compression active
    pub compression_active: bool,
    /// Compression level
    pub compression_level: u32,
}

impl Default for SerializationConfig {
    fn default() -> Self {
        Self {
            compression_active: true,
            compression_level: 1,
        }
    }
}

/// Serialization engine for cache value data
#[derive(Debug)]
#[allow(dead_code)] // Cold tier serialization - Main serialization engine for coordinating cache value processing
pub struct SerializationEngine {
    config: SerializationConfig,
}

impl SerializationEngine {
    #[allow(dead_code)] // Cold tier serialization - Constructor for serialization engine with configuration
    pub fn new(config: SerializationConfig) -> Self {
        Self { config }
    }
}

/// Serialization result
#[derive(Debug)]
#[allow(dead_code)] // Cold tier serialization - Result structure containing serialized data and size metrics
pub struct SerializationResult {
    pub data: Vec<u8>,
    pub original_size: usize,
    pub compressed_size: usize,
}

/// Deserialization result
#[derive(Debug)]
#[allow(dead_code)] // Cold tier serialization - Result structure containing deserialized data and size information
pub struct DeserializationResult<V: CacheValue> {
    pub data: V,
    pub size: usize,
}

/// Serialization format version for compatibility
#[allow(dead_code)] // Cold tier serialization - Version constant for format compatibility checks, used by header.rs
pub const SERIALIZATION_VERSION: u32 = 1;

/// Magic bytes for file format identification
#[allow(dead_code)] // Cold tier serialization - File format identifier for storage format validation, used by header.rs
pub const MAGIC_BYTES: &[u8] = b"COLD";
