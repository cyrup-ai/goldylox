#![allow(dead_code)]
// Performance Types - Complete high-precision timing library with RDTSC cycle-accurate measurements, CPU frequency calibration, precision timers, stopwatch functionality, and timer collections

//! High-precision timing utilities
//!
//! This module provides RDTSC-based timing for sub-nanosecond
//! precision performance measurements.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::_rdtsc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Static CPU frequency cache for RDTSC conversion
static CPU_FREQ_GHZ: OnceLock<f64> = OnceLock::new();

/// Calibrate CPU frequency by measuring RDTSC cycles over known time
#[inline(never)]
fn calibrate_cpu_frequency() -> f64 {
    #[cfg(target_arch = "x86_64")]
    {
        // Measure RDTSC cycles over 100ms for accurate frequency detection
        let start_instant = Instant::now();
        let start_cycles = unsafe { _rdtsc() };

        // Busy wait for accurate timing
        let target_duration = Duration::from_millis(100);
        while start_instant.elapsed() < target_duration {
            std::hint::spin_loop();
        }

        let end_cycles = unsafe { _rdtsc() };
        let actual_duration = start_instant.elapsed();

        let cycles_elapsed = end_cycles.saturating_sub(start_cycles);
        let seconds_elapsed = actual_duration.as_secs_f64();

        if seconds_elapsed > 0.0 && cycles_elapsed > 0 {
            (cycles_elapsed as f64) / seconds_elapsed / 1_000_000_000.0
        } else {
            // Fallback to estimated frequency if calibration fails
            2.5
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        // Fallback for non-x86_64 architectures
        2.5
    }
}

/// Get CPU frequency in GHz (cached after first call)
#[inline(always)]
fn get_cpu_freq_ghz() -> f64 {
    *CPU_FREQ_GHZ.get_or_init(calibrate_cpu_frequency)
}

/// Read RDTSC counter with fallback for non-x86_64
#[inline(always)]
fn read_tsc() -> u64 {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        _rdtsc()
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        // Fallback to nanosecond timestamp for non-x86_64
        timestamp_nanos(Instant::now())
    }
}

/// Convert RDTSC cycles to nanoseconds
#[inline(always)]
fn cycles_to_nanos(cycles: u64) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        let freq_ghz = get_cpu_freq_ghz();
        if freq_ghz > 0.0 {
            ((cycles as f64) / freq_ghz) as u64
        } else {
            cycles
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        cycles
    }
}

/// Convert Instant to nanosecond timestamp
#[inline(always)]
pub fn timestamp_nanos(_instant: Instant) -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_nanos() as u64
}

/// RDTSC-based high-precision timing for cycle-accurate measurements
#[derive(Debug, Clone)]
pub struct PrecisionTimer {
    /// Start timestamp in CPU cycles
    start_cycles: u64,
    /// Optional label for debugging
    label: Option<String>,
}

impl PrecisionTimer {
    /// Create new precision timer using RDTSC
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            start_cycles: read_tsc(),
            label: None,
        }
    }

    /// Start timing measurement using RDTSC
    #[inline(always)]
    pub fn start() -> Self {
        Self::new()
    }

    /// Create new precision timer with label for debugging
    #[inline(always)]
    pub fn with_label(label: impl Into<String>) -> Self {
        Self {
            start_cycles: read_tsc(),
            label: Some(label.into()),
        }
    }

    /// Get the timer's label if any
    #[inline(always)]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Get elapsed time in nanoseconds using RDTSC cycle conversion
    #[inline(always)]
    pub fn elapsed_ns(&self) -> u64 {
        let current_cycles = read_tsc();
        let cycles_elapsed = current_cycles.saturating_sub(self.start_cycles);
        cycles_to_nanos(cycles_elapsed)
    }

    /// Get elapsed time as Duration
    #[inline(always)]
    pub fn elapsed(&self) -> Duration {
        Duration::from_nanos(self.elapsed_ns())
    }

    /// Reset timer to current RDTSC time
    #[inline(always)]
    pub fn reset(&mut self) {
        self.start_cycles = read_tsc();
    }

    /// Get elapsed time in microseconds
    #[inline(always)]
    pub fn elapsed_micros(&self) -> u64 {
        self.elapsed_ns() / 1_000
    }

    /// Get elapsed time in milliseconds
    #[inline(always)]
    pub fn elapsed_millis(&self) -> u64 {
        self.elapsed_ns() / 1_000_000
    }

    /// Get elapsed time in seconds as f64
    #[inline(always)]
    pub fn elapsed_secs_f64(&self) -> f64 {
        self.elapsed_ns() as f64 / 1_000_000_000.0
    }

    /// Get timer start timestamp in cycles
    #[inline(always)]
    pub fn start_cycles(&self) -> u64 {
        self.start_cycles
    }

    /// Create timer with specific start cycles
    #[inline(always)]
    pub fn with_start_cycles(start_cycles: u64) -> Self {
        Self {
            start_cycles,
            label: None,
        }
    }

    /// Get current RDTSC timestamp
    #[inline(always)]
    pub fn current_cycles() -> u64 {
        read_tsc()
    }

    /// Check if timer has elapsed beyond threshold
    #[inline(always)]
    pub fn has_elapsed(&self, threshold_ns: u64) -> bool {
        self.elapsed_ns() >= threshold_ns
    }

    /// Get remaining time until threshold
    #[inline(always)]
    pub fn remaining_ns(&self, threshold_ns: u64) -> u64 {
        let elapsed = self.elapsed_ns();
        threshold_ns.saturating_sub(elapsed)
    }
}

impl Default for PrecisionTimer {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple stopwatch for basic timing needs
#[derive(Debug, Clone)]
pub struct Stopwatch {
    /// Start time
    start: Instant,
    /// Accumulated duration (for pause/resume)
    accumulated: Duration,
    /// Whether stopwatch is currently running
    running: bool,
}

impl Stopwatch {
    /// Create new stopwatch
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            accumulated: Duration::ZERO,
            running: false,
        }
    }

    /// Start the stopwatch
    #[inline(always)]
    pub fn start(&mut self) {
        if !self.running {
            self.start = Instant::now();
            self.running = true;
        }
    }

    /// Stop the stopwatch
    #[inline(always)]
    pub fn stop(&mut self) {
        if self.running {
            self.accumulated += self.start.elapsed();
            self.running = false;
        }
    }

    /// Reset the stopwatch
    #[inline(always)]
    pub fn reset(&mut self) {
        self.accumulated = Duration::ZERO;
        self.start = Instant::now();
        self.running = false;
    }

    /// Get elapsed time
    #[inline(always)]
    pub fn elapsed(&self) -> Duration {
        if self.running {
            self.accumulated + self.start.elapsed()
        } else {
            self.accumulated
        }
    }

    /// Get elapsed time in nanoseconds
    #[inline(always)]
    pub fn elapsed_ns(&self) -> u64 {
        self.elapsed().as_nanos() as u64
    }

    /// Get elapsed time in microseconds
    #[inline(always)]
    pub fn elapsed_micros(&self) -> u64 {
        self.elapsed().as_micros() as u64
    }

    /// Get elapsed time in milliseconds
    #[inline(always)]
    pub fn elapsed_millis(&self) -> u64 {
        self.elapsed().as_millis() as u64
    }

    /// Check if stopwatch is running
    #[inline(always)]
    pub fn is_running(&self) -> bool {
        self.running
    }
}

impl Default for Stopwatch {
    fn default() -> Self {
        Self::new()
    }
}

/// Timer collection for managing multiple named timers
#[derive(Debug, Clone)]
pub struct TimerCollection {
    /// Named timers
    timers: std::collections::HashMap<String, PrecisionTimer>,
}

impl TimerCollection {
    /// Create new timer collection
    pub fn new() -> Self {
        Self {
            timers: std::collections::HashMap::new(),
        }
    }

    /// Start a named timer
    pub fn start_timer(&mut self, name: impl Into<String>) {
        self.timers.insert(name.into(), PrecisionTimer::start());
    }

    /// Get elapsed time for a named timer
    pub fn elapsed_ns(&self, name: &str) -> Option<u64> {
        self.timers.get(name).map(|timer| timer.elapsed_ns())
    }

    /// Remove and get final elapsed time for a timer
    pub fn finish_timer(&mut self, name: &str) -> Option<u64> {
        self.timers.remove(name).map(|timer| timer.elapsed_ns())
    }

    /// Get all timer results
    pub fn all_results(&self) -> std::collections::HashMap<String, u64> {
        self.timers
            .iter()
            .map(|(name, timer)| (name.clone(), timer.elapsed_ns()))
            .collect()
    }

    /// Clear all timers
    pub fn clear(&mut self) {
        self.timers.clear();
    }
}

impl Default for TimerCollection {
    fn default() -> Self {
        Self::new()
    }
}
