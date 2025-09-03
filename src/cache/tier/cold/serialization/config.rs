//! Configuration and types for cold tier serialization
//!
//! This module contains configuration structures, constants, and the
//! serialization engine for cold tier cache operations.

use crate::cache::traits::CacheValue;

/// Serialization engine configuration
#[derive(Debug, Clone)]
pub struct SerializationConfig {
    /// Enable compression
    pub enable_compression: bool,
    /// Compression level
    pub compression_level: u32,
}

impl Default for SerializationConfig {
    fn default() -> Self {
        Self {
            enable_compression: false,
            compression_level: 1,
        }
    }
}

/// Serialization engine for cache value data
#[derive(Debug)]
pub struct SerializationEngine {
    
    config: SerializationConfig,
}

impl SerializationEngine {
    pub fn new(config: SerializationConfig) -> Self {
        Self { config }
    }
}

/// Serialization result
#[derive(Debug)]
pub struct SerializationResult {
    pub data: Vec<u8>,
    pub original_size: usize,
    pub compressed_size: usize,
}

/// Deserialization result
#[derive(Debug)]
pub struct DeserializationResult<V: CacheValue> {
    pub data: V,
    pub size: usize,
}

/// Serialization format version for compatibility
pub const SERIALIZATION_VERSION: u32 = 1;

/// Magic bytes for file format identification
pub const MAGIC_BYTES: &[u8] = b"COLD";
