//! Invalidation manager for coordinated invalidations
//!
//! This module implements the main InvalidationManager that coordinates
//! invalidations across cache tiers and handles request processing.

// Internal coherence architecture - components may not be used in minimal API

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use crossbeam_skiplist::SkipMap;

use super::configuration::InvalidationConfig;
use super::types::{InvalidationPriority, InvalidationRequest};
use crate::cache::coherence::InvalidationStatistics;
use crate::cache::coherence::communication::CoherenceMessage;
use crate::cache::coherence::data_structures::{CacheTier, CoherenceKey, InvalidationReason};
use crate::cache::traits::{CacheKey, CacheValue};

/// Invalidation manager for coordinated invalidations
#[derive(Debug)]
pub struct InvalidationManager<K: CacheKey> {
    /// Pending invalidations
    pub pending_invalidations: SkipMap<CoherenceKey<K>, InvalidationRequest<K>>,
    /// Invalidation sequence counter
    pub sequence_counter: AtomicU32,
    /// Invalidation statistics
    pub invalidation_stats: InvalidationStatistics,
    /// Configuration parameters
    pub config: InvalidationConfig,
}
impl<K: CacheKey> InvalidationManager<K> {
    pub fn new(_max_pending: u32) -> Self {
        Self {
            pending_invalidations: SkipMap::new(),
            sequence_counter: AtomicU32::new(0),
            invalidation_stats: InvalidationStatistics::new(),
            config: InvalidationConfig::default(),
        }
    }

    /// Submit invalidation request
    pub fn submit_invalidation(
        &self,
        key: CoherenceKey<K>,
        target_tier: CacheTier,
        reason: InvalidationReason,
        priority: InvalidationPriority,
    ) -> u32 {
        let sequence = self.sequence_counter.fetch_add(1, Ordering::AcqRel);

        let request = InvalidationRequest {
            key: key.clone(),
            target_tier,
            reason,
            sequence,
            created_at: Instant::now(),
            retry_count: 0,
            last_attempt: None,
            priority,
        };

        self.pending_invalidations.insert(key, request);
        self.invalidation_stats
            .total_requested
            .fetch_add(1, Ordering::Relaxed);

        // Update peak pending count
        let current_pending = self.pending_invalidations.len() as u32;
        let current_peak = self
            .invalidation_stats
            .peak_pending_count
            .load(Ordering::Relaxed);
        if current_pending > current_peak {
            self.invalidation_stats
                .peak_pending_count
                .store(current_pending, Ordering::Relaxed);
        }

        sequence
    }

    /// Process pending invalidations
    pub fn process_pending<V: CacheValue>(&self) -> Vec<CoherenceMessage<K, V>> {
        let mut messages = Vec::new();
        let now = Instant::now();
        let mut completed_keys = Vec::new();

        // Collect requests to process (sorted by priority and sequence)
        let mut requests: Vec<_> = self
            .pending_invalidations
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();

        // Sort by priority (high to low) then by sequence (low to high)
        requests.sort_by(|(_, a), (_, b)| {
            b.priority
                .cmp(&a.priority)
                .then(a.sequence.cmp(&b.sequence))
        });

        for (key, mut request) in requests {
            // Check if request has timed out
            if now.duration_since(request.created_at).as_millis() as u64
                > self.config.request_timeout_ms
            {
                self.invalidation_stats
                    .timed_out
                    .fetch_add(1, Ordering::Relaxed);
                completed_keys.push(key);
                continue;
            }

            // Check if we should retry
            if let Some(last_attempt) = request.last_attempt
                && (now.duration_since(last_attempt).as_millis() as u64)
                    < self.config.retry_delay_ms
            {
                continue; // Too soon to retry
            }

            // Check retry limit
            if request.retry_count >= self.config.max_retries {
                self.invalidation_stats
                    .failed
                    .fetch_add(1, Ordering::Relaxed);
                completed_keys.push(key);
                continue;
            }

            // Create invalidation message
            let message = CoherenceMessage::Invalidate {
                key: request.key.clone(),
                target_tier: request.target_tier,
                reason: request.reason,
                sequence: request.sequence,
                timestamp_ns: now.elapsed().as_nanos() as u64,
            };

            messages.push(message);

            // Update request
            request.retry_count += 1;
            request.last_attempt = Some(now);

            if request.retry_count > 1 {
                self.invalidation_stats
                    .retried
                    .fetch_add(1, Ordering::Relaxed);
            }

            // Update the request in the map
            self.pending_invalidations.insert(key, request);
        }

        // Remove completed requests
        for key in completed_keys {
            self.pending_invalidations.remove(&key);
        }

        messages
    }

    /// Mark invalidation as completed
    pub fn mark_completed(&self, key: &CoherenceKey<K>, success: bool) {
        if let Some(entry) = self.pending_invalidations.remove(key) {
            let request = entry.value().clone();
            let processing_time = request.created_at.elapsed().as_nanos() as u64;

            if success {
                self.invalidation_stats
                    .successful
                    .fetch_add(1, Ordering::Relaxed);
            } else {
                self.invalidation_stats
                    .failed
                    .fetch_add(1, Ordering::Relaxed);
            }

            // Update average processing time
            let current_avg = self
                .invalidation_stats
                .avg_processing_time_ns
                .load(Ordering::Relaxed);
            let new_avg = if current_avg == 0 {
                processing_time
            } else {
                (current_avg * 7 + processing_time) / 8 // Exponential moving average
            };
            self.invalidation_stats
                .avg_processing_time_ns
                .store(new_avg, Ordering::Relaxed);
        }
    }

    /// Get pending invalidation count
    #[allow(dead_code)] // MESI coherence - used in invalidation management and coordination
    pub fn pending_count(&self) -> usize {
        self.pending_invalidations.len()
    }

    /// Clear all pending invalidations
    #[allow(dead_code)] // MESI coherence - used in invalidation management and coordination
    pub fn clear_pending(&self) {
        self.pending_invalidations.clear();
    }

    /// Get invalidations for specific tier
    #[allow(dead_code)] // MESI coherence - used in invalidation management and coordination
    pub fn get_tier_invalidations(&self, tier: CacheTier) -> Vec<InvalidationRequest<K>> {
        self.pending_invalidations
            .iter()
            .filter_map(|entry| {
                let request = entry.value();
                if request.target_tier == tier {
                    Some(request.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}
