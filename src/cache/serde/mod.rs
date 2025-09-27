#![allow(dead_code)]
// Serde integration - Complete serialization library with type-safe wrappers, efficient encoding/decoding, and cache-optimized serialization

//! Serialization support for cache types with serde integration
//!
//! This module provides wrapper types to make any serde-compatible type
//! work as cache keys and values using the simplified trait system.

use std::fmt::Debug;
use std::hash::Hash;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::traits::{CacheKey, CacheValue};
use crate::cache::traits::supporting_types::HashContext;

/// Wrapper type that makes any serde type work as a CacheKey
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct SerdeCacheKey<T>(pub T)
where
    T: Serialize
        + DeserializeOwned
        + Clone
        + Hash
        + Eq
        + Ord
        + Send
        + Sync
        + Debug
        + Default
        + 'static;

impl<T> Default for SerdeCacheKey<T>
where
    T: Serialize
        + DeserializeOwned
        + Clone
        + Hash
        + Eq
        + Ord
        + Send
        + Sync
        + Debug
        + Default
        + 'static,
{
    fn default() -> Self {
        SerdeCacheKey(T::default())
    }
}

impl<T> bincode::Encode for SerdeCacheKey<T>
where
    T: Serialize
        + DeserializeOwned
        + Clone
        + Hash
        + Eq
        + Ord
        + Send
        + Sync
        + Debug
        + Default
        + 'static,
{
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        // Use bincode's serde integration
        bincode::serde::Compat(&self.0).encode(encoder)
    }
}

impl<T> bincode::Decode<()> for SerdeCacheKey<T>
where
    T: Serialize
        + DeserializeOwned
        + Clone
        + Hash
        + Eq
        + Ord
        + Send
        + Sync
        + Debug
        + Default
        + 'static,
{
    fn decode<D: bincode::de::Decoder<Context = ()>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        // Use bincode's serde integration
        let compat = bincode::serde::Compat::<T>::decode(decoder)?;
        Ok(SerdeCacheKey(compat.0))
    }
}

impl<T> CacheKey for SerdeCacheKey<T>
where
    T: Serialize
        + DeserializeOwned
        + Clone
        + Hash
        + Eq
        + Ord
        + Send
        + Sync
        + Debug
        + Default
        + 'static,
{
    type HashContext = super::traits::impls::StandardHashContext;
    type Priority = super::traits::impls::StandardPriority;
    type SizeEstimator = super::traits::impls::StandardSizeEstimator;

    fn estimated_size(&self) -> usize {
        std::mem::size_of::<T>() + std::mem::size_of::<SerdeCacheKey<T>>()
    }

    fn tier_affinity(&self) -> super::traits::types_and_enums::TierAffinity {
        super::traits::types_and_enums::TierAffinity::Auto
    }

    fn hash_context(&self) -> Self::HashContext {
        super::traits::impls::StandardHashContext::ahash_default()
    }

    fn priority(&self) -> Self::Priority {
        super::traits::impls::StandardPriority::normal()
    }

    fn size_estimator(&self) -> Self::SizeEstimator {
        super::traits::impls::StandardSizeEstimator::new()
    }

    fn fast_hash(&self, context: &Self::HashContext) -> u64 {
        context.compute_hash(&self.0)
    }
}

/// Wrapper type that makes any serde type work as a CacheValue
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SerdeCacheValue<T>(pub T)
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + 'static;

impl<T> Default for SerdeCacheValue<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + 'static,
{
    fn default() -> Self {
        SerdeCacheValue(T::default())
    }
}

impl<T> bincode::Encode for SerdeCacheValue<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + 'static,
{
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        // Use bincode's serde integration
        bincode::serde::Compat(&self.0).encode(encoder)
    }
}

impl<T> bincode::Decode<()> for SerdeCacheValue<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + 'static,
{
    fn decode<D: bincode::de::Decoder<Context = ()>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        // Use bincode's serde integration
        let compat = bincode::serde::Compat::<T>::decode(decoder)?;
        Ok(SerdeCacheValue(compat.0))
    }
}

impl<T> CacheValue for SerdeCacheValue<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + 'static,
{
    type Metadata = super::traits::metadata::CacheValueMetadata;

    fn estimated_size(&self) -> usize {
        std::mem::size_of::<T>() + std::mem::size_of::<SerdeCacheValue<T>>()
    }

    fn is_expensive(&self) -> bool {
        // Serializable types are generally expensive to recreate
        self.estimated_size() > 256
    }

    fn metadata(&self) -> Self::Metadata {
        super::traits::metadata::CacheValueMetadata::from_cache_value(self)
    }
}

/// Helper trait for converting types to serde wrappers
// Helper trait - may not be used in minimal API
pub trait ToSerdeCache {
    type Key: CacheKey;
    type Value: CacheValue;

    fn to_serde_key(self) -> Self::Key;
    fn to_serde_value(self) -> Self::Value;
}

impl<T> ToSerdeCache for T
where
    T: Serialize
        + DeserializeOwned
        + Clone
        + Hash
        + Eq
        + Ord
        + Send
        + Sync
        + Debug
        + Default
        + 'static,
{
    type Key = SerdeCacheKey<T>;
    type Value = SerdeCacheValue<T>;

    fn to_serde_key(self) -> Self::Key {
        SerdeCacheKey(self)
    }

    fn to_serde_value(self) -> Self::Value {
        SerdeCacheValue(self)
    }
}

// Manual Deserialize implementations to avoid trait bound conflicts
impl<'de, T> Deserialize<'de> for SerdeCacheKey<T>
where
    T: Serialize
        + DeserializeOwned
        + Clone
        + Hash
        + Eq
        + Ord
        + Send
        + Sync
        + Debug
        + Default
        + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let inner = T::deserialize(deserializer)?;
        Ok(SerdeCacheKey(inner))
    }
}

impl<'de, T> Deserialize<'de> for SerdeCacheValue<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let inner = T::deserialize(deserializer)?;
        Ok(SerdeCacheValue(inner))
    }
}
