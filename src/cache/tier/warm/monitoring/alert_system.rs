//! Alert system for memory monitoring and notifications
//!
//! This module handles alert generation, cooldown management, and notification
//! delivery for memory pressure monitoring.

#![allow(dead_code)] // Warm tier monitoring - Complete alert system library for memory pressure notifications

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender, unbounded};
use crossbeam_utils::CachePadded;

use super::types::MemoryAlert;
use crate::cache::tier::warm::atomic_float::AtomicF64;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::types::timestamp_nanos;

/// Alert statistics
#[derive(Debug)]
pub struct AlertStats {
    /// Alerts sent by type
    pub alerts_by_type: [CachePadded<AtomicU64>; 6],
    /// False positive alerts
    pub false_positives: CachePadded<AtomicU64>,
    /// Alert response times
    pub avg_response_time_ms: AtomicF64,
    /// Alert effectiveness score
    pub effectiveness_score: AtomicF64,
}

impl Default for AlertStats {
    fn default() -> Self {
        Self::new()
    }
}

impl AlertStats {
    pub fn new() -> Self {
        Self {
            alerts_by_type: [
                CachePadded::new(AtomicU64::new(0)),
                CachePadded::new(AtomicU64::new(0)),
                CachePadded::new(AtomicU64::new(0)),
                CachePadded::new(AtomicU64::new(0)),
                CachePadded::new(AtomicU64::new(0)),
                CachePadded::new(AtomicU64::new(0)),
            ],
            false_positives: CachePadded::new(AtomicU64::new(0)),
            avg_response_time_ms: AtomicF64::new(0.0),
            effectiveness_score: AtomicF64::new(0.5),
        }
    }

    /// Record alert sent by type
    pub fn record_alert(&self, alert_type: usize) {
        if alert_type < self.alerts_by_type.len() {
            self.alerts_by_type[alert_type].fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get alert counts by type
    pub fn get_alert_counts(&self) -> [u64; 6] {
        [
            self.alerts_by_type[0].load(Ordering::Relaxed),
            self.alerts_by_type[1].load(Ordering::Relaxed),
            self.alerts_by_type[2].load(Ordering::Relaxed),
            self.alerts_by_type[3].load(Ordering::Relaxed),
            self.alerts_by_type[4].load(Ordering::Relaxed),
            self.alerts_by_type[5].load(Ordering::Relaxed),
        ]
    }
}

/// Memory alerting system
#[derive(Debug)]
pub struct MemoryAlertSystem {
    /// Alert channel for sending notifications
    pub alert_tx: Sender<MemoryAlert>,
    pub alert_rx: Receiver<MemoryAlert>,
    /// Last alert timestamps to prevent spam
    pub last_alert_timestamps: [CachePadded<AtomicU64>; 4], // One per pressure level
    /// Alert cooldown period
    pub alert_cooldown_ms: u64,
    /// Alert statistics
    pub alert_stats: AlertStats,
}

impl MemoryAlertSystem {
    pub fn new(alert_cooldown_ms: u64) -> Self {
        let (alert_tx, alert_rx) = unbounded();

        Self {
            alert_tx,
            alert_rx,
            last_alert_timestamps: [
                CachePadded::new(AtomicU64::new(0)),
                CachePadded::new(AtomicU64::new(0)),
                CachePadded::new(AtomicU64::new(0)),
                CachePadded::new(AtomicU64::new(0)),
            ],
            alert_cooldown_ms,
            alert_stats: AlertStats::new(),
        }
    }

    /// Send alert if cooldown period has elapsed
    pub fn try_send_alert(&self, alert: MemoryAlert) -> Result<bool, CacheOperationError> {
        let alert_level = match &alert {
            MemoryAlert::LowPressure { .. } => 0,
            MemoryAlert::MediumPressure { .. } => 1,
            MemoryAlert::HighPressure { .. } => 2,
            MemoryAlert::CriticalPressure { .. } => 3,
            MemoryAlert::MemoryLeak { .. } => return self.send_alert_unchecked(alert, 4),
            MemoryAlert::OomRisk { .. } => return self.send_alert_unchecked(alert, 5),
        };

        let current_time = timestamp_nanos(Instant::now());
        if self.should_send_alert(alert_level, current_time) {
            self.send_alert_unchecked(alert, alert_level)
        } else {
            Ok(false)
        }
    }

    /// Send alert without cooldown check
    fn send_alert_unchecked(
        &self,
        alert: MemoryAlert,
        alert_type: usize,
    ) -> Result<bool, CacheOperationError> {
        self.alert_stats.record_alert(alert_type);

        match self.alert_tx.try_send(alert) {
            Ok(()) => Ok(true),
            Err(_) => {
                // Alert channel full - this is itself an alert condition
                Err(CacheOperationError::resource_exhausted(
                    "Alert channel full",
                ))
            }
        }
    }

    /// Check if an alert should be sent based on cooldown period
    fn should_send_alert(&self, alert_level: usize, current_time: u64) -> bool {
        if alert_level >= self.last_alert_timestamps.len() {
            return false;
        }

        let last_alert = self.last_alert_timestamps[alert_level].load(Ordering::Relaxed);
        let time_since_last = current_time.saturating_sub(last_alert);
        let cooldown_ns = self.alert_cooldown_ms * 1_000_000;

        if time_since_last >= cooldown_ns {
            self.last_alert_timestamps[alert_level].store(current_time, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Receive memory alerts (non-blocking)
    pub fn try_receive_alert(&self) -> Option<MemoryAlert> {
        self.alert_rx.try_recv().ok()
    }

    /// Get alert system statistics
    pub fn get_stats(&self) -> &AlertStats {
        &self.alert_stats
    }
}
