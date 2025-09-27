//! Inter-process communication types for loxd daemon
//!
//! Defines commands and events used for communication between the daemon
//! manager and cache workers.

use chrono::{DateTime, Utc};

/// Commands sent *to* a cache worker thread.
#[derive(Debug)]
pub enum Cmd {
    Start,
    Stop,
    Restart,
    Shutdown,   // worker should exit
    TickHealth, // periodic health probe
    TickStats,  // periodic statistics collection
}

/// Events emitted *from* workers back to the manager.
#[derive(Debug, Clone)]
pub enum Evt {
    State {
        service: String,
        kind: &'static str, // "running"|"stopped"|etc.
        ts: DateTime<Utc>,
        pid: Option<u32>,
    },
    Health {
        service: String,
        healthy: bool,
        ts: DateTime<Utc>,
    },
    Stats {
        service: String,
        hits: u64,
        misses: u64,
        evictions: u64,
        memory_used: u64,
        ts: DateTime<Utc>,
    },
    Fatal {
        service: String,
        msg: &'static str,
        ts: DateTime<Utc>,
    },
}
