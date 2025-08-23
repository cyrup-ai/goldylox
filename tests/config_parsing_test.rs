use goldylox::cache::config::loader::ConfigLoader;

#[test]
fn test_toml_config_parsing() {
    let config = ConfigLoader::from_file("sample_config.toml").expect("Failed to load TOML config");

    // Verify hot tier settings
    assert_eq!(config.hot_tier.max_entries, 1024);
    assert!(config.hot_tier.enabled);
    assert_eq!(config.hot_tier.cache_line_size, 64);
    assert_eq!(config.hot_tier.prefetch_distance, 2);

    // Verify warm tier settings
    assert_eq!(config.warm_tier.max_entries, 8192);
    assert_eq!(config.warm_tier.max_size_bytes, 16777216);
    assert!(config.warm_tier.enabled);
    assert_eq!(config.warm_tier.entry_timeout_ns, 300000000000);

    // Verify cold tier settings
    assert!(config.cold_tier.enabled);
    assert_eq!(config.cold_tier.storage_path.as_str(), "/tmp/cache_storage");
    assert_eq!(config.cold_tier.compression_level, 6);

    // Verify worker settings
    assert!(config.worker.enabled);
    assert_eq!(config.worker.thread_pool_size, 4);
    assert_eq!(config.worker.task_queue_capacity, 256);

    // Verify monitoring settings
    assert!(config.monitoring.enabled);
    assert_eq!(config.monitoring.sample_interval_ns, 100000000);
    assert_eq!(config.monitoring.max_history_samples, 1000);
}

#[test]
fn test_json_config_parsing() {
    let config = ConfigLoader::from_file("sample_config.json").expect("Failed to load JSON config");

    // Verify hot tier settings
    assert_eq!(config.hot_tier.max_entries, 512);
    assert!(config.hot_tier.enabled);
    assert_eq!(config.hot_tier.cache_line_size, 64);
    assert_eq!(config.hot_tier.prefetch_distance, 1);

    // Verify warm tier settings
    assert_eq!(config.warm_tier.max_entries, 4096);
    assert_eq!(config.warm_tier.max_size_bytes, 8388608);
    assert!(config.warm_tier.enabled);
    assert_eq!(config.warm_tier.entry_timeout_ns, 600000000000);

    // Verify cold tier settings (disabled in JSON config)
    assert!(!config.cold_tier.enabled);
    assert_eq!(config.cold_tier.compression_level, 3);

    // Verify worker settings
    assert!(config.worker.enabled);
    assert_eq!(config.worker.thread_pool_size, 2);
    assert_eq!(config.worker.task_queue_capacity, 128);

    // Verify monitoring settings (disabled in JSON config)
    assert!(!config.monitoring.enabled);
    assert_eq!(config.monitoring.sample_interval_ns, 50000000);
    assert_eq!(config.monitoring.max_history_samples, 500);
}

#[test]
fn test_config_serialization_roundtrip() {
    // Load TOML config
    let original_config =
        ConfigLoader::from_file("sample_config.toml").expect("Failed to load TOML config");

    // Serialize to JSON
    let json_str = ConfigLoader::to_json(&original_config).expect("Failed to serialize to JSON");

    // Deserialize from JSON
    let json_config = ConfigLoader::from_json(&json_str).expect("Failed to deserialize from JSON");

    // Verify key fields match
    assert_eq!(
        original_config.hot_tier.max_entries,
        json_config.hot_tier.max_entries
    );
    assert_eq!(
        original_config.warm_tier.max_entries,
        json_config.warm_tier.max_entries
    );
    assert_eq!(
        original_config.cold_tier.enabled,
        json_config.cold_tier.enabled
    );
    assert_eq!(
        original_config.worker.thread_pool_size,
        json_config.worker.thread_pool_size
    );
}

#[test]
fn test_config_validation_errors() {
    use goldylox::cache::config::types::{CacheConfig, ConfigError};

    // Test invalid power of 2 validation
    let mut invalid_config = CacheConfig::default();
    invalid_config.hot_tier.enabled = true;
    invalid_config.hot_tier.max_entries = 100; // Not a power of 2

    let result = ConfigLoader::to_json(&invalid_config);
    assert!(result.is_err());

    if let Err(ConfigError::InvalidFieldValue { field, reason, .. }) = result {
        assert_eq!(field, "hot_tier.max_entries");
        assert!(reason.contains("power of 2"));
    } else {
        panic!("Expected InvalidFieldValue error");
    }
}

#[test]
fn test_file_not_found_error() {
    let result = ConfigLoader::from_file("nonexistent_config.toml");
    assert!(result.is_err());

    if let Err(err) = result {
        assert!(err.to_string().contains("not found"));
    }
}

#[test]
fn test_unsupported_format_error() {
    let result = ConfigLoader::from_file("sample_config.xml");
    assert!(result.is_err());

    if let Err(err) = result {
        assert!(err.to_string().contains("Unsupported file format"));
    }
}
