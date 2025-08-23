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
pub mod write_propagation;

// Re-export main types for convenience
pub use communication::{
    CoherenceError, CoherenceMessage, CommunicationHub, ExclusiveResponse, MessageStatistics,
    MessageStatisticsSnapshot, ReadResponse, WriteResponse,
};
pub use data_structures::{
    CacheLineState, CacheTier, CoherenceController, CoherenceKey, CoherenceMetadata,
    InvalidationReason, MesiState, ProtocolConfiguration,
};
pub use invalidation::{
    InvalidationConfig, InvalidationManager, InvalidationPriority, InvalidationRequest,
    InvalidationResult, InvalidationStatistics, InvalidationStatisticsSnapshot,
};
// Re-export protocol implementation and global functions
pub use protocol::{coherent_read, coherent_write, init_coherence_controller};
pub use state_management::{
    StateTransitionRequest, StateTransitionValidator, TransitionReason, TransitionResult,
    TransitionStatistics, TransitionStatisticsSnapshot, ViolationDetector,
    ViolationStatisticsSnapshot,
};
pub use statistics::{
    CoherencePerformanceMetrics, CoherenceStatistics, CoherenceStatisticsSnapshot,
    OperationMetrics, TierCoherenceStats, TierSpecificStats,
};
pub use write_propagation::{
    PropagationConfig, PropagationPolicy, PropagationStatistics, PropagationStatisticsSnapshot,
    WorkerChannels, WorkerHealth, WriteBackCompletion, WriteBackRequest, WriteBackResult,
    WriteBackTask, WritePriority, WritePropagationSystem,
};
