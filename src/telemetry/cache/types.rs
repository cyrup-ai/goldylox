#![allow(dead_code)] // Telemetry System - Complete cache measurement types with metrics tracking and timing utilities

//! Cache measurement types and error definitions

/// Performance measurement types
#[derive(Debug, Clone, Copy, Default)]
pub struct CacheMetrics {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub memory_used: usize,
    pub entries: usize,
}

/// Timing measurement utilities
#[derive(Debug, Clone, Copy)]
pub struct TimingMeasurement {
    pub start_time: std::time::Instant,
    pub duration: Option<std::time::Duration>,
}

impl TimingMeasurement {
    pub fn start() -> Self {
        Self {
            start_time: std::time::Instant::now(),
            duration: None,
        }
    }

    pub fn finish(&mut self) {
        self.duration = Some(self.start_time.elapsed());
    }
}

impl Default for TimingMeasurement {
    fn default() -> Self {
        Self::start()
    }
}

/// Get current timestamp in nanoseconds
pub fn timestamp_nanos() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}
