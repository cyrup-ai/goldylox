//! Unified header format supporting versioned binary storage
//!
//! This module provides a unified header system that supports both legacy V1 format
//! (compatible with existing storage_ops.rs) and new V2 format (with advanced features).

use std::time::{SystemTime, UNIX_EPOCH};

use super::config::{MAGIC_BYTES, SERIALIZATION_VERSION};
use crate::cache::traits::CacheOperationError;
use crate::cache::traits::CompressionAlgorithm;

/// Unified storage header supporting multiple format versions
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // Cold tier serialization - Complete versioned header system for storage format compatibility
pub enum StorageHeader {
    /// Version 1: Legacy format compatible with existing storage_ops.rs
    /// Format: MAGIC(4) + VERSION(4) + SIZE(4) + CHECKSUM(4) = 16 bytes
    V1 {
        magic_bytes: [u8; 4],
        version: u32,
        size: u32,
        checksum: u32,
    },
    /// Version 2: Advanced format with compression and metadata  
    /// Format: MAGIC(4) + VERSION(4) + ESTIMATED_SIZE(4) + TIMESTAMP(8) + COMPRESSION(1) + COMPRESSED_SIZE(4) = 25 bytes
    V2 {
        magic_bytes: [u8; 4],
        version: u32,
        estimated_size: u32,
        timestamp: u64,
        compression_algorithm: u8,
        compressed_size: u32,
    },
}

impl StorageHeader {
    /// Create V1 header for legacy compatibility
    #[allow(dead_code)] // Cold tier serialization - V1 header constructor for legacy storage format compatibility
    pub fn new_v1(size: u32, checksum: u32) -> Result<Self, CacheOperationError> {
        let magic_bytes: [u8; 4] = MAGIC_BYTES
            .try_into()
            .map_err(|_| CacheOperationError::serialization_failed("Invalid magic bytes length"))?;

        Ok(StorageHeader::V1 {
            magic_bytes,
            version: SERIALIZATION_VERSION,
            size,
            checksum,
        })
    }

    /// Create V2 header with advanced features
    #[allow(dead_code)] // Cold tier serialization - V2 header constructor with compression and timestamp support
    pub fn new_v2(
        estimated_size: u32,
        compression: CompressionAlgorithm,
        compressed_size: u32,
    ) -> Result<Self, CacheOperationError> {
        let magic_bytes: [u8; 4] = MAGIC_BYTES
            .try_into()
            .map_err(|_| CacheOperationError::serialization_failed("Invalid magic bytes length"))?;

        let compression_flag = match compression {
            CompressionAlgorithm::None => 0u8,
            CompressionAlgorithm::Lz4 => 1u8,
            CompressionAlgorithm::Zstd => 2u8,
            CompressionAlgorithm::Deflate => 3u8,
            CompressionAlgorithm::Brotli => 4u8,
        };

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| {
                CacheOperationError::serialization_failed(format!("System time error: {}", e))
            })?
            .as_nanos() as u64;

        Ok(StorageHeader::V2 {
            magic_bytes,
            version: SERIALIZATION_VERSION,
            estimated_size,
            timestamp,
            compression_algorithm: compression_flag,
            compressed_size,
        })
    }

    /// Get the size of this header when serialized
    #[allow(dead_code)] // Cold tier serialization - Header size calculation for storage operations
    pub fn size(&self) -> usize {
        match self {
            StorageHeader::V1 { .. } => 16,
            StorageHeader::V2 { .. } => 25,
        }
    }

    /// Serialize header to bytes
    #[allow(dead_code)] // Cold tier serialization - Header serialization for storage file operations
    pub fn serialize(&self) -> Vec<u8> {
        match self {
            StorageHeader::V1 {
                magic_bytes,
                version,
                size,
                checksum,
            } => {
                let mut bytes = Vec::with_capacity(16);
                bytes.extend_from_slice(magic_bytes);
                bytes.extend_from_slice(&version.to_le_bytes());
                bytes.extend_from_slice(&size.to_le_bytes());
                bytes.extend_from_slice(&checksum.to_le_bytes());
                bytes
            }
            StorageHeader::V2 {
                magic_bytes,
                version,
                estimated_size,
                timestamp,
                compression_algorithm,
                compressed_size,
            } => {
                let mut bytes = Vec::with_capacity(25);
                bytes.extend_from_slice(magic_bytes);
                bytes.extend_from_slice(&version.to_le_bytes());
                bytes.extend_from_slice(&estimated_size.to_le_bytes());
                bytes.extend_from_slice(&timestamp.to_le_bytes());
                bytes.push(*compression_algorithm);
                bytes.extend_from_slice(&compressed_size.to_le_bytes());
                bytes
            }
        }
    }

    /// Deserialize header from bytes with automatic version detection
    /// Returns (header, bytes_consumed)
    #[allow(dead_code)] // Cold tier serialization - Header deserialization with automatic version detection
    pub fn deserialize(data: &[u8]) -> Result<(Self, usize), CacheOperationError> {
        if data.len() < 4 {
            return Err(CacheOperationError::resource_exhausted(
                "Insufficient data for magic bytes",
            ));
        }

        // Check magic bytes first
        let magic_bytes: [u8; 4] = data[0..4]
            .try_into()
            .map_err(|_| CacheOperationError::serialization_failed("Invalid magic bytes"))?;

        if magic_bytes != MAGIC_BYTES {
            return Err(CacheOperationError::serialization_failed(
                "Invalid magic bytes",
            ));
        }

        if data.len() < 8 {
            return Err(CacheOperationError::resource_exhausted(
                "Insufficient data for version",
            ));
        }

        // Proper version detection: read version field first
        let version = Self::detect_version(data)?;

        // Use version to determine format
        match version {
            1 => {
                if data.len() < 16 {
                    return Err(CacheOperationError::resource_exhausted(
                        "Insufficient data for V1 header",
                    ));
                }
                Self::deserialize_v1(data)
            }
            2 => {
                if data.len() < 25 {
                    return Err(CacheOperationError::resource_exhausted(
                        "Insufficient data for V2 header",
                    ));
                }
                Self::deserialize_v2(data)
            }
            v => Err(CacheOperationError::serialization_failed(format!(
                "Unsupported version: {}",
                v
            ))),
        }
    }

    /// Deserialize V1 header format
    #[allow(dead_code)] // Cold tier serialization - V1 format deserialization for legacy compatibility
    fn deserialize_v1(data: &[u8]) -> Result<(Self, usize), CacheOperationError> {
        if data.len() < 16 {
            return Err(CacheOperationError::resource_exhausted(
                "Insufficient data for V1 header",
            ));
        }

        let magic_bytes: [u8; 4] = data[0..4]
            .try_into()
            .map_err(|_| CacheOperationError::serialization_failed("Invalid magic bytes"))?;

        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let size = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        let checksum = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);

        let header = StorageHeader::V1 {
            magic_bytes,
            version,
            size,
            checksum,
        };

        Ok((header, 16))
    }

    /// Deserialize V2 header format
    #[allow(dead_code)] // Cold tier serialization - V2 format deserialization with advanced features
    fn deserialize_v2(data: &[u8]) -> Result<(Self, usize), CacheOperationError> {
        if data.len() < 25 {
            return Err(CacheOperationError::resource_exhausted(
                "Insufficient data for V2 header",
            ));
        }

        let magic_bytes: [u8; 4] = data[0..4]
            .try_into()
            .map_err(|_| CacheOperationError::serialization_failed("Invalid magic bytes"))?;

        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let estimated_size = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        let timestamp = u64::from_le_bytes([
            data[12], data[13], data[14], data[15], data[16], data[17], data[18], data[19],
        ]);
        let compression_algorithm = data[20];
        let compressed_size = u32::from_le_bytes([data[21], data[22], data[23], data[24]]);

        // Validate compression algorithm
        if compression_algorithm > 4 {
            return Err(CacheOperationError::serialization_failed(format!(
                "Invalid compression algorithm: {}",
                compression_algorithm
            )));
        }

        let header = StorageHeader::V2 {
            magic_bytes,
            version,
            estimated_size,
            timestamp,
            compression_algorithm,
            compressed_size,
        };

        Ok((header, 25))
    }
    /// Detect format version from raw bytes
    #[allow(dead_code)] // Cold tier serialization - Version detection for storage format compatibility
    pub fn detect_version(data: &[u8]) -> Result<u32, CacheOperationError> {
        if data.len() < 8 {
            return Err(CacheOperationError::resource_exhausted(
                "Insufficient data for version detection",
            ));
        }

        // Check magic bytes
        if &data[0..4] != MAGIC_BYTES {
            return Err(CacheOperationError::serialization_failed(
                "Invalid magic bytes",
            ));
        }

        // Read version
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        Ok(version)
    }

    /// Get magic bytes from header
    #[allow(dead_code)] // Cold tier serialization - Magic bytes accessor for format validation
    pub fn magic_bytes(&self) -> &[u8; 4] {
        match self {
            StorageHeader::V1 { magic_bytes, .. } => magic_bytes,
            StorageHeader::V2 { magic_bytes, .. } => magic_bytes,
        }
    }

    /// Get version from header
    #[allow(dead_code)] // Cold tier serialization - Version accessor for compatibility checking
    pub fn version(&self) -> u32 {
        match self {
            StorageHeader::V1 { version, .. } => *version,
            StorageHeader::V2 { version, .. } => *version,
        }
    }

    /// Get data size from header (payload size)
    #[allow(dead_code)] // Cold tier serialization - Data size accessor for storage operations
    pub fn data_size(&self) -> u32 {
        match self {
            StorageHeader::V1 { size, .. } => *size,
            StorageHeader::V2 {
                compressed_size, ..
            } => *compressed_size,
        }
    }

    /// Get checksum (V1 only)
    #[allow(dead_code)] // Cold tier serialization - Checksum accessor for V1 format integrity verification
    pub fn checksum(&self) -> Option<u32> {
        match self {
            StorageHeader::V1 { checksum, .. } => Some(*checksum),
            StorageHeader::V2 { .. } => None,
        }
    }

    /// Get compression algorithm (V2 only)
    #[allow(dead_code)] // Cold tier serialization - Compression algorithm accessor for V2 format decompression
    pub fn compression_algorithm(&self) -> Option<CompressionAlgorithm> {
        match self {
            StorageHeader::V1 { .. } => None,
            StorageHeader::V2 {
                compression_algorithm,
                ..
            } => match *compression_algorithm {
                0 => Some(CompressionAlgorithm::None),
                1 => Some(CompressionAlgorithm::Lz4),
                2 => Some(CompressionAlgorithm::Zstd),
                3 => Some(CompressionAlgorithm::Deflate),
                4 => Some(CompressionAlgorithm::Brotli),
                _ => None,
            },
        }
    }

    /// Get timestamp (V2 only)
    #[allow(dead_code)] // Cold tier serialization - Timestamp accessor for V2 format metadata
    pub fn timestamp(&self) -> Option<u64> {
        match self {
            StorageHeader::V1 { .. } => None,
            StorageHeader::V2 { timestamp, .. } => Some(*timestamp),
        }
    }

    /// Validate header consistency
    #[allow(dead_code)] // Cold tier serialization - Header validation for storage integrity checking
    pub fn validate(&self) -> Result<(), CacheOperationError> {
        // Check magic bytes
        if self.magic_bytes() != MAGIC_BYTES {
            return Err(CacheOperationError::serialization_failed(
                "Invalid magic bytes",
            ));
        }

        // Check version
        let version = self.version();
        if version > SERIALIZATION_VERSION {
            return Err(CacheOperationError::serialization_failed(format!(
                "Unsupported version: {} > {}",
                version, SERIALIZATION_VERSION
            )));
        }

        // Additional validation for V2
        if let StorageHeader::V2 {
            compression_algorithm,
            ..
        } = self
            && *compression_algorithm > 4
        {
            return Err(CacheOperationError::serialization_failed(format!(
                "Invalid compression algorithm: {}",
                compression_algorithm
            )));
        }

        Ok(())
    }
}
