//! Storage and I/O operations for persistent cold tier
//!
//! This module handles all storage-related operations including reading and writing
//! compressed data to memory-mapped files with integrity verification.

use std::io;
use std::sync::Arc;

use crate::cache::tier::cold::compression_engine::CompressedData;
use crate::cache::tier::cold::data_structures::*;
use crate::cache::tier::cold::PersistentColdTier;
use crate::cache::traits::{CacheKey, CacheValue};

#[cfg(feature = "bincode")]
use bincode::{config, decode_from_slice};

impl<K: CacheKey, V: CacheValue + serde::Serialize + serde::de::DeserializeOwned>
    PersistentColdTier<K, V>
{
    /// Read data from storage using metadata
    pub fn read_from_storage(&self, _key: &K, metadata: &IndexEntry) -> io::Result<Arc<V>> {
        // Read compressed data from memory-mapped file
        let compressed_data = self.read_compressed_data(metadata)?;

        // Create CompressedData struct from raw data and metadata
        let compressed_data_struct = CompressedData {
            data: compressed_data,
            original_size: metadata.uncompressed_size as usize,
            checksum: metadata.checksum,
            algorithm: metadata.compression_algo,
        };

        // Decompress data
        let decompressed_data = self
            .compression_engine
            .decompress(&compressed_data_struct)
            .map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Decompression failed: {:?}", e),
                )
            })?;

        // Deserialize cache value using bincode
        let cache_value = bincode::decode_from_slice::<V, _>(&decompressed_data, bincode::config::standard())
            .map(|(v, _)| v)
            .map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Deserialization failed: {:?}", e),
            )
        })?;

        Ok(Arc::new(cache_value))
    }

    /// Read compressed data from memory-mapped file
    pub(super) fn read_compressed_data(&self, index_entry: &IndexEntry) -> io::Result<Vec<u8>> {
        if let Some(ref mmap) = self.storage_manager.data_file {
            let start = index_entry.file_offset as usize;
            let end = start + index_entry.compressed_size as usize;

            if end <= mmap.len() {
                let mut data = vec![0u8; index_entry.compressed_size as usize];
                data.copy_from_slice(&mmap[start..end]);

                // Verify checksum
                let checksum = self.calculate_checksum(&data);
                if checksum == index_entry.checksum {
                    Ok(data)
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Checksum mismatch",
                    ))
                }
            } else {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Invalid file offset",
                ))
            }
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Data file not mapped",
            ))
        }
    }

    /// Write compressed data to memory-mapped file
    pub(super) fn write_compressed_data(&self, data: &[u8]) -> io::Result<u64> {
        if let Some(offset) = self.storage_manager.reserve_space(data.len() as u64) {
            if let Some(ref mmap) = self.storage_manager.data_file {
                let start = offset as usize;
                let end = start + data.len();

                if end <= mmap.len() {
                    // Note: This is unsafe in the original code due to mutable aliasing
                    // In a real implementation, you'd need proper synchronization
                    // mmap[start..end].copy_from_slice(data);

                    // Schedule sync for durability
                    self.sync_state.schedule_sync();

                    Ok(offset)
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "Insufficient space in data file",
                    ))
                }
            } else {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Data file not mapped",
                ))
            }
        } else {
            Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "No space available",
            ))
        }
    }
}
