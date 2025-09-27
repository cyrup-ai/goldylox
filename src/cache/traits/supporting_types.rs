//! Supporting trait definitions for sophisticated cache system functionality
//!
//! This module provides specialized traits that extend and support the core cache functionality
//! with zero-cost abstractions for hashing, prioritization, serialization, and metadata.

#![allow(dead_code)] // Cache traits - Supporting trait definitions for sophisticated functionality

// Internal supporting traits - may not be used in minimal API

use std::fmt::Debug;
use std::hash::Hash;

use super::types_and_enums::{PriorityClass, VolatilityLevel};

/// Sophisticated hash context for specialized key types
pub trait HashContext: Send + Sync + Debug + Clone + Default {
    /// Hash algorithm identifier
    fn algorithm(&self) -> HashAlgorithm;

    /// Seed value for hash functions
    fn seed(&self) -> u64;

    /// Context-specific hash computation
    fn compute_hash<T: Hash>(&self, value: &T) -> u64;
}

/// Priority levels for cache eviction with fine-grained control
pub trait Priority: Send + Sync + Debug + Copy + PartialOrd + Ord + Default {
    /// Numerical priority value (higher = more important)
    fn value(&self) -> u32;

    /// Priority class for grouping
    fn class(&self) -> PriorityClass;

    /// Dynamic priority adjustment based on access patterns
    fn adjust(&self, factor: f32) -> Self;
}

/// Size estimation trait for memory accounting
pub trait SizeEstimator: Send + Sync + Debug + Default {
    /// Estimate memory usage in bytes
    fn estimate_size<T>(&self, value: &T) -> usize;

    /// Deep size estimation including referenced data
    fn deep_estimate_size<T>(&self, value: &T) -> usize;

    /// Overhead estimation for cache structures
    fn overhead_size(&self) -> usize;
}

/// Value metadata for intelligent caching decisions
pub trait ValueMetadata: Send + Sync + Debug + Clone {
    /// Creation cost estimation (CPU cycles)
    fn creation_cost(&self) -> u64;

    /// Access frequency estimation
    fn access_frequency(&self) -> f64;

    /// Value volatility (how often it changes)
    fn volatility(&self) -> VolatilityLevel;

    /// Compression ratio estimate
    fn compression_ratio(&self) -> f32;
}

/// Serialization context for persistent storage
pub trait SerializationContext: Send + Sync + Debug + Default {
    /// Serialization format preference
    fn format(&self) -> SerializationFormat;

    /// Compression algorithm preference  
    fn compression(&self) -> CompressionAlgorithm;

    /// Schema version for evolution
    fn schema_version(&self) -> u32;
}

/// Hash algorithm enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    AHash,
    FxHash,
    SipHash,
    Blake3,
}

/// Serialization format enumeration
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    bincode::Encode,
    bincode::Decode,
)]
pub enum SerializationFormat {
    Binary,
    Json,
    MessagePack,
    Bincode,
    Protobuf,
    Custom,
}

/// Compression algorithm enumeration
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    bincode::Encode,
    bincode::Decode,
)]
pub enum CompressionAlgorithm {
    None,
    Lz4,
    Zstd,
    Deflate,
    Brotli,
}

// Simple implementations for basic types

/// Simple hash context implementation
#[derive(Debug, Clone, Default)]
pub struct SimpleHashContext;

impl HashContext for SimpleHashContext {
    fn algorithm(&self) -> HashAlgorithm {
        HashAlgorithm::AHash
    }

    fn seed(&self) -> u64 {
        0
    }

    fn compute_hash<T: Hash>(&self, value: &T) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }
}

impl HashContext for () {
    fn algorithm(&self) -> HashAlgorithm {
        HashAlgorithm::AHash
    }

    fn seed(&self) -> u64 {
        0
    }

    fn compute_hash<T: Hash>(&self, value: &T) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }
}

/// Simple priority implementation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct SimplePriority(pub u8);

impl Priority for SimplePriority {
    fn value(&self) -> u32 {
        self.0 as u32
    }

    fn class(&self) -> PriorityClass {
        match self.0 {
            0..=3 => PriorityClass::Low,
            4..=6 => PriorityClass::Normal,
            7..=10 => PriorityClass::High,
            _ => PriorityClass::High,
        }
    }

    fn adjust(&self, factor: f32) -> Self {
        let new_val = ((self.0 as f32) * factor).clamp(0.0, 255.0) as u8;
        SimplePriority(new_val)
    }
}

impl Priority for u8 {
    fn value(&self) -> u32 {
        *self as u32
    }

    fn class(&self) -> PriorityClass {
        match *self {
            0..=3 => PriorityClass::Low,
            4..=6 => PriorityClass::Normal,
            7..=10 => PriorityClass::High,
            _ => PriorityClass::High,
        }
    }

    fn adjust(&self, factor: f32) -> Self {
        ((*self as f32) * factor).clamp(0.0, 255.0) as u8
    }
}

/// Simple size estimator implementation
#[derive(Debug, Default)]
pub struct SimpleSizeEstimator;

impl SizeEstimator for SimpleSizeEstimator {
    fn estimate_size<T>(&self, _value: &T) -> usize {
        std::mem::size_of::<T>()
    }

    fn deep_estimate_size<T>(&self, _value: &T) -> usize {
        std::mem::size_of::<T>()
    }

    fn overhead_size(&self) -> usize {
        64 // Reasonable cache overhead estimate
    }
}

impl SizeEstimator for usize {
    fn estimate_size<T>(&self, _value: &T) -> usize {
        *self
    }

    fn deep_estimate_size<T>(&self, _value: &T) -> usize {
        *self
    }

    fn overhead_size(&self) -> usize {
        64
    }
}

impl ValueMetadata for () {
    fn creation_cost(&self) -> u64 {
        0
    }

    fn access_frequency(&self) -> f64 {
        0.0
    }

    fn volatility(&self) -> VolatilityLevel {
        VolatilityLevel::Stable
    }

    fn compression_ratio(&self) -> f32 {
        1.0
    }
}

// Conversion between CompressionAlgorithm types
impl From<crate::cache::tier::cold::data_structures::CompressionAlgorithm>
    for CompressionAlgorithm
{
    fn from(cold_alg: crate::cache::tier::cold::data_structures::CompressionAlgorithm) -> Self {
        use crate::cache::tier::cold::data_structures::CompressionAlgorithm as ColdAlg;
        match cold_alg {
            ColdAlg::None => CompressionAlgorithm::None,
            ColdAlg::Lz4 => CompressionAlgorithm::Lz4,
            ColdAlg::Gzip => CompressionAlgorithm::Deflate, // Map Gzip to Deflate
            ColdAlg::Zstd => CompressionAlgorithm::Zstd,
            ColdAlg::Snappy => CompressionAlgorithm::Brotli, // Map Snappy to Brotli
            ColdAlg::Brotli => CompressionAlgorithm::Brotli,
        }
    }
}
