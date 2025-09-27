#![allow(dead_code)]
// Warm tier atomic float - Complete atomic floating-point library with bit manipulation, lock-free operations, and array utilities

//! Lock-free atomic floating point operations using bit manipulation
//!
//! This module provides atomic f64 operations without external dependencies
//! by using AtomicU64 to store the bit representation of floating point values.

use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_utils::CachePadded;

/// Atomic f64 implementation using bit manipulation for zero-allocation operations
#[derive(Debug)]
pub struct AtomicF64 {
    /// Store f64 as u64 bits for atomic operations
    bits: CachePadded<AtomicU64>,
}

impl AtomicF64 {
    /// Create new atomic f64 with initial value
    #[inline]
    pub fn new(value: f64) -> Self {
        Self {
            bits: CachePadded::new(AtomicU64::new(value.to_bits())),
        }
    }

    /// Load the current value
    #[inline]
    pub fn load(&self, ordering: Ordering) -> f64 {
        f64::from_bits(self.bits.load(ordering))
    }

    /// Store a new value
    #[inline]
    pub fn store(&self, value: f64, ordering: Ordering) {
        self.bits.store(value.to_bits(), ordering);
    }

    /// Swap with new value and return old value
    #[inline]
    pub fn swap(&self, value: f64, ordering: Ordering) -> f64 {
        let old_bits = self.bits.swap(value.to_bits(), ordering);
        f64::from_bits(old_bits)
    }

    /// Compare and exchange weak
    #[inline]
    pub fn compare_exchange_weak(
        &self,
        current: f64,
        new: f64,
        success: Ordering,
        failure: Ordering,
    ) -> Result<f64, f64> {
        match self
            .bits
            .compare_exchange_weak(current.to_bits(), new.to_bits(), success, failure)
        {
            Ok(old_bits) => Ok(f64::from_bits(old_bits)),
            Err(old_bits) => Err(f64::from_bits(old_bits)),
        }
    }

    /// Compare and exchange strong
    #[inline]
    pub fn compare_exchange(
        &self,
        current: f64,
        new: f64,
        success: Ordering,
        failure: Ordering,
    ) -> Result<f64, f64> {
        match self
            .bits
            .compare_exchange(current.to_bits(), new.to_bits(), success, failure)
        {
            Ok(old_bits) => Ok(f64::from_bits(old_bits)),
            Err(old_bits) => Err(f64::from_bits(old_bits)),
        }
    }

    /// Fetch and update with closure (lock-free retry loop)
    #[inline]
    pub fn fetch_update<F>(
        &self,
        set_order: Ordering,
        fetch_order: Ordering,
        mut f: F,
    ) -> Result<f64, f64>
    where
        F: FnMut(f64) -> Option<f64>,
    {
        let mut current_bits = self.bits.load(fetch_order);
        loop {
            let current = f64::from_bits(current_bits);
            let new = match f(current) {
                Some(new) => new,
                None => return Err(current),
            };

            match self.bits.compare_exchange_weak(
                current_bits,
                new.to_bits(),
                set_order,
                fetch_order,
            ) {
                Ok(_) => return Ok(current),
                Err(bits) => current_bits = bits,
            }
        }
    }

    /// Atomic add operation (using fetch_update for lock-free implementation)
    #[inline]
    pub fn fetch_add(&self, value: f64, ordering: Ordering) -> f64 {
        self.fetch_update(ordering, ordering, |current| Some(current + value))
            .unwrap_or_else(|x| x)
    }

    /// Atomic subtract operation
    #[inline]
    pub fn fetch_sub(&self, value: f64, ordering: Ordering) -> f64 {
        self.fetch_add(-value, ordering)
    }

    /// Atomic multiply operation
    #[inline]
    pub fn fetch_mul(&self, value: f64, ordering: Ordering) -> f64 {
        self.fetch_update(ordering, ordering, |current| Some(current * value))
            .unwrap_or_else(|x| x)
    }

    /// Atomic divide operation
    #[inline]
    pub fn fetch_div(&self, value: f64, ordering: Ordering) -> f64 {
        self.fetch_update(ordering, ordering, |current| {
            if value != 0.0 {
                Some(current / value)
            } else {
                None // Division by zero
            }
        })
        .unwrap_or_else(|x| x)
    }

    /// Atomic maximum operation
    #[inline]
    pub fn fetch_max(&self, value: f64, ordering: Ordering) -> f64 {
        self.fetch_update(ordering, ordering, |current| Some(current.max(value)))
            .unwrap_or_else(|x| x)
    }

    /// Atomic minimum operation
    #[inline]
    pub fn fetch_min(&self, value: f64, ordering: Ordering) -> f64 {
        self.fetch_update(ordering, ordering, |current| Some(current.min(value)))
            .unwrap_or_else(|x| x)
    }

    /// Update value using exponential moving average
    #[inline]
    pub fn update_ema(&self, new_value: f64, alpha: f64, ordering: Ordering) -> f64 {
        self.fetch_update(ordering, ordering, |current| {
            Some(current * (1.0 - alpha) + new_value * alpha)
        })
        .unwrap_or_else(|x| x)
    }

    /// Atomic increment by 1.0
    #[inline]
    pub fn increment(&self, ordering: Ordering) -> f64 {
        self.fetch_add(1.0, ordering)
    }

    /// Atomic decrement by 1.0
    #[inline]
    pub fn decrement(&self, ordering: Ordering) -> f64 {
        self.fetch_sub(1.0, ordering)
    }

    /// Check if value is approximately equal to target (within epsilon)
    #[inline]
    pub fn is_approximately(&self, target: f64, epsilon: f64, ordering: Ordering) -> bool {
        let current = self.load(ordering);
        (current - target).abs() <= epsilon
    }

    /// Get raw bits (for debugging/serialization)
    #[inline]
    pub fn to_bits(&self, ordering: Ordering) -> u64 {
        self.bits.load(ordering)
    }

    /// Load from raw bits (for debugging/deserialization)
    #[inline]
    pub fn store_bits(&self, bits: u64, ordering: Ordering) {
        self.bits.store(bits, ordering);
    }
}

impl Default for AtomicF64 {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl Clone for AtomicF64 {
    fn clone(&self) -> Self {
        Self::new(self.load(Ordering::Relaxed))
    }
}

/// Utility functions for atomic floating point arrays
pub mod array_utils {
    use super::*;

    /// Create array of atomic f64 values
    pub fn create_atomic_f64_array<const N: usize>(initial_value: f64) -> [AtomicF64; N] {
        // Use MaybeUninit for zero-cost initialization
        use std::mem::MaybeUninit;

        let mut array: [MaybeUninit<AtomicF64>; N] = unsafe { MaybeUninit::uninit().assume_init() };

        for element in &mut array {
            element.write(AtomicF64::new(initial_value));
        }

        unsafe {
            let ptr = array.as_ptr() as *const [AtomicF64; N];
            ptr.read()
        }
    }

    /// Update all elements in atomic f64 array
    pub fn update_array<const N: usize>(
        array: &[AtomicF64; N],
        values: &[f64; N],
        ordering: Ordering,
    ) {
        for (atomic, &value) in array.iter().zip(values.iter()) {
            atomic.store(value, ordering);
        }
    }

    /// Load all elements from atomic f64 array
    pub fn load_array<const N: usize>(array: &[AtomicF64; N], ordering: Ordering) -> [f64; N] {
        let mut result = [0.0; N];
        for (i, atomic) in array.iter().enumerate() {
            result[i] = atomic.load(ordering);
        }
        result
    }

    /// Calculate sum of all elements in atomic f64 array
    pub fn sum_array<const N: usize>(array: &[AtomicF64; N], ordering: Ordering) -> f64 {
        array.iter().map(|atomic| atomic.load(ordering)).sum()
    }

    /// Find maximum value in atomic f64 array
    pub fn max_array<const N: usize>(array: &[AtomicF64; N], ordering: Ordering) -> f64 {
        array
            .iter()
            .map(|atomic| atomic.load(ordering))
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Find minimum value in atomic f64 array
    pub fn min_array<const N: usize>(array: &[AtomicF64; N], ordering: Ordering) -> f64 {
        array
            .iter()
            .map(|atomic| atomic.load(ordering))
            .fold(f64::INFINITY, f64::min)
    }

    /// Calculate average of all elements in atomic f64 array
    pub fn average_array<const N: usize>(array: &[AtomicF64; N], ordering: Ordering) -> f64 {
        if N == 0 {
            0.0
        } else {
            sum_array(array, ordering) / N as f64
        }
    }
}
