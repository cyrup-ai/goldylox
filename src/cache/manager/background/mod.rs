//! Background operation coordination and task management
//!
//! This module handles background tasks, maintenance scheduling, and worker coordination
//! for cache operations that don't need to block the main request path.

pub mod scheduler;
pub mod statistics;
pub mod types;
pub mod worker;
pub mod worker_state;

// NO RE-EXPORTS - Use canonical imports only:
// - Use crate::cache::manager::background::types::MaintenanceScheduler
// - Use crate::cache::manager::background::types::MaintenanceConfig
// - Use crate::cache::manager::background::types::BackgroundWorkerState
