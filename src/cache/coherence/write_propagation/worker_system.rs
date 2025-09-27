//! Background worker system for write propagation
//!
//! This module handles background task processing, worker communication,
//! and task completion tracking for the write propagation system.

use std::time::Instant;

use crossbeam_channel::{Receiver, Sender};

use super::types::WriteBackRequest;
use crate::cache::coherence::data_structures::CoherenceKey;
use crate::cache::traits::{CacheKey, CacheValue};

/// Background worker communication channels
#[derive(Debug)]
pub struct WorkerChannels<K: CacheKey, V: CacheValue> {
    /// Task submission channel to background worker
    pub task_tx: Sender<WriteBackTask<K, V>>,
    pub task_rx: Receiver<WriteBackTask<K, V>>,
    /// Completion notification channel from background worker
    pub completion_tx: Sender<WriteBackCompletion>,
    pub completion_rx: Receiver<WriteBackCompletion>,
}

/// Background writeback task for worker processing
#[derive(Debug, Clone)]
pub struct WriteBackTask<K: CacheKey, V: CacheValue> {
    /// Task ID for completion tracking
    pub task_id: u64,
    /// Request to be processed  
    pub request: WriteBackRequest<CoherenceKey<K>, V>, // Generic coherence key
    /// Submission timestamp
    #[allow(dead_code)]
    // MESI coherence - used in background worker task scheduling and timeout management
    pub submitted_at: Instant,
    /// Maximum processing time
    #[allow(dead_code)]
    // MESI coherence - used in background worker task scheduling and timeout management
    pub timeout_ns: u64,
}

/// Task completion notification from background worker
#[derive(Debug, Clone)]
pub struct WriteBackCompletion {
    /// Completed task ID
    #[allow(dead_code)]
    // MESI coherence - used in background worker completion tracking and task correlation
    pub task_id: u64,
    /// Processing result
    pub result: WriteBackResult,
    /// Processing duration
    pub processing_time_ns: u64,
    /// Completion timestamp
    #[allow(dead_code)]
    // MESI coherence - used in background worker completion tracking and performance analysis
    pub completed_at: Instant,
}

/// Background worker processing result
#[derive(Debug, Clone)]
pub enum WriteBackResult {
    /// Successfully written to target tier
    Success,
    /// Failed with recoverable error (should retry)
    RetryableError { reason: String },
    /// Failed with permanent error (should not retry)
    PermanentError { reason: String },
    /// Processing timed out
    Timeout,
}
