#![allow(dead_code)]
// SIMD Types - Complete vectorized mathematical operations library with AVX2/SSE2 optimization, runtime CPU detection, statistical analysis, filtering, and normalization

//! SIMD-optimized vector operations for cache management
//!
//! This module provides hardware SIMD acceleration for mathematical operations
//! using SSE2/AVX2 intrinsics with scalar fallbacks.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// SIMD-optimized vector operations
#[derive(Debug, Clone)]
pub struct SimdVectorOps;

impl SimdVectorOps {
    /// Find minimum value using hardware SIMD operations (Fix 3: Safe SIMD extraction)
    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    pub fn find_min_f64(values: &[f64]) -> Option<f64> {
        if values.is_empty() {
            return None;
        }

        // Runtime CPU feature detection
        if !is_x86_feature_detected!("avx2") {
            return Self::find_min_f64_scalar(values);
        }

        unsafe {
            let chunks = values.chunks_exact(4);
            let remainder = chunks.remainder();

            let mut min_vec = _mm256_set1_pd(f64::INFINITY);

            // Process 4 f64 values at a time with AVX2
            for chunk in chunks {
                let data = _mm256_loadu_pd(chunk.as_ptr());
                min_vec = _mm256_min_pd(min_vec, data);
            }

            // Safe extraction using proper SIMD store operation
            let mut mins = [0.0f64; 4];
            _mm256_storeu_pd(mins.as_mut_ptr(), min_vec);
            let mut global_min = mins[0].min(mins[1]).min(mins[2]).min(mins[3]);

            // Handle remainder
            for &value in remainder {
                global_min = global_min.min(value);
            }

            if global_min == f64::INFINITY {
                None
            } else {
                Some(global_min)
            }
        }
    }

    /// Scalar fallback for find_min_f64
    #[inline(always)]
    fn find_min_f64_scalar(values: &[f64]) -> Option<f64> {
        values.iter().fold(None, |acc, &x| match acc {
            None => Some(x),
            Some(min) => Some(min.min(x)),
        })
    }

    /// Find minimum value using scalar fallback
    #[cfg(not(target_arch = "x86_64"))]
    #[inline(always)]
    pub fn find_min_f64(values: &[f64]) -> Option<f64> {
        values.iter().fold(None, |acc, &x| match acc {
            None => Some(x),
            Some(min) => Some(min.min(x)),
        })
    }

    /// Find maximum value using hardware SIMD operations (Fix 3: Safe SIMD extraction)
    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    pub fn find_max_f64(values: &[f64]) -> Option<f64> {
        if values.is_empty() {
            return None;
        }

        // Runtime CPU feature detection
        if !is_x86_feature_detected!("avx2") {
            return Self::find_max_f64_scalar(values);
        }

        unsafe {
            let chunks = values.chunks_exact(4);
            let remainder = chunks.remainder();

            let mut max_vec = _mm256_set1_pd(f64::NEG_INFINITY);

            // Process 4 f64 values at a time with AVX2
            for chunk in chunks {
                let data = _mm256_loadu_pd(chunk.as_ptr());
                max_vec = _mm256_max_pd(max_vec, data);
            }

            // Safe extraction using proper SIMD store operation
            let mut maxs = [0.0f64; 4];
            _mm256_storeu_pd(maxs.as_mut_ptr(), max_vec);
            let mut global_max = maxs[0].max(maxs[1]).max(maxs[2]).max(maxs[3]);

            // Handle remainder
            for &value in remainder {
                global_max = global_max.max(value);
            }

            if global_max == f64::NEG_INFINITY {
                None
            } else {
                Some(global_max)
            }
        }
    }

    /// Scalar fallback for find_max_f64
    #[inline(always)]
    fn find_max_f64_scalar(values: &[f64]) -> Option<f64> {
        values.iter().fold(None, |acc, &x| match acc {
            None => Some(x),
            Some(max) => Some(max.max(x)),
        })
    }

    /// Find maximum value using scalar fallback
    #[cfg(not(target_arch = "x86_64"))]
    #[inline(always)]
    pub fn find_max_f64(values: &[f64]) -> Option<f64> {
        values.iter().fold(None, |acc, &x| match acc {
            None => Some(x),
            Some(max) => Some(max.max(x)),
        })
    }

    /// Sum array values using hardware SIMD operations (Fix 3: Safe SIMD extraction)
    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    pub fn sum_f64(values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }

        // Runtime CPU feature detection
        if !is_x86_feature_detected!("avx2") {
            return Self::sum_f64_scalar(values);
        }

        unsafe {
            let chunks = values.chunks_exact(4);
            let remainder = chunks.remainder();

            let mut sum_vec = _mm256_setzero_pd();

            // Process 4 f64 values at a time with AVX2
            for chunk in chunks {
                let data = _mm256_loadu_pd(chunk.as_ptr());
                sum_vec = _mm256_add_pd(sum_vec, data);
            }

            // Safe extraction using proper SIMD store operation
            let mut sums = [0.0f64; 4];
            _mm256_storeu_pd(sums.as_mut_ptr(), sum_vec);
            let mut total = sums[0] + sums[1] + sums[2] + sums[3];

            // Handle remainder
            for &value in remainder {
                total += value;
            }

            total
        }
    }

    /// Scalar fallback for sum_f64
    #[inline(always)]
    fn sum_f64_scalar(values: &[f64]) -> f64 {
        values.iter().sum()
    }

    /// Sum array values using scalar fallback
    #[cfg(not(target_arch = "x86_64"))]
    #[inline(always)]
    pub fn sum_f64(values: &[f64]) -> f64 {
        values.iter().sum()
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

    /// Vectorized comparison for threshold filtering (Fix 5: Complete SIMD coverage)
    pub fn filter_above_threshold(values: &[f64], threshold: f64) -> Vec<usize> {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                return Self::filter_above_threshold_simd(values, threshold);
            }
        }

        Self::filter_above_threshold_scalar(values, threshold)
    }

    /// SIMD-optimized threshold filtering
    #[cfg(target_arch = "x86_64")]
    fn filter_above_threshold_simd(values: &[f64], threshold: f64) -> Vec<usize> {
        let mut result = Vec::new();
        let threshold_vec = unsafe { _mm256_set1_pd(threshold) };

        let chunks = values.chunks_exact(4);
        let remainder = chunks.remainder();
        let mut base_idx = 0;

        unsafe {
            for chunk in chunks {
                let data = _mm256_loadu_pd(chunk.as_ptr());
                let cmp = _mm256_cmp_pd(data, threshold_vec, _CMP_GT_OQ);
                let mask = _mm256_movemask_pd(cmp) as u8;

                // Check each bit in the mask
                for i in 0..4 {
                    if (mask & (1 << i)) != 0 {
                        result.push(base_idx + i);
                    }
                }
                base_idx += 4;
            }
        }

        // Handle remainder with scalar
        for (i, &value) in remainder.iter().enumerate() {
            if value > threshold {
                result.push(base_idx + i);
            }
        }

        result
    }

    /// Scalar fallback for filter_above_threshold
    #[inline(always)]
    fn filter_above_threshold_scalar(values: &[f64], threshold: f64) -> Vec<usize> {
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

    /// Vectorized scaling operation using SIMD (Fix 4: Runtime CPU detection)
    #[cfg(target_arch = "x86_64")]
    pub fn scale_values(values: &mut [f64], scale_factor: f64) {
        // Runtime CPU feature detection
        if !is_x86_feature_detected!("avx2") {
            Self::scale_values_scalar(values, scale_factor);
            return;
        }

        unsafe {
            let scale_vec = _mm256_set1_pd(scale_factor);
            let chunks = values.chunks_exact_mut(4);
            let remainder_start = values.len() - chunks.remainder().len();

            for chunk in chunks {
                let data = _mm256_loadu_pd(chunk.as_ptr());
                let scaled = _mm256_mul_pd(data, scale_vec);
                _mm256_storeu_pd(chunk.as_mut_ptr(), scaled);
            }

            // Handle remainder
            for value in &mut values[remainder_start..] {
                *value *= scale_factor;
            }
        }
    }

    /// Scalar fallback for scale_values
    #[inline(always)]
    fn scale_values_scalar(values: &mut [f64], scale_factor: f64) {
        for value in values.iter_mut() {
            *value *= scale_factor;
        }
    }

    /// Vectorized scaling operation using scalar fallback
    #[cfg(not(target_arch = "x86_64"))]
    pub fn scale_values(values: &mut [f64], scale_factor: f64) {
        for value in values.iter_mut() {
            *value *= scale_factor;
        }
    }

    /// Vectorized addition of two arrays using SIMD (Fix 4: Runtime CPU detection)
    #[cfg(target_arch = "x86_64")]
    pub fn add_arrays(a: &[f64], b: &[f64], result: &mut [f64]) {
        assert_eq!(a.len(), b.len());
        assert_eq!(a.len(), result.len());

        // Runtime CPU feature detection
        if !is_x86_feature_detected!("avx2") {
            Self::add_arrays_scalar(a, b, result);
            return;
        }

        unsafe {
            let chunks_a = a.chunks_exact(4);
            let chunks_b = b.chunks_exact(4);
            let chunks_result = result.chunks_exact_mut(4);
            let remainder_start = a.len() - chunks_a.remainder().len();

            for ((chunk_a, chunk_b), chunk_result) in chunks_a.zip(chunks_b).zip(chunks_result) {
                let data_a = _mm256_loadu_pd(chunk_a.as_ptr());
                let data_b = _mm256_loadu_pd(chunk_b.as_ptr());
                let sum = _mm256_add_pd(data_a, data_b);
                _mm256_storeu_pd(chunk_result.as_mut_ptr(), sum);
            }

            // Handle remainder
            for i in remainder_start..a.len() {
                result[i] = a[i] + b[i];
            }
        }
    }

    /// Scalar fallback for add_arrays
    #[inline(always)]
    fn add_arrays_scalar(a: &[f64], b: &[f64], result: &mut [f64]) {
        assert_eq!(a.len(), b.len());
        assert_eq!(a.len(), result.len());

        for i in 0..a.len() {
            result[i] = a[i] + b[i];
        }
    }

    /// Vectorized addition of two arrays using scalar fallback
    #[cfg(not(target_arch = "x86_64"))]
    pub fn add_arrays(a: &[f64], b: &[f64], result: &mut [f64]) {
        assert_eq!(a.len(), b.len());
        assert_eq!(a.len(), result.len());

        for i in 0..a.len() {
            result[i] = a[i] + b[i];
        }
    }

    /// Vectorized dot product using SIMD (Fix 3: Safe SIMD extraction)
    #[cfg(target_arch = "x86_64")]
    pub fn dot_product(a: &[f64], b: &[f64]) -> f64 {
        assert_eq!(a.len(), b.len());

        // Runtime CPU feature detection
        if !is_x86_feature_detected!("avx2") {
            return Self::dot_product_scalar(a, b);
        }

        unsafe {
            let chunks_a = a.chunks_exact(4);
            let chunks_b = b.chunks_exact(4);
            let remainder_start = a.len() - chunks_a.remainder().len();

            let mut sum_vec = _mm256_setzero_pd();

            // Process chunks of 4 using SIMD
            for (chunk_a, chunk_b) in chunks_a.zip(chunks_b) {
                let data_a = _mm256_loadu_pd(chunk_a.as_ptr());
                let data_b = _mm256_loadu_pd(chunk_b.as_ptr());
                let product = _mm256_mul_pd(data_a, data_b);
                sum_vec = _mm256_add_pd(sum_vec, product);
            }

            // Safe extraction using proper SIMD store operation
            let mut sums = [0.0f64; 4];
            _mm256_storeu_pd(sums.as_mut_ptr(), sum_vec);
            let mut total = sums[0] + sums[1] + sums[2] + sums[3];

            // Handle remainder
            for i in remainder_start..a.len() {
                total += a[i] * b[i];
            }

            total
        }
    }

    /// Scalar fallback for dot_product
    #[inline(always)]
    fn dot_product_scalar(a: &[f64], b: &[f64]) -> f64 {
        assert_eq!(a.len(), b.len());
        a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum()
    }

    /// Vectorized dot product using scalar fallback
    #[cfg(not(target_arch = "x86_64"))]
    pub fn dot_product(a: &[f64], b: &[f64]) -> f64 {
        assert_eq!(a.len(), b.len());
        a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum()
    }

    /// Calculate element-wise differences between arrays
    pub fn element_diff(a: &[f64], b: &[f64]) -> Vec<f64> {
        assert_eq!(a.len(), b.len());

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                return Self::element_diff_simd(a, b);
            }
        }

        // Scalar fallback
        a.iter().zip(b.iter()).map(|(&x, &y)| x - y).collect()
    }

    /// SIMD-optimized element-wise differences
    #[cfg(target_arch = "x86_64")]
    fn element_diff_simd(a: &[f64], b: &[f64]) -> Vec<f64> {
        let mut result = Vec::with_capacity(a.len());

        unsafe {
            let chunks_a = a.chunks_exact(4);
            let chunks_b = b.chunks_exact(4);
            let remainder_start = a.len() - chunks_a.remainder().len();

            for (chunk_a, chunk_b) in chunks_a.zip(chunks_b) {
                let data_a = _mm256_loadu_pd(chunk_a.as_ptr());
                let data_b = _mm256_loadu_pd(chunk_b.as_ptr());
                let diff = _mm256_sub_pd(data_a, data_b);

                let mut diffs = [0.0f64; 4];
                _mm256_storeu_pd(diffs.as_mut_ptr(), diff);
                result.extend_from_slice(&diffs);
            }

            // Handle remainder
            for i in remainder_start..a.len() {
                result.push(a[i] - b[i]);
            }
        }

        result
    }

    /// Calculate squared differences for variance computations
    pub fn squared_differences(values: &[f64], mean: f64) -> Vec<f64> {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                return Self::squared_differences_simd(values, mean);
            }
        }

        // Scalar fallback
        values
            .iter()
            .map(|&x| {
                let diff = x - mean;
                diff * diff
            })
            .collect()
    }

    /// SIMD-optimized squared differences
    #[cfg(target_arch = "x86_64")]
    fn squared_differences_simd(values: &[f64], mean: f64) -> Vec<f64> {
        let mut result = Vec::with_capacity(values.len());

        unsafe {
            let mean_vec = _mm256_set1_pd(mean);
            let chunks = values.chunks_exact(4);
            let remainder_start = values.len() - chunks.remainder().len();

            for chunk in chunks {
                let data = _mm256_loadu_pd(chunk.as_ptr());
                let diff = _mm256_sub_pd(data, mean_vec);
                let squared = _mm256_mul_pd(diff, diff);

                let mut squared_diffs = [0.0f64; 4];
                _mm256_storeu_pd(squared_diffs.as_mut_ptr(), squared);
                result.extend_from_slice(&squared_diffs);
            }

            // Handle remainder
            for i in remainder_start..values.len() {
                let diff = values[i] - mean;
                result.push(diff * diff);
            }
        }

        result
    }

    /// Find indices of values within a range
    pub fn find_in_range(values: &[f64], min_val: f64, max_val: f64) -> Vec<usize> {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                return Self::find_in_range_simd(values, min_val, max_val);
            }
        }

        // Scalar fallback
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

    /// SIMD-optimized range finding
    #[cfg(target_arch = "x86_64")]
    fn find_in_range_simd(values: &[f64], min_val: f64, max_val: f64) -> Vec<usize> {
        let mut result = Vec::new();

        unsafe {
            let min_vec = _mm256_set1_pd(min_val);
            let max_vec = _mm256_set1_pd(max_val);
            let chunks = values.chunks_exact(4);
            let remainder_start = values.len() - chunks.remainder().len();
            let mut base_idx = 0;

            for chunk in chunks {
                let data = _mm256_loadu_pd(chunk.as_ptr());
                let ge_min = _mm256_cmp_pd(data, min_vec, _CMP_GE_OQ);
                let le_max = _mm256_cmp_pd(data, max_vec, _CMP_LE_OQ);
                let in_range = _mm256_and_pd(ge_min, le_max);
                let mask = _mm256_movemask_pd(in_range) as u8;

                // Check each bit in the mask
                for i in 0..4 {
                    if (mask & (1 << i)) != 0 {
                        result.push(base_idx + i);
                    }
                }
                base_idx += 4;
            }

            // Handle remainder
            for (i, &value) in values[remainder_start..].iter().enumerate() {
                if value >= min_val && value <= max_val {
                    result.push(remainder_start + i);
                }
            }
        }

        result
    }
}
