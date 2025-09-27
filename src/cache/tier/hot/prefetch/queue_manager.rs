//! Queue management for prefetch requests
//!
//! This module handles the prefetch request queue, including prioritization,
//! deduplication, and queue maintenance operations.

#![allow(dead_code)] // Hot tier prefetch - Complete queue management library for prefetch request prioritization and coordination

use std::collections::VecDeque;

use super::types::{PredictionConfidence, PrefetchConfig, PrefetchRequest};
use crate::cache::traits::CacheKey;

/// Manages the prefetch request queue with prioritization
#[derive(Debug)]
pub struct QueueManager<K: CacheKey> {
    /// Prefetch queue with prioritization
    prefetch_queue: VecDeque<PrefetchRequest<K>>,
    /// Configuration
    config: PrefetchConfig,
}

impl<K: CacheKey> QueueManager<K> {
    /// Create new queue manager
    pub fn new(config: PrefetchConfig) -> Self {
        Self {
            prefetch_queue: VecDeque::with_capacity(config.prefetch_queue_size),
            config,
        }
    }

    /// Add prefetch request to queue
    pub fn add_prefetch_request(&mut self, request: PrefetchRequest<K>) {
        // Check if already in queue
        if self.prefetch_queue.iter().any(|req| req.key == request.key) {
            return;
        }

        // Add to queue, maintaining priority order
        let insert_pos = self
            .prefetch_queue
            .iter()
            .position(|req| req.priority < request.priority)
            .unwrap_or(self.prefetch_queue.len());

        if insert_pos < self.config.prefetch_queue_size {
            self.prefetch_queue.insert(insert_pos, request);

            // Trim queue if too large
            if self.prefetch_queue.len() > self.config.prefetch_queue_size {
                self.prefetch_queue.pop_back();
            }
        }
    }

    /// Add multiple prefetch requests
    pub fn add_prefetch_requests(&mut self, requests: Vec<PrefetchRequest<K>>) {
        for request in requests {
            self.add_prefetch_request(request);
        }
    }

    /// Get next prefetch request
    pub fn get_next_prefetch(&mut self) -> Option<PrefetchRequest<K>> {
        self.prefetch_queue.pop_front()
    }

    /// Get multiple prefetch requests
    pub fn get_next_prefetches(&mut self, count: usize) -> Vec<PrefetchRequest<K>> {
        let mut requests = Vec::with_capacity(count);

        for _ in 0..count {
            if let Some(request) = self.prefetch_queue.pop_front() {
                requests.push(request);
            } else {
                break;
            }
        }

        requests
    }

    /// Check if key should be prefetched
    pub fn should_prefetch(&self, key: &K) -> bool {
        self.prefetch_queue
            .iter()
            .any(|req| req.key == *key && req.confidence != PredictionConfidence::Low)
    }

    /// Remove request from queue
    pub fn remove_request(&mut self, key: &K) -> bool {
        if let Some(pos) = self.prefetch_queue.iter().position(|req| req.key == *key) {
            self.prefetch_queue.remove(pos);
            true
        } else {
            false
        }
    }

    /// Get queue status
    #[allow(dead_code)] // Hot tier prefetch - Queue status monitoring for capacity management
    pub fn queue_status(&self) -> (usize, usize) {
        (self.prefetch_queue.len(), self.config.prefetch_queue_size)
    }

    /// Get queue length
    #[allow(dead_code)] // Hot tier prefetch - Queue length monitoring for load analysis
    pub fn queue_len(&self) -> usize {
        self.prefetch_queue.len()
    }

    /// Check if queue is empty
    #[allow(dead_code)] // Hot tier prefetch - Empty queue check for scheduling optimization
    pub fn is_empty(&self) -> bool {
        self.prefetch_queue.is_empty()
    }

    /// Check if queue is full
    #[allow(dead_code)] // Hot tier prefetch - Full queue check for backpressure management
    pub fn is_full(&self) -> bool {
        self.prefetch_queue.len() >= self.config.prefetch_queue_size
    }

    /// Clear all requests from queue
    #[allow(dead_code)] // Hot tier prefetch - Queue clearing for reset and cleanup operations
    pub fn clear(&mut self) {
        self.prefetch_queue.clear();
    }

    /// Get requests by confidence level
    #[allow(dead_code)] // Hot tier prefetch - Confidence-based request filtering for priority scheduling
    pub fn get_requests_by_confidence(
        &self,
        min_confidence: PredictionConfidence,
    ) -> Vec<&PrefetchRequest<K>> {
        self.prefetch_queue
            .iter()
            .filter(|req| {
                self.confidence_level(req.confidence) >= self.confidence_level(min_confidence)
            })
            .collect()
    }

    /// Convert confidence to numeric level for comparison
    fn confidence_level(&self, confidence: PredictionConfidence) -> u8 {
        match confidence {
            PredictionConfidence::Low => 1,
            PredictionConfidence::Medium => 2,
            PredictionConfidence::High => 3,
            PredictionConfidence::VeryHigh => 4,
        }
    }

    /// Get requests by priority range
    #[allow(dead_code)] // Hot tier prefetch - Priority-based request filtering for adaptive scheduling
    pub fn get_requests_by_priority(&self, min_priority: u8) -> Vec<&PrefetchRequest<K>> {
        self.prefetch_queue
            .iter()
            .filter(|req| req.priority >= min_priority)
            .collect()
    }

    /// Reorder queue by priority
    #[allow(dead_code)] // Hot tier prefetch - Priority-based queue reordering for optimal scheduling
    pub fn reorder_by_priority(&mut self) {
        // Convert to Vec, sort, and convert back
        let mut requests: Vec<_> = self.prefetch_queue.drain(..).collect();
        requests.sort_by(|a, b| b.priority.cmp(&a.priority));
        self.prefetch_queue.extend(requests);
    }

    /// Remove expired requests based on predicted access time
    #[allow(dead_code)] // Hot tier prefetch - Expired request cleanup for memory management
    pub fn remove_expired_requests(
        &mut self,
        current_time_ns: u64,
        expiry_threshold_ns: u64,
    ) -> usize {
        let initial_len = self.prefetch_queue.len();

        self.prefetch_queue
            .retain(|req| current_time_ns < req.predicted_access_time + expiry_threshold_ns);

        initial_len - self.prefetch_queue.len()
    }

    /// Get queue statistics
    pub fn get_queue_stats(&self) -> QueueStats {
        let total_requests = self.prefetch_queue.len();

        let mut confidence_distribution = [0; 4];
        let mut priority_sum = 0u64;

        for request in &self.prefetch_queue {
            let confidence_idx = match request.confidence {
                PredictionConfidence::Low => 0,
                PredictionConfidence::Medium => 1,
                PredictionConfidence::High => 2,
                PredictionConfidence::VeryHigh => 3,
            };
            confidence_distribution[confidence_idx] += 1;
            priority_sum += request.priority as u64;
        }

        let avg_priority = if total_requests > 0 {
            priority_sum as f64 / total_requests as f64
        } else {
            0.0
        };

        QueueStats {
            total_requests,
            capacity: self.config.prefetch_queue_size,
            utilization: total_requests as f64 / self.config.prefetch_queue_size as f64,
            avg_priority,
            confidence_distribution: ConfidenceDistribution {
                low: confidence_distribution[0],
                medium: confidence_distribution[1],
                high: confidence_distribution[2],
                very_high: confidence_distribution[3],
            },
        }
    }

    /// Peek at next request without removing it
    pub fn peek_next(&self) -> Option<&PrefetchRequest<K>> {
        self.prefetch_queue.front()
    }

    /// Peek at multiple requests without removing them
    pub fn peek_next_multiple(&self, count: usize) -> Vec<&PrefetchRequest<K>> {
        self.prefetch_queue.iter().take(count).collect()
    }

    /// Check if specific key is in queue
    pub fn contains_key(&self, key: &K) -> bool {
        self.prefetch_queue.iter().any(|req| req.key == *key)
    }

    /// Get position of key in queue
    pub fn get_key_position(&self, key: &K) -> Option<usize> {
        self.prefetch_queue.iter().position(|req| req.key == *key)
    }

    /// Update configuration
    pub fn update_config(&mut self, new_config: PrefetchConfig) {
        self.config = new_config;

        // Resize queue if necessary
        if self.prefetch_queue.len() > self.config.prefetch_queue_size {
            self.prefetch_queue
                .truncate(self.config.prefetch_queue_size);
        }
    }
}

/// Queue statistics
#[derive(Debug, Clone)]
pub struct QueueStats {
    pub total_requests: usize,
    #[allow(dead_code)] // Hot tier prefetch - Queue capacity tracking for resource management
    pub capacity: usize,
    #[allow(dead_code)] // Hot tier prefetch - Queue utilization metrics for performance monitoring
    pub utilization: f64,
    #[allow(dead_code)]
    // Hot tier prefetch - Average priority calculation for scheduling optimization
    pub avg_priority: f64,
    pub confidence_distribution: ConfidenceDistribution,
}

/// Distribution of confidence levels in queue
#[derive(Debug, Clone)]
pub struct ConfidenceDistribution {
    pub low: usize,
    pub medium: usize,
    pub high: usize,
    pub very_high: usize,
}
