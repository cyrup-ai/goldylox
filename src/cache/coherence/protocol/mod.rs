//! Coherence protocol implementation
//!
//! This module implements the core coherence controller logic that coordinates
//! all coherence operations across cache tiers using the MESI protocol.

// Internal protocol architecture - components may not be used in minimal API

pub mod exclusive_access;
pub mod global_api;
pub mod message_handling;
pub mod read_operations;
pub mod types;
pub mod write_operations;

// Re-export main types and functions
// Removed broken re-export - use direct import: crate::cache::coherence::data_structures::CoherenceController
