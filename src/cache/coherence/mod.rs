//! MESI-like cache coherence protocol implementation
//!
//! This module provides a complete cache coherence protocol implementation
//! decomposed into logical submodules for maintainability and clarity.

pub mod communication;
pub mod data_structures;
pub mod invalidation;
pub mod protocol;
pub mod state_management;
pub mod statistics;
pub mod worker;
pub mod write_propagation;

// Worker types available through direct submodule access only
pub use data_structures::{CacheTier, CoherenceController, ProtocolConfiguration};
// Worker manager and protocol API exports removed - not used in current implementation

// Internal types remain accessible within coherence module but not exported
pub(crate) use communication::{CoherenceError, CoherenceMessage};
pub(crate) use data_structures::{CacheLineState, CoherenceKey, InvalidationReason, MesiState};
pub(crate) use invalidation::InvalidationStatistics;
// Remove unused protocol functions
pub(crate) use state_management::{StateTransitionRequest, TransitionReason};
pub(crate) use statistics::{CoherenceStatistics, CoherenceStatisticsSnapshot};
// Remove unused write propagation imports
