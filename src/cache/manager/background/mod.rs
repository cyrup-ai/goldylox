//! Background operation coordination and task management
//!
//! This module handles background tasks, maintenance scheduling, and worker coordination
//! for cache operations that don't need to block the main request path.

pub mod scheduler;
pub mod statistics;
pub mod types;
pub mod worker;
pub mod worker_state;

// Re-export main types and structs
// Re-export coordinator implementation

pub use types::{
    BackgroundWorkerState, MaintenanceConfig, MaintenanceScheduler,
    MaintenanceStats, MaintenanceTask, MaintenanceTaskType, WorkerStatus,
};
