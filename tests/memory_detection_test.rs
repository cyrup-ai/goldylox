//! Unit tests for memory detection functionality
//!
//! Tests the SystemMemoryDetector with caching, circuit breaker, and fallback logic.

use std::sync::atomic::Ordering;
use std::time::Duration;

use goldylox::cache::memory::pressure_monitor::{
    get_system_memory_with_fallback, SystemMemoryDetector,
};

#[test]
fn test_memory_detection_basic() {
    let detector = SystemMemoryDetector::new();
    let memory = detector.get_total_system_memory();

    // Should return reasonable memory amount (at least 1GB, less than 1TB)
    assert!(
        memory >= 1024 * 1024 * 1024,
        "Memory should be at least 1GB, got: {}",
        memory
    );
    assert!(
        memory <= 1024_u64.pow(4),
        "Memory should be less than 1TB, got: {}",
        memory
    );
}

#[test]
fn test_memory_caching() {
    let detector = SystemMemoryDetector::new();

    let memory1 = detector.get_total_system_memory();
    let memory2 = detector.get_total_system_memory();

    // Should return same cached value
    assert_eq!(memory1, memory2, "Cached memory values should be identical");

    // Check that cache was actually used
    let cached = detector.cached_total_memory.load(Ordering::Relaxed);
    assert_eq!(cached, memory1, "Memory should be cached");
}

#[test]
fn test_intelligent_fallback() {
    let detector = SystemMemoryDetector::new();
    let fallback = detector.get_intelligent_fallback();

    // Should return reasonable fallback based on CPU count
    let cpu_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let expected = match cpu_count {
        1..=2 => 2 * 1024 * 1024 * 1024,   // 2GB for low-end systems
        3..=8 => 8 * 1024 * 1024 * 1024,   // 8GB for mid-range systems
        9..=16 => 16 * 1024 * 1024 * 1024, // 16GB for high-end systems
        _ => 32 * 1024 * 1024 * 1024,      // 32GB for server-class systems
    };

    assert_eq!(fallback, expected, "Fallback should match CPU count tiers");
}

#[test]
fn test_circuit_breaker_integration() {
    let detector = SystemMemoryDetector::new();

    // Circuit breaker should start in closed state
    let initial_memory = detector.get_total_system_memory();
    assert!(initial_memory > 0, "Should get valid memory initially");

    // Test that circuit breaker state can be accessed
    use goldylox::cache::manager::error_recovery::types::CircuitState;
    let state = detector.circuit_breaker.get_state(0);
    assert_eq!(state, CircuitState::Closed, "Circuit should start closed");
}

#[test]
fn test_global_memory_function() {
    let memory1 = get_system_memory_with_fallback();
    let memory2 = get_system_memory_with_fallback();

    // Should return consistent values from global instance
    assert_eq!(
        memory1, memory2,
        "Global function should return cached values"
    );
    assert!(memory1 >= 1024 * 1024 * 1024, "Should return at least 1GB");
}

#[test]
fn test_cache_duration() {
    let detector = SystemMemoryDetector::new();

    // Check that cache duration is set correctly
    assert_eq!(
        detector.cache_duration,
        Duration::from_secs(30),
        "Cache duration should be 30 seconds"
    );
}

#[test]
fn test_failure_tracking() {
    let detector = SystemMemoryDetector::new();

    // Initial failure count should be zero
    let initial_failures = detector.failure_count.load(Ordering::Relaxed);
    assert_eq!(initial_failures, 0, "Should start with zero failures");

    // After successful detection, failure count should remain zero
    let _memory = detector.get_total_system_memory();
    let failures_after_success = detector.failure_count.load(Ordering::Relaxed);
    assert_eq!(
        failures_after_success, 0,
        "Failure count should remain zero after success"
    );
}

#[test]
fn test_memory_detection_concurrency() {
    use std::sync::Arc;
    use std::thread;

    let detector = Arc::new(SystemMemoryDetector::new());
    let mut handles = vec![];

    // Spawn multiple threads to test concurrent access
    for _ in 0..4 {
        let detector_clone = Arc::clone(&detector);
        let handle = thread::spawn(move || detector_clone.get_total_system_memory());
        handles.push(handle);
    }

    let mut results = vec![];
    for handle in handles {
        results.push(handle.join().expect("Memory detection thread should complete"));
    }

    // All threads should get the same cached result
    assert!(
        results.windows(2).all(|w| w[0] == w[1]),
        "Concurrent access should return consistent results"
    );
    assert!(
        results[0] >= 1024 * 1024 * 1024,
        "Should return valid memory amount"
    );
}

#[cfg(test)]
mod platform_specific_tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn test_macos_memory_detection() {
        // Test that macOS memory detection works
        let memory = get_system_memory_with_fallback();
        assert!(memory > 0, "macOS should detect memory");

        // macOS systems typically have at least 4GB
        assert!(
            memory >= 4 * 1024 * 1024 * 1024,
            "macOS should have at least 4GB"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux_memory_detection() {
        // Test that Linux memory detection works
        let memory = get_system_memory_with_fallback();
        assert!(memory > 0, "Linux should detect memory");

        // Should be able to parse /proc/meminfo
        assert!(
            memory >= 1024 * 1024 * 1024,
            "Linux should have at least 1GB"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_windows_memory_detection() {
        // Test that Windows memory detection works
        let memory = get_system_memory_with_fallback();
        assert!(memory > 0, "Windows should detect memory");

        // Windows systems typically have at least 4GB
        assert!(
            memory >= 4 * 1024 * 1024 * 1024,
            "Windows should have at least 4GB"
        );
    }
}

#[test]
fn test_memory_bounds_validation() {
    let detector = SystemMemoryDetector::new();
    let memory = detector.get_total_system_memory();

    // Test reasonable bounds for modern systems
    const MIN_MEMORY: u64 = 512 * 1024 * 1024; // 512MB minimum
    const MAX_MEMORY: u64 = 2048 * 1024 * 1024 * 1024; // 2TB maximum

    assert!(
        memory >= MIN_MEMORY,
        "Memory should be at least 512MB, got: {}",
        memory
    );
    assert!(
        memory <= MAX_MEMORY,
        "Memory should be at most 2TB, got: {}",
        memory
    );
}
