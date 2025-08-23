//! Coherence protocol implementation
//!
//! This module implements the core coherence controller logic that coordinates
//! all coherence operations across cache tiers using the MESI protocol.

pub mod exclusive_access;
pub mod global_api;
pub mod message_handling;
pub mod read_operations;
pub mod types;
pub mod write_operations;

// Re-export main types and functions
pub use global_api::{coherent_read, coherent_write, init_coherence_controller};
pub use types::CoherenceController;
