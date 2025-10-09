//! Concurrent access tracker for pattern recognition
//!
//! This module implements the main access tracking functionality with
//! lock-free data structures and background analysis processing.

#![allow(dead_code)] // Warm tier access tracking - Complete access tracker library for lock-free access pattern monitoring

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender, unbounded};
use crossbeam_skiplist::SkipMap;

use super::frequency_estimator::FrequencyEstimator;
use super::pattern_classifier::TemporalPatternClassifier;
use super::types::{AccessRecord, HitStatus};
use crate::cache::tier::warm::core::WarmCacheKey;
use crate::cache::traits::{AccessType, CacheKey};
use crate::cache::types::timestamp_nanos;

/// Access analysis task for background processing
#[derive(Debug, Clone)]
pub enum AccessAnalysisTask<K: CacheKey> {
    /// Analyze access pattern
    AnalyzePattern {
        key: WarmCacheKey<K>,
        access_type: AccessType,
        timestamp_ns: u64,
    },
    /// Update frequency estimates
    UpdateFrequency {
        key: WarmCacheKey<K>,
        new_frequency: f64,
    },
    /// Classify temporal patterns
    ClassifyPattern {
        access_sequence: Vec<u64>,
        window_size: usize,
    },
}

/// Concurrent access tracker for pattern recognition
#[derive(Debug)]
pub struct ConcurrentAccessTracker<K: CacheKey> {
    /// Recent access patterns (lock-free ring buffer)
    pub(crate) recent_accesses: SkipMap<u64, AccessRecord>,
    /// Temporal pattern classifier
    pattern_classifier: TemporalPatternClassifier,
    /// Access frequency estimator
    frequency_estimator: FrequencyEstimator<K>,
    /// Pattern analysis background channel
    analysis_tx: Sender<AccessAnalysisTask<K>>,
    analysis_rx: Receiver<AccessAnalysisTask<K>>,
}

impl<K: CacheKey> Default for ConcurrentAccessTracker<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: CacheKey> ConcurrentAccessTracker<K> {
    /// Create new concurrent access tracker
    pub fn new() -> Self {
        let (analysis_tx, analysis_rx) = unbounded();

        Self {
            recent_accesses: SkipMap::new(),
            pattern_classifier: TemporalPatternClassifier::new(),
            frequency_estimator: FrequencyEstimator::new(),
            analysis_tx,
            analysis_rx,
        }
    }

    /// Record access for pattern analysis
    pub fn record_access(&self, key: &WarmCacheKey<K>, access_type: AccessType, hit: bool) {
        let now_ns = timestamp_nanos(Instant::now());
        let thread_id = self.get_thread_id();
        let key_hash = key.cache_hash();

        let record = AccessRecord {
            timestamp_ns: now_ns,
            access_type,
            thread_id,
            hit_status: if hit { HitStatus::Hit } else { HitStatus::Miss },
            key_hash,
        };

        // Insert with timestamp as key for ordering
        self.recent_accesses.insert(now_ns, record);

        // Cleanup old entries (keep only recent history)
        self.cleanup_old_accesses(now_ns);

        // Send for background analysis
        let task = AccessAnalysisTask::AnalyzePattern {
            key: key.clone(),
            access_type,
            timestamp_ns: now_ns,
        };

        let _ = self.analysis_tx.try_send(task);
    }

    /// Cleanup access records older than analysis window
    fn cleanup_old_accesses(&self, current_ns: u64) {
        let cutoff_ns = current_ns.saturating_sub(300_000_000_000); // 5 minutes

        // Remove entries older than cutoff
        let mut to_remove = Vec::new();
        for entry in self.recent_accesses.iter() {
            if *entry.key() < cutoff_ns {
                to_remove.push(*entry.key());
            } else {
                break; // Skip list is ordered
            }
        }

        for key in to_remove {
            self.recent_accesses.remove(&key);
        }
    }

    /// Get current thread ID for analysis using stable APIs
    fn get_thread_id(&self) -> u64 {
        // Use the ThreadId hash as a stable identifier
        let thread_id = std::thread::current().id();
        let mut hasher = DefaultHasher::new();
        thread_id.hash(&mut hasher);
        hasher.finish()
    }

    /// Get access pattern for key within time window
    pub fn get_access_pattern(&self, _key: &WarmCacheKey<K>, window_ns: u64) -> Vec<AccessRecord> {
        let now_ns = timestamp_nanos(Instant::now());
        let cutoff_ns = now_ns.saturating_sub(window_ns);

        let mut pattern = Vec::new();

        for entry in self.recent_accesses.range(cutoff_ns..) {
            pattern.push(entry.value().clone());
        }

        pattern
    }

    /// Process background analysis tasks
    pub fn process_analysis_tasks(&self) {
        while let Ok(task) = self.analysis_rx.try_recv() {
            match task {
                AccessAnalysisTask::AnalyzePattern {
                    key,
                    access_type,
                    timestamp_ns,
                } => {
                    self.analyze_access_pattern(&key, access_type, timestamp_ns);
                }
                AccessAnalysisTask::UpdateFrequency { key, new_frequency } => {
                    self.frequency_estimator
                        .update_frequency(&key, new_frequency);
                }
                AccessAnalysisTask::ClassifyPattern {
                    access_sequence,
                    window_size,
                } => {
                    self.pattern_classifier
                        .classify_sequence(&access_sequence, window_size);
                }
            }
        }
    }

    /// Analyze specific access pattern
    fn analyze_access_pattern(
        &self,
        key: &WarmCacheKey<K>,
        _access_type: AccessType,
        _timestamp_ns: u64,
    ) {
        // Get recent access pattern for this key
        let pattern = self.get_access_pattern(key, 60_000_000_000); // 1 minute window

        // Extract timing sequence
        let timestamps: Vec<u64> = pattern.iter().map(|r| r.timestamp_ns).collect();

        // Classify temporal pattern
        if timestamps.len() >= 3 {
            let task = AccessAnalysisTask::ClassifyPattern {
                access_sequence: timestamps,
                window_size: 10,
            };
            let _ = self.analysis_tx.try_send(task);
        }

        // Update frequency estimate
        let frequency = self.calculate_access_frequency(&pattern);
        let task = AccessAnalysisTask::UpdateFrequency {
            key: key.clone(),
            new_frequency: frequency,
        };
        let _ = self.analysis_tx.try_send(task);
    }

    /// Calculate access frequency from pattern
    fn calculate_access_frequency(&self, pattern: &[AccessRecord]) -> f64 {
        if pattern.len() < 2 {
            return 1.0;
        }

        // Calculate average time between accesses
        let mut intervals = Vec::new();
        for window in pattern.windows(2) {
            let interval = window[1].timestamp_ns - window[0].timestamp_ns;
            intervals.push(interval);
        }

        if intervals.is_empty() {
            return 1.0;
        }

        let avg_interval_ns = intervals.iter().sum::<u64>() / intervals.len() as u64;

        // Convert to frequency (accesses per second)
        if avg_interval_ns > 0 {
            1_000_000_000.0 / avg_interval_ns as f64
        } else {
            1000.0 // High frequency fallback
        }
    }

    /// Get pattern classifier reference
    pub fn get_pattern_classifier(&self) -> &TemporalPatternClassifier {
        &self.pattern_classifier
    }

    /// Get frequency estimator reference
    pub fn get_frequency_estimator(&self) -> &FrequencyEstimator<K> {
        &self.frequency_estimator
    }
}
