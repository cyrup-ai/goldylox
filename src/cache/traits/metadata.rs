//! Value metadata and serialization context traits with concrete implementations
//!
//! This module provides blazing-fast metadata and serialization interfaces optimized
//! for high-performance cache value management and persistent storage.

#![allow(dead_code)] // Cache traits - Metadata trait definitions and implementations

// Internal metadata traits - may not be used in minimal API

use std::fmt::Debug;

use super::supporting_types::{CompressionAlgorithm, SerializationFormat, ValueMetadata};
use super::types_and_enums::VolatilityLevel;
use crate::cache::traits::CacheValue;

/// Serialization context for persistent storage
pub trait SerializationContext: Send + Sync + Debug {
    /// Serialization format preference
    fn format(&self) -> SerializationFormat;

    /// Compression algorithm preference  
    fn compression(&self) -> CompressionAlgorithm;

    /// Schema version for evolution
    fn schema_version(&self) -> u32;
}

/// Metadata for cache values
#[derive(Debug, Clone, PartialEq)]
pub struct CacheValueMetadata {
    creation_cost: u64,
    access_frequency: f64,
    volatility: VolatilityLevel,
    compression_ratio: f32,
    component_count: usize,
    run_count: usize,
}

impl CacheValueMetadata {
    /// Create metadata from cache value
    #[inline]
    pub fn from_cache_value<V: CacheValue>(value: &V) -> Self {
        let estimated_size = value.estimated_size();

        // Estimate creation cost based on size
        let creation_cost = (estimated_size as u64) * 10;

        // Compression ratio estimate based on value size
        let compression_ratio = if estimated_size > 1024 { 0.7 } else { 0.9 };

        Self {
            creation_cost,
            access_frequency: 1.0,               // Default frequency
            volatility: VolatilityLevel::Stable, // Values are generally stable
            compression_ratio,
            // Estimate component count based on value complexity and size
            // Sophisticated heuristics for generic cache values
            component_count: if estimated_size > 8192 {
                estimated_size / 64 // Large complex values with many components
            } else if estimated_size > 1024 {
                estimated_size / 128 // Medium complexity values
            } else if estimated_size > 256 {
                estimated_size / 256 // Simple structured values
            } else {
                1 // Primitive or small values have single component
            },
            // Estimate run count based on structural complexity patterns
            // Reflects segmentation and memory layout complexity
            run_count: if estimated_size > 4096 {
                ((estimated_size as f64).log2() as usize)
                    .saturating_sub(8)
                    .max(1)
            } else if estimated_size > 512 {
                3 // Medium complexity typically has header/body/footer
            } else if estimated_size > 64 {
                2 // Simple structures have head/tail pattern
            } else {
                1 // Single run for primitive values
            },
        }
    }

    /// Create metadata with custom parameters
    #[inline(always)]
    pub const fn new(
        creation_cost: u64,
        access_frequency: f64,
        volatility: VolatilityLevel,
        compression_ratio: f32,
        component_count: usize,
        run_count: usize,
    ) -> Self {
        Self {
            creation_cost,
            access_frequency,
            volatility,
            compression_ratio,
            component_count,
            run_count,
        }
    }

    /// Get component count
    #[inline(always)]
    pub fn component_count(&self) -> usize {
        self.component_count
    }

    /// Get run count
    #[inline(always)]
    pub fn run_count(&self) -> usize {
        self.run_count
    }

    /// Update access frequency
    #[inline(always)]
    pub fn update_access_frequency(&mut self, new_frequency: f64) {
        self.access_frequency = new_frequency;
    }

    /// Check if value is complex (many components or runs)
    #[inline(always)]
    pub fn is_complex(&self) -> bool {
        self.component_count > 100 || self.run_count > 5
    }

    /// Check if value is simple (few components and runs)
    #[inline(always)]
    pub fn is_simple(&self) -> bool {
        self.component_count <= 20 && self.run_count <= 2
    }

    /// Estimate memory footprint in bytes
    #[inline(always)]
    pub fn estimated_memory_footprint(&self) -> usize {
        // Generic estimation based on creation cost
        (self.creation_cost / 10) as usize
    }

    /// Get value complexity score (0.0-1.0, higher = more complex)
    #[inline(always)]
    pub fn complexity_score(&self) -> f32 {
        let component_score = (self.component_count as f32 / 200.0).min(1.0);
        let run_score = (self.run_count as f32 / 10.0).min(1.0);
        (component_score + run_score) / 2.0
    }

    /// Check if value should be prioritized in cache
    #[inline(always)]
    pub fn should_prioritize(&self) -> bool {
        self.creation_cost > 50000 || self.is_complex()
    }
}

impl ValueMetadata for CacheValueMetadata {
    #[inline(always)]
    fn creation_cost(&self) -> u64 {
        self.creation_cost
    }

    #[inline(always)]
    fn access_frequency(&self) -> f64 {
        self.access_frequency
    }

    #[inline(always)]
    fn volatility(&self) -> VolatilityLevel {
        self.volatility
    }

    #[inline(always)]
    fn compression_ratio(&self) -> f32 {
        self.compression_ratio
    }
}

/// Standard serialization context
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardSerializationContext {
    format: SerializationFormat,
    compression: CompressionAlgorithm,
    schema_version: u32,
}

impl StandardSerializationContext {
    /// Create new serialization context
    #[inline(always)]
    pub const fn new(format: SerializationFormat, compression: CompressionAlgorithm) -> Self {
        Self {
            format,
            compression,
            schema_version: 1,
        }
    }

    /// Create new context with custom schema version
    #[inline(always)]
    pub const fn with_version(
        format: SerializationFormat,
        compression: CompressionAlgorithm,
        schema_version: u32,
    ) -> Self {
        Self {
            format,
            compression,
            schema_version,
        }
    }

    /// Create binary context with LZ4 compression
    #[inline(always)]
    pub const fn binary_lz4() -> Self {
        Self::new(SerializationFormat::Binary, CompressionAlgorithm::Lz4)
    }

    /// Create protobuf context with Zstd compression
    #[inline(always)]
    pub const fn protobuf_zstd() -> Self {
        Self::new(SerializationFormat::Protobuf, CompressionAlgorithm::Zstd)
    }

    /// Create message pack context with LZ4 compression
    #[inline(always)]
    pub const fn msgpack_lz4() -> Self {
        Self::new(SerializationFormat::MessagePack, CompressionAlgorithm::Lz4)
    }

    /// Create custom format with no compression
    #[inline(always)]
    pub const fn custom_uncompressed() -> Self {
        Self::new(SerializationFormat::Custom, CompressionAlgorithm::None)
    }

    /// Check if compression is active
    #[inline(always)]
    pub fn has_compression(&self) -> bool {
        !matches!(self.compression, CompressionAlgorithm::None)
    }

    /// Check if format is binary-based
    #[inline(always)]
    pub fn is_binary_format(&self) -> bool {
        matches!(
            self.format,
            SerializationFormat::Binary | SerializationFormat::Protobuf
        )
    }

    /// Get estimated serialization overhead
    #[inline(always)]
    pub fn serialization_overhead(&self) -> f32 {
        match self.format {
            SerializationFormat::Binary => 1.1,       // 10% overhead
            SerializationFormat::Bincode => 1.05,     // 5% overhead
            SerializationFormat::Json => 1.3,         // 30% overhead
            SerializationFormat::MessagePack => 1.15, // 15% overhead
            SerializationFormat::Protobuf => 1.2,     // 20% overhead
            SerializationFormat::Custom => 1.0,       // No overhead
        }
    }
}

impl SerializationContext for StandardSerializationContext {
    #[inline(always)]
    fn format(&self) -> SerializationFormat {
        self.format
    }

    #[inline(always)]
    fn compression(&self) -> CompressionAlgorithm {
        self.compression
    }

    #[inline(always)]
    fn schema_version(&self) -> u32 {
        self.schema_version
    }
}
