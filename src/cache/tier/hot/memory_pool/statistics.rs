//! Memory pool statistics and monitoring
//!
//! This module provides statistics tracking and health monitoring
//! for the SIMD-aligned memory pool.

/// Memory pool statistics
#[derive(Debug, Clone, Copy)]
pub struct MemoryPoolStats {
    pub total_slots: usize,
    pub occupied_slots: usize,
    pub available_slots: usize,
    pub total_memory_usage: usize,
    pub average_slot_size: usize,
    pub fragmentation_ratio: f64,
}

impl MemoryPoolStats {
    /// Get memory utilization ratio (0.0 to 1.0)
    pub fn utilization_ratio(&self) -> f64 {
        if self.total_slots > 0 {
            self.occupied_slots as f64 / self.total_slots as f64
        } else {
            0.0
        }
    }

    /// Merge statistics from another MemoryPoolStats
    pub fn merge(&mut self, other: MemoryPoolStats) {
        self.total_slots += other.total_slots;
        self.occupied_slots += other.occupied_slots;
        self.available_slots += other.available_slots;
        self.total_memory_usage += other.total_memory_usage;

        // Average the slot sizes
        self.average_slot_size = (self.average_slot_size + other.average_slot_size) / 2;

        // Average the fragmentation ratios
        self.fragmentation_ratio = (self.fragmentation_ratio + other.fragmentation_ratio) / 2.0;
    }

    /// Check if pool is nearly full
    pub fn is_nearly_full(&self, threshold: f64) -> bool {
        self.utilization_ratio() > threshold
    }

    /// Check if compaction is recommended
    pub fn should_compact(&self) -> bool {
        self.fragmentation_ratio > 0.3 && self.occupied_slots > 10
    }
}

impl Default for MemoryPoolStats {
    fn default() -> Self {
        Self {
            total_slots: 0,
            occupied_slots: 0,
            available_slots: 0,
            total_memory_usage: 0,
            average_slot_size: 0,
            fragmentation_ratio: 0.0,
        }
    }
}
