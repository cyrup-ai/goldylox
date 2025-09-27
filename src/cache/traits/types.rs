//! Type definitions and enumerations used throughout the traits system.
//!
//! This module contains remaining unique types that are not duplicated in types_and_enums.rs

#![allow(dead_code)] // Cache traits - Core type definitions for cache traits

// Internal trait types - may not be used in minimal API

use std::time::Instant;

use super::types_and_enums::TierAffinity;

/// Cache entry metadata (unique to this module)
#[derive(Debug, Clone)]
pub struct CacheEntryMetadata {
    pub created_at: Instant,
    pub last_accessed: Instant,
    pub access_count: u64,
    pub size_bytes: usize,
    pub tier_affinity: TierAffinity,
}
