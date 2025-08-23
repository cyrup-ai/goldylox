//! Integration tests for memory pressure monitoring system

use goldylox::cache::config::types::{CacheConfig, MemoryConfig};
use goldylox::cache::memory::{MemoryManager, create_advanced_pressure_monitor, get_system_memory_with_fallback};

#[test]
fn test_memory_config_validation() {
    let mut config = MemoryConfig::default();
    
    // Valid configuration should pass
    assert!(config.validate().is_ok());
    
    // Invalid threshold ordering should fail
    config.low_pressure_threshold = 0.8;
    config.medium_pressure_threshold = 0.7;
    assert!(config.validate().is_err());
    
    // Reset to valid values
    config.low_pressure_threshold = 0.6;
    config.medium_pressure_threshold = 0.75;
    
    // Invalid threshold range should fail
    config.low_pressure_threshold = 1.5;
    assert!(config.validate().is_err());
    
    // Reset and test negative threshold
    config.low_pressure_threshold = -0.1;
    assert!(config.validate().is_err());
    
    // Reset and test zero timing parameters
    config.low_pressure_threshold = 0.6;
    config.alert_cooldown_ms = 0;
    assert!(config.validate().is_err());
    
    config.alert_cooldown_ms = 30000;
    config.sample_interval_ms = 0;
    assert!(config.validate().is_err());
    
    config.sample_interval_ms = 1000;
    config.max_history_samples = 0;
    assert!(config.validate().is_err());
    
    // Reset and test invalid memory limit
    config.max_history_samples = 256;
    config.max_memory_usage = Some(1024); // Less than 1MB minimum
    assert!(config.validate().is_err());
    
    // Valid memory limit should pass
    config.max_memory_usage = Some(64 * 1024 * 1024); // 64MB
    assert!(config.validate().is_ok());
}

#[test]
fn test_cache_config_validation() {
    let config = CacheConfig::default();
    assert!(config.validate().is_ok());
    
    let mut config = CacheConfig::default();
    config.memory_config.critical_pressure_threshold = 0.5; // Less than high threshold
    assert!(config.validate().is_err());
}

#[test]
fn test_memory_manager_creation() {
    let config = CacheConfig::default();
    assert!(config.validate().is_ok());
    
    let memory_manager = MemoryManager::new(&config);
    assert!(memory_manager.is_ok());
}

#[test]
fn test_configurable_pressure_thresholds() {
    let mut config = CacheConfig::default();
    config.memory_config.low_pressure_threshold = 0.5;
    config.memory_config.medium_pressure_threshold = 0.7;
    config.memory_config.high_pressure_threshold = 0.85;
    config.memory_config.critical_pressure_threshold = 0.95;
    
    assert!(config.validate().is_ok());
    
    let monitor = create_advanced_pressure_monitor(&config);
    assert!(monitor.is_ok());
}

#[test]
fn test_cross_platform_memory_detection() {
    let memory = get_system_memory_with_fallback();
    assert!(memory > 0);
    // Should detect at least 1GB on any modern system (or fallback to 8GB)
    assert!(memory >= 1024 * 1024 * 1024); 
}

#[test]
fn test_memory_config_defaults() {
    let config = MemoryConfig::default();
    
    // Test default values are reasonable
    assert_eq!(config.max_memory_usage, None);
    assert!(config.monitoring_enabled);
    assert_eq!(config.low_pressure_threshold, 0.6);
    assert_eq!(config.medium_pressure_threshold, 0.75);
    assert_eq!(config.high_pressure_threshold, 0.9);
    assert_eq!(config.critical_pressure_threshold, 0.98);
    assert!(config.leak_detection_enabled);
    assert_eq!(config.alert_cooldown_ms, 30000);
    assert_eq!(config.sample_interval_ms, 1000);
    assert_eq!(config.max_history_samples, 256);
    
    // Default values should be valid
    assert!(config.validate().is_ok());
}

#[test]
fn test_high_performance_config() {
    let config = CacheConfig::high_performance();
    assert!(config.validate().is_ok());
    
    let memory_manager = MemoryManager::new(&config);
    assert!(memory_manager.is_ok());
}

#[test]
fn test_low_memory_config() {
    let config = CacheConfig::low_memory();
    assert!(config.validate().is_ok());
    
    let memory_manager = MemoryManager::new(&config);
    assert!(memory_manager.is_ok());
}

#[test]
fn test_custom_memory_limit() {
    let mut config = CacheConfig::default();
    config.memory_config.max_memory_usage = Some(512 * 1024 * 1024); // 512MB
    
    assert!(config.validate().is_ok());
    
    let monitor = create_advanced_pressure_monitor(&config);
    assert!(monitor.is_ok());
}

#[test]
fn test_memory_config_boundary_values() {
    let mut config = MemoryConfig::default();
    
    // Test boundary values for thresholds
    config.low_pressure_threshold = 0.0;
    config.medium_pressure_threshold = 0.01;
    config.high_pressure_threshold = 0.02;
    config.critical_pressure_threshold = 1.0;
    assert!(config.validate().is_ok());
    
    // Test minimum memory limit
    config.max_memory_usage = Some(1024 * 1024); // Exactly 1MB
    assert!(config.validate().is_ok());
    
    // Test minimum timing values
    config.alert_cooldown_ms = 1;
    config.sample_interval_ms = 1;
    config.max_history_samples = 1;
    assert!(config.validate().is_ok());
}

#[test] 
fn test_threshold_ordering_validation() {
    let mut config = MemoryConfig::default();
    
    // Test each threshold ordering constraint
    
    // low >= medium should fail
    config.low_pressure_threshold = 0.7;
    config.medium_pressure_threshold = 0.7;
    assert!(config.validate().is_err());
    
    // medium >= high should fail
    config.low_pressure_threshold = 0.6;
    config.medium_pressure_threshold = 0.8;
    config.high_pressure_threshold = 0.8;
    assert!(config.validate().is_err());
    
    // high >= critical should fail
    config.medium_pressure_threshold = 0.75;
    config.high_pressure_threshold = 0.9;
    config.critical_pressure_threshold = 0.9;
    assert!(config.validate().is_err());
    
    // Proper ordering should pass
    config.critical_pressure_threshold = 0.95;
    assert!(config.validate().is_ok());
}