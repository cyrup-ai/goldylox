//! Error recovery data provider abstraction for telemetry integration
//!
//! This module provides zero-allocation trait abstraction for accessing error recovery metrics
//! without exposing the full ErrorRecoverySystem to all components.

/// Zero-allocation trait for accessing error recovery metrics
pub trait ErrorRecoveryProvider {
    /// Get total error count across all error types
    fn get_total_errors(&self) -> u64;
    
    /// Get error distribution array (16 error types)
    fn get_error_distribution(&self) -> [f64; 16];
    
    /// Get system health score (0.0 = unhealthy, 1.0 = healthy)
    fn get_health_score(&self) -> f64;
    
    /// Get count of active recovery operations
    fn get_active_recoveries(&self) -> u32;
    
    /// Get mean time to recovery in milliseconds
    fn get_mttr_ms(&self) -> f64;
}

/// Real error recovery provider backed by UnifiedCacheManager
#[derive(Debug)]
#[allow(dead_code)] // Error recovery - unified manager error provider for comprehensive error tracking
pub struct UnifiedManagerErrorProvider<'a, K: crate::cache::traits::core::CacheKey, V: crate::cache::traits::core::CacheValue> {
    error_recovery: &'a crate::cache::manager::error_recovery::core::ErrorRecoverySystem<K, V>,
}

impl<'a, K: crate::cache::traits::core::CacheKey, V: crate::cache::traits::core::CacheValue> UnifiedManagerErrorProvider<'a, K, V> {
    #[inline(always)]
    #[allow(dead_code)] // Error recovery - constructor for unified manager error provider
    pub fn new(error_recovery: &'a crate::cache::manager::error_recovery::core::ErrorRecoverySystem<K, V>) -> Self {
        Self { error_recovery }
    }
}

impl<K: crate::cache::traits::core::CacheKey, V: crate::cache::traits::core::CacheValue> ErrorRecoveryProvider for UnifiedManagerErrorProvider<'_, K, V> {
    #[inline(always)]
    fn get_total_errors(&self) -> u64 {
        self.error_recovery.error_stats.get_total_error_count()
    }
    
    #[inline(always)]
    fn get_error_distribution(&self) -> [f64; 16] {
        self.error_recovery.error_stats.get_error_distribution()
    }
    
    #[inline(always)]
    fn get_health_score(&self) -> f64 {
        self.error_recovery.error_stats.get_health_score()
    }
    
    #[inline(always)]
    fn get_active_recoveries(&self) -> u32 {
        self.error_recovery.recovery_strategies.get_active_recovery_count()
    }
    
    #[inline(always)]
    fn get_mttr_ms(&self) -> f64 {
        self.error_recovery.error_stats.get_mttr_ms()
    }
}

/// Fallback error recovery provider with estimated/default values
#[derive(Debug, Default)]
pub struct FallbackErrorProvider {
    estimated_errors: std::sync::atomic::AtomicU64,
    estimated_health: std::sync::atomic::AtomicU64, // Store as u64 bits
}

impl FallbackErrorProvider {
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            estimated_errors: std::sync::atomic::AtomicU64::new(0),
            estimated_health: std::sync::atomic::AtomicU64::new(f64::to_bits(0.95)), // Default 95% health
        }
    }
    
    #[inline(always)]
    #[allow(dead_code)] // Error recovery - fallback provider estimate update method
    pub fn update_estimates(&self, errors: u64, health: f64) {
        self.estimated_errors.store(errors, std::sync::atomic::Ordering::Relaxed);
        self.estimated_health.store(f64::to_bits(health), std::sync::atomic::Ordering::Relaxed);
    }
}

impl ErrorRecoveryProvider for FallbackErrorProvider {
    #[inline(always)]
    fn get_total_errors(&self) -> u64 {
        self.estimated_errors.load(std::sync::atomic::Ordering::Relaxed)
    }
    
    #[inline(always)]
    fn get_error_distribution(&self) -> [f64; 16] {
        // Estimated distribution based on common error patterns
        [0.15, 0.12, 0.10, 0.08, 0.07, 0.06, 0.05, 0.04, 0.03, 0.03, 0.02, 0.02, 0.01, 0.01, 0.01, 0.20]
    }
    
    #[inline(always)]
    fn get_health_score(&self) -> f64 {
        f64::from_bits(self.estimated_health.load(std::sync::atomic::Ordering::Relaxed))
    }
    
    #[inline(always)]
    fn get_active_recoveries(&self) -> u32 {
        0 // Assume no active recoveries for fallback
    }
    
    #[inline(always)]
    fn get_mttr_ms(&self) -> f64 {
        100.0 // Default 100ms estimated MTTR
    }
}

/// Empty error recovery provider (for testing/minimal contexts)
#[allow(dead_code)] // Error recovery - empty provider for testing and minimal contexts
#[derive(Debug, Default)]
pub struct EmptyErrorProvider;

impl ErrorRecoveryProvider for EmptyErrorProvider {
    #[inline(always)]
    fn get_total_errors(&self) -> u64 { 0 }
    
    #[inline(always)]
    fn get_error_distribution(&self) -> [f64; 16] { [0.0; 16] }
    
    #[inline(always)]
    fn get_health_score(&self) -> f64 { 1.0 }
    
    #[inline(always)]
    fn get_active_recoveries(&self) -> u32 { 0 }
    
    #[inline(always)]
    fn get_mttr_ms(&self) -> f64 { 0.0 }
}