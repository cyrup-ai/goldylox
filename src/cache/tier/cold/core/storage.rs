//! Storage and I/O operations for persistent cold tier
//!
//! This module handles all storage-related operations including reading and writing
//! compressed data to memory-mapped files with integrity verification.

use std::io;

use crate::cache::tier::cold::PersistentColdTier;
use crate::cache::tier::cold::compression_engine::CompressedData;
use crate::cache::tier::cold::data_structures::*;
use crate::cache::traits::{CacheKey, CacheValue};

impl<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()>,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Decode<()>
        + bincode::Encode,
> PersistentColdTier<K, V>
{
    /// Read data from storage using metadata
    #[allow(dead_code)] // Cold tier - read_from_storage used in cold tier data retrieval
    pub fn read_from_storage(&self, _key: &K, metadata: &IndexEntry) -> io::Result<V> {
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
        let cache_value =
            bincode::decode_from_slice::<V, _>(&decompressed_data, bincode::config::standard())
                .map(|(v, _)| v)
                .map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Deserialization failed: {:?}", e),
                    )
                })?;

        Ok(cache_value)
    }

    /// Read compressed data from memory-mapped file
    pub(super) fn read_compressed_data(&self, index_entry: &IndexEntry) -> io::Result<Vec<u8>> {
        // Access storage manager directly (worker owns the data, no locks needed)
        let storage = &self.storage_manager;

        if let Some(ref mmap) = storage.data_file {
            let start = index_entry.file_offset as usize;
            let end = start + index_entry.compressed_size as usize;

            if end <= mmap.len() {
                let mut data = vec![0u8; index_entry.compressed_size as usize];

                // SAFETY: Reading from a region that was previously written with atomic ordering
                // The fence ensures we see all writes that happened-before this read
                unsafe {
                    // Ensure we see all prior writes
                    std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);

                    // Read from the memory-mapped region
                    let mmap_ptr = mmap.as_ptr();
                    let src_ptr = mmap_ptr.add(start);
                    std::ptr::copy_nonoverlapping(
                        src_ptr,
                        data.as_mut_ptr(),
                        index_entry.compressed_size as usize,
                    );
                }

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
        // Access storage manager directly (worker owns the data, no locks needed)
        let storage = &self.storage_manager;

        if let Some(offset) = storage.reserve_space(data.len() as u64) {
            if let Some(ref mmap) = storage.data_file {
                let start = offset as usize;
                let end = start + data.len();

                if end <= mmap.len() {
                    // SAFETY: We have exclusive access to this region through atomic reserve_space
                    // The write_position is atomically incremented, ensuring no overlapping writes
                    // Each thread gets a unique offset range that won't be accessed by others
                    unsafe {
                        // Get a mutable pointer to the reserved region
                        let mmap_ptr = mmap.as_ptr() as *mut u8;
                        let dest_ptr = mmap_ptr.add(start);

                        // Use atomic memory ordering for proper synchronization
                        std::ptr::copy_nonoverlapping(data.as_ptr(), dest_ptr, data.len());

                        // Ensure write is visible to other threads
                        std::sync::atomic::fence(std::sync::atomic::Ordering::Release);
                    }

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
