//! Node setup and cache configuration for e-commerce example
//!
//! This module handles the creation and configuration of distributed cache nodes
//! with realistic e-commerce workload optimization.

use crate::ecommerce::types::*;
use goldylox::Goldylox;
use std::sync::{Arc, atomic::{AtomicU64, AtomicBool}};
use std::time::Instant;

/// Set up the distributed cache nodes with realistic configuration
pub fn setup_distributed_nodes() -> Result<WorkloadState, Box<dyn std::error::Error>> {
    let mut nodes = Vec::new();
    
    // Create three geographically distributed nodes
    let node_configs = vec![
        ("us-east-1", "US East (Virginia)"),
        ("us-west-1", "US West (Oregon)"), 
        ("eu-west-1", "Europe (Ireland)"),
    ];

    for (node_id, location) in node_configs {
        let node = create_cache_node(node_id, location)?;
        nodes.push(Arc::new(node));
    }

    let workload = WorkloadState {
        nodes,
        start_time: Instant::now(),
        phase: AtomicU64::new(0),
        running: AtomicBool::new(false),
    };

    Ok(workload)
}

/// Create a single cache node with optimized configuration for e-commerce workloads
pub fn create_cache_node(node_id: &str, location: &str) -> Result<CacheNode, Box<dyn std::error::Error>> {
    
    // Product cache: Hot tier optimized for frequent lookups
    let product_cache = Goldylox::<String, Product>::builder()
        .hot_tier_max_entries(5_000)        // Keep top 5K products hot
        .hot_tier_memory_limit_mb(128)
        .warm_tier_max_entries(50_000)      // 50K products in warm tier
        .warm_tier_max_memory_bytes(512 * 1024 * 1024) // 512MB
        .cold_tier_base_dir("./tmp")
        .cache_id(format!("goldylox_products_{}", node_id))
        .cold_tier_max_size_bytes(5 * 1024 * 1024 * 1024) // 5GB for full catalog
        .compression_level(6)               // Balanced compression
        .background_worker_threads(2)
        .build()?;

    // Session cache: Warm tier optimized for user sessions  
    let session_cache = Goldylox::<String, UserSession>::builder()
        .hot_tier_max_entries(1_000)        // Active sessions
        .hot_tier_memory_limit_mb(64)
        .warm_tier_max_entries(10_000)      // Recent sessions
        .warm_tier_max_memory_bytes(256 * 1024 * 1024) // 256MB
        .build()?;

    // Analytics cache: Cold tier optimized for large, infrequent data
    let analytics_cache = Goldylox::<String, AnalyticsEvent>::builder()
        .cold_tier_base_dir("./tmp")
        .cache_id(format!("goldylox_analytics_{}", node_id))
        .cold_tier_max_size_bytes(20 * 1024 * 1024 * 1024) // 20GB for analytics
        .compression_level(9)               // Maximum compression for analytics
        .build()?;

    Ok(CacheNode {
        node_id: node_id.to_string(),
        location: location.to_string(),
        product_cache: Arc::new(product_cache),
        session_cache: Arc::new(session_cache),
        analytics_cache: Arc::new(analytics_cache),
    })
}