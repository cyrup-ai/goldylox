//! Storage operations for cold tier cache
//!
//! This module handles the file I/O operations for storing and retrieving
//! serialized cache values in the cold tier cache.

use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::Arc;

use super::super::storage::{ColdEntry, ColdTierCache};
use super::binary_format::{deserialize_cache_value, serialize_cache_value};
use super::config::{MAGIC_BYTES, SERIALIZATION_VERSION};
use super::header::StorageHeader;
use super::utilities::calculate_checksum;
use crate::cache::traits::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::traits::CompressionAlgorithm;

impl<K: CacheKey, V: CacheValue + serde::Serialize + serde::de::DeserializeOwned>
    ColdTierCache<K, V>
{
    /// Store cache value in persistent storage
    pub fn put(&self, key: K, value: Arc<V>) -> Result<(), CacheOperationError> {
        // Serialize the cache value (payload only)
        let serialized_payload = serialize_cache_value(&*value, self.compression_algorithm)?;
        let checksum = calculate_checksum(&serialized_payload);

        // Create V1 header for compatibility
        let header = StorageHeader::new_v1(serialized_payload.len() as u32, checksum)?;
        let header_bytes = header.serialize();

        // Get current write position
        let offset = self.write_offset.load(std::sync::atomic::Ordering::Relaxed);

        // Write to file
        {
            let mut file = self.storage_file.lock().map_err(|_| {
                CacheOperationError::resource_exhausted("Storage file lock poisoned")
            })?;
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;

            // Write header using StorageHeader
            file.write_all(&header_bytes)
                .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;

            // Write payload data
            file.write_all(&serialized_payload)
                .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;

            file.flush()
                .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;
        }

        // Update index
        let total_size = header.size() + serialized_payload.len();
        let entry = ColdEntry::new(offset, total_size as u32, checksum);

        {
            let mut index = self
                .index
                .lock()
                .map_err(|_| CacheOperationError::resource_exhausted("Index lock poisoned"))?;
            index.insert(key, entry);
        }

        // Update write offset
        self.write_offset.store(
            offset + total_size as u64,
            std::sync::atomic::Ordering::Relaxed,
        );

        // Record statistics
        Self::record_write(total_size as u32);

        Ok(())
    }

    /// Retrieve cache value from persistent storage
    pub fn get(&self, key: &K) -> Result<Option<Arc<V>>, CacheOperationError> {
        // Check index first
        let entry = {
            let index = self
                .index
                .lock()
                .map_err(|_| CacheOperationError::resource_exhausted("Index read lock poisoned"))?;
            index.get(key).cloned()
        };

        let entry = match entry {
            Some(entry) => entry,
            None => {
                Self::record_miss();
                return Ok(None);
            }
        };

        // Read from file
        let payload_data = {
            let mut file = self
                .storage_file
                .lock()
                .map_err(|_| CacheOperationError::resource_exhausted("File read lock poisoned"))?;
            file.seek(SeekFrom::Start(entry.file_offset))
                .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;

            // Read enough bytes to detect header format and parse it
            let mut header_buffer = vec![0u8; 25]; // Max header size (V2)
            let bytes_read = file.read(&mut header_buffer)
                .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;
            
            if bytes_read < 16 {
                return Err(CacheOperationError::io_failed(
                    "Insufficient data for header".to_string(),
                ));
            }

            // Parse header with version detection
            let (header, header_size) = StorageHeader::deserialize(&header_buffer[..bytes_read])?;

            // Validate header
            header.validate()?;

            // If we read more than the header size, we need to seek back
            let seek_position = entry.file_offset + header_size as u64;
            file.seek(SeekFrom::Start(seek_position))
                .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;

            // Read payload data
            let payload_size = header.data_size() as usize;
            let mut payload_data = vec![0u8; payload_size];
            file.read_exact(&mut payload_data)
                .map_err(|e| CacheOperationError::io_failed(e.to_string()))?;

            // Verify checksum for V1 headers
            if let Some(stored_checksum) = header.checksum() {
                let calculated_checksum = calculate_checksum(&payload_data);
                if calculated_checksum != stored_checksum {
                    return Err(CacheOperationError::io_failed(
                        "Checksum mismatch".to_string(),
                    ));
                }
            }

            payload_data
        };

        // Deserialize using the payload data (no header, just compressed payload)
        let cache_value = deserialize_cache_value(&payload_data, self.compression_algorithm)?;

        // Update access information
        self.update_access(key).map_err(|e| {
            CacheOperationError::resource_exhausted(&format!("Failed to update access: {}", e))
        })?;

        // Record statistics
        Self::record_hit();

        match cache_value {
            Some(value) => Ok(Some(Arc::new(value))),
            None => Ok(None),
        }
    }
}
