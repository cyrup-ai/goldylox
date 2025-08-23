//! Utility functions for cold tier serialization
//!
//! This module contains helper functions for checksum calculation
//! and other serialization utilities.

/// Calculate checksum for integrity verification
pub fn calculate_checksum(data: &[u8]) -> u32 {
    let mut checksum = 0u32;
    for byte in data {
        checksum = checksum.wrapping_add(*byte as u32);
        checksum = checksum.wrapping_mul(31);
    }
    checksum
}
