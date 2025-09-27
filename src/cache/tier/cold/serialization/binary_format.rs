//! This module provides generic binary serialization for any cache value
//! that implements the CacheValue trait, making the cache truly generic.

use bincode::{config, decode_from_slice, encode_to_vec};
use brotli;
use lz4_flex;
use serde::{Serialize, de::DeserializeOwned};
use std::io::{Cursor, Read};
use zstd;

use crate::cache::traits::CacheOperationError;
use crate::cache::traits::CacheValue;
// Supporting traits removed - using simplified trait system
// Note: CompressionAlgorithm is defined in both supporting_types and types_and_enums
// We need to use the one from types_and_enums to match SerializationContext
use crate::cache::traits::CompressionAlgorithm;

// Constants removed - now handled by config module and header.rs

// BinaryFormatHeader removed - now handled by header.rs unified StorageHeader

// Migration logic removed - now handled by header.rs StorageHeader

/// Helper function to serialize any value using bincode
#[inline]
#[allow(dead_code)] // Cold tier serialization - Helper function for bincode serialization operations
fn serialize_with_bincode<V: Serialize + bincode::Encode>(value: &V) -> Result<Vec<u8>, String> {
    encode_to_vec(value, config::standard())
        .map_err(|e| format!("Bincode serialization failed: {}", e))
}

/// Helper function to deserialize any value using bincode
#[inline]
fn deserialize_with_bincode<V: DeserializeOwned + bincode::Decode<()>>(
    data: &[u8],
) -> Result<V, String> {
    decode_from_slice(data, config::standard())
        .map(|(v, _)| v)
        .map_err(|e| format!("Bincode deserialization failed: {}", e))
}

/// Serialize cache value to binary format (payload only, no headers)
///
/// This function handles only payload serialization and compression.
/// Header management is handled separately by storage_ops.rs using StorageHeader.
#[allow(dead_code)] // Cold tier serialization - Core serialization function for cache values with compression support
pub fn serialize_cache_value<V: CacheValue + Serialize + bincode::Encode>(
    value: &V,
    compression_algorithm: CompressionAlgorithm,
) -> Result<Vec<u8>, CacheOperationError> {
    // Use standard bincode serialization for cold tier
    // Compression algorithm is now passed as parameter

    // Serialize the value using bincode for cold tier
    let serialized_data = encode_to_vec(value, config::standard()).map_err(|e| {
        CacheOperationError::SerializationError(format!("Bincode serialization failed: {}", e))
    })?;

    // Apply compression based on context settings
    let compressed_data = compress_data(&serialized_data, compression_algorithm)?;

    // Return compressed payload only (no header)
    Ok(compressed_data)
}

/// Deserialize cache value from binary format (payload only, no headers)
///
/// This function handles only payload decompression and deserialization.
/// Header parsing is handled separately by storage_ops.rs using StorageHeader.
/// The compression algorithm is determined from the header and passed as a parameter.
#[allow(dead_code)] // Cold tier serialization - Core deserialization function for cache values with compression support
pub fn deserialize_cache_value<V: CacheValue + DeserializeOwned + bincode::Decode<()>>(
    compressed_data: &[u8],
    compression_algorithm: CompressionAlgorithm,
) -> Result<Option<V>, CacheOperationError> {
    // Compression algorithm is now passed as parameter

    // Decompress the data
    let decompressed_data = decompress_data(compressed_data, compression_algorithm)?;

    // Deserialize using bincode
    let deserialized_value = deserialize_with_bincode::<V>(&decompressed_data).map_err(|e| {
        CacheOperationError::serialization_failed(format!("Bincode deserialization failed: {}", e))
    })?;

    Ok(Some(deserialized_value))
}

/// Compress data using the specified algorithm
#[inline]
#[allow(dead_code)] // Cold tier serialization - Multi-algorithm compression engine supporting LZ4, ZSTD, Deflate, Brotli
pub fn compress_data(
    data: &[u8],
    algorithm: CompressionAlgorithm,
) -> Result<Vec<u8>, CacheOperationError> {
    match algorithm {
        CompressionAlgorithm::None => Ok(data.to_vec()),

        CompressionAlgorithm::Lz4 => Ok(lz4_flex::compress_prepend_size(data)),

        CompressionAlgorithm::Zstd => zstd::encode_all(data, 3).map_err(|e| {
            CacheOperationError::serialization_failed(format!("ZSTD compression failed: {}", e))
        }),

        CompressionAlgorithm::Deflate => {
            use flate2::{Compression, write::DeflateEncoder};
            use std::io::Write;

            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(data).map_err(|e| {
                CacheOperationError::serialization_failed(format!(
                    "Deflate compression failed: {}",
                    e
                ))
            })?;
            encoder.finish().map_err(|e| {
                CacheOperationError::serialization_failed(format!(
                    "Deflate compression finish failed: {}",
                    e
                ))
            })
        }

        CompressionAlgorithm::Brotli => {
            let mut output = Vec::new();
            let params = brotli::enc::BrotliEncoderParams::default();
            brotli::enc::BrotliCompress(&mut Cursor::new(data), &mut output, &params).map_err(
                |e| {
                    CacheOperationError::serialization_failed(format!(
                        "Brotli compression failed: {:?}",
                        e
                    ))
                },
            )?;
            Ok(output)
        }
    }
}

/// Decompress data using the specified algorithm
#[inline]
#[allow(dead_code)] // Cold tier serialization - Multi-algorithm decompression engine supporting LZ4, ZSTD, Deflate, Brotli
pub fn decompress_data(
    data: &[u8],
    algorithm: CompressionAlgorithm,
) -> Result<Vec<u8>, CacheOperationError> {
    match algorithm {
        CompressionAlgorithm::None => Ok(data.to_vec()),

        CompressionAlgorithm::Lz4 => lz4_flex::decompress_size_prepended(data)
            .map_err(|_| CacheOperationError::serialization_failed("LZ4 decompression failed")),

        CompressionAlgorithm::Zstd => zstd::decode_all(data).map_err(|e| {
            CacheOperationError::serialization_failed(format!("ZSTD decompression failed: {}", e))
        }),

        CompressionAlgorithm::Deflate => {
            use flate2::write::DeflateDecoder;
            use std::io::Write;

            let mut decoder = DeflateDecoder::new(Vec::new());
            decoder.write_all(data).map_err(|e| {
                CacheOperationError::serialization_failed(format!(
                    "Deflate decompression failed: {}",
                    e
                ))
            })?;
            decoder.finish().map_err(|e| {
                CacheOperationError::serialization_failed(format!(
                    "Deflate decompression finish failed: {}",
                    e
                ))
            })
        }

        CompressionAlgorithm::Brotli => {
            let mut output = Vec::new();
            let mut decompressor = brotli::Decompressor::new(Cursor::new(data), 4096);
            decompressor.read_to_end(&mut output).map_err(|e| {
                CacheOperationError::serialization_failed(format!(
                    "Brotli decompression failed: {}",
                    e
                ))
            })?;
            Ok(output)
        }
    }
}

/// Get estimated size for a cache value with compression ratio consideration
#[allow(dead_code)] // Cold tier serialization - Size estimation utility for capacity planning and memory management
pub fn estimate_serialized_size<V: CacheValue + Serialize>(value: &V) -> usize {
    let base_size = value.estimated_size();
    // Default compression ratio estimate for cold tier (Zstd)
    let compression_ratio = 0.7; // ~30% compression expected

    // Apply compression ratio estimate for size calculation
    (base_size as f32 * compression_ratio) as usize
}

// Validation logic removed - now handled by header.rs StorageHeader::validate()
