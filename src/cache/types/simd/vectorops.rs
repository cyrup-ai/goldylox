//! SIMD-optimized vector operations for cache management
//!
//! This module provides vectorized mathematical operations for
//! high-performance data processing and analysis.

/// SIMD-optimized vector operations for cache management
#[derive(Debug, Clone)]
pub struct SimdVectorOps;

impl SimdVectorOps {
    /// Find minimum value in array using SIMD-style operations
    #[inline(always)]
    pub fn find_min_f64(values: &[f64]) -> Option<f64> {
        if values.is_empty() {
            return None;
        }

        // Process in chunks of 4 for simulated SIMD
        let chunks = values.chunks_exact(4);
        let remainder = chunks.remainder();

        let mut global_min = f64::INFINITY;

        // Process main chunks
        for chunk in chunks {
            let local_min = chunk.iter().fold(f64::INFINITY, |acc, &x| acc.min(x));
            global_min = global_min.min(local_min);
        }

        // Process remainder
        for &value in remainder {
            global_min = global_min.min(value);
        }

        if global_min == f64::INFINITY {
            None
        } else {
            Some(global_min)
        }
    }

    /// Find maximum value in array using SIMD-style operations
    #[inline(always)]
    pub fn find_max_f64(values: &[f64]) -> Option<f64> {
        if values.is_empty() {
            return None;
        }

        // Process in chunks of 4 for simulated SIMD
        let chunks = values.chunks_exact(4);
        let remainder = chunks.remainder();

        let mut global_max = f64::NEG_INFINITY;

        // Process main chunks
        for chunk in chunks {
            let local_max = chunk.iter().fold(f64::NEG_INFINITY, |acc, &x| acc.max(x));
            global_max = global_max.max(local_max);
        }

        // Process remainder
        for &value in remainder {
            global_max = global_max.max(value);
        }

        if global_max == f64::NEG_INFINITY {
            None
        } else {
            Some(global_max)
        }
    }

    /// Sum array values using SIMD-style operations
    #[inline(always)]
    pub fn sum_f64(values: &[f64]) -> f64 {
        // Process in chunks of 4 for simulated SIMD
        let chunks = values.chunks_exact(4);
        let remainder = chunks.remainder();

        let mut sum = 0.0;

        // Process main chunks with parallel accumulation
        for chunk in chunks {
            let chunk_sum: f64 = chunk.iter().sum();
            sum += chunk_sum;
        }

        // Process remainder
        sum += remainder.iter().sum::<f64>();
        sum
    }

    /// Compute average with SIMD optimization
    #[inline(always)]
    pub fn average_f64(values: &[f64]) -> Option<f64> {
        if values.is_empty() {
            None
        } else {
            Some(Self::sum_f64(values) / values.len() as f64)
        }
    }

    /// Vectorized comparison for threshold filtering
    pub fn filter_above_threshold(values: &[f64], threshold: f64) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &value)| if value > threshold { Some(i) } else { None })
            .collect()
    }

    /// Vectorized normalization to 0-1 range
    pub fn normalize_range(values: &mut [f64]) {
        if let (Some(min), Some(max)) = (Self::find_min_f64(values), Self::find_max_f64(values)) {
            let range = max - min;
            if range > 0.0 {
                for value in values.iter_mut() {
                    *value = (*value - min) / range;
                }
            }
        }
    }

    /// Vectorized scaling operation
    pub fn scale_values(values: &mut [f64], scale_factor: f64) {
        for chunk in values.chunks_exact_mut(4) {
            chunk[0] *= scale_factor;
            chunk[1] *= scale_factor;
            chunk[2] *= scale_factor;
            chunk[3] *= scale_factor;
        }

        // Handle remainder
        let remainder_start = (values.len() / 4) * 4;
        for value in &mut values[remainder_start..] {
            *value *= scale_factor;
        }
    }

    /// Vectorized addition of two arrays
    pub fn add_arrays(a: &[f64], b: &[f64], result: &mut [f64]) {
        assert_eq!(a.len(), b.len());
        assert_eq!(a.len(), result.len());

        let chunks_a = a.chunks_exact(4);
        let chunks_b = b.chunks_exact(4);
        let chunks_result = result.chunks_exact_mut(4);

        for ((chunk_a, chunk_b), chunk_result) in chunks_a.zip(chunks_b).zip(chunks_result) {
            chunk_result[0] = chunk_a[0] + chunk_b[0];
            chunk_result[1] = chunk_a[1] + chunk_b[1];
            chunk_result[2] = chunk_a[2] + chunk_b[2];
            chunk_result[3] = chunk_a[3] + chunk_b[3];
        }

        // Handle remainder
        let remainder_start = (a.len() / 4) * 4;
        for i in remainder_start..a.len() {
            result[i] = a[i] + b[i];
        }
    }

    /// Vectorized dot product
    pub fn dot_product(a: &[f64], b: &[f64]) -> f64 {
        assert_eq!(a.len(), b.len());

        let chunks_a = a.chunks_exact(4);
        let chunks_b = b.chunks_exact(4);
        let remainder_start = (a.len() / 4) * 4;

        let mut sum = 0.0;

        // Process chunks of 4
        for (chunk_a, chunk_b) in chunks_a.zip(chunks_b) {
            sum += chunk_a[0] * chunk_b[0]
                + chunk_a[1] * chunk_b[1]
                + chunk_a[2] * chunk_b[2]
                + chunk_a[3] * chunk_b[3];
        }

        // Handle remainder
        for i in remainder_start..a.len() {
            sum += a[i] * b[i];
        }

        sum
    }

    /// Calculate element-wise differences between arrays
    pub fn element_diff(a: &[f64], b: &[f64]) -> Vec<f64> {
        assert_eq!(a.len(), b.len());
        let mut result = vec![0.0; a.len()];

        for (i, (va, vb)) in a.iter().zip(b.iter()).enumerate() {
            result[i] = va - vb;
        }

        result
    }

    /// Calculate squared differences for variance computations
    pub fn squared_differences(values: &[f64], mean: f64) -> Vec<f64> {
        values
            .iter()
            .map(|&x| {
                let diff = x - mean;
                diff * diff
            })
            .collect()
    }

    /// Find indices of values within a range
    pub fn find_in_range(values: &[f64], min_val: f64, max_val: f64) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &value)| {
                if value >= min_val && value <= max_val {
                    Some(i)
                } else {
                    None
                }
            })
            .collect()
    }
}
