//! Goldylox E-commerce Cache Implementation
//!
//! This implementation uses Goldylox's multi-tier cache system with
//! realistic e-commerce data patterns with products, user sessions, and analytics.

// Module declarations
pub mod display;
pub mod seeding;
pub mod setup;
pub mod types;
pub mod workloads;

// Re-export main functions for backwards compatibility
pub use display::*;
pub use seeding::*;
pub use workloads::*;

use std::sync::atomic::Ordering;
use types::CacheCommand;

/// Main entry point for the e-commerce implementation
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Goldylox Real-World E-commerce Cache Implementation");
    println!("═════════════════════════════════════════════════════════");
    println!("   Multi-tier cache with ML eviction, SIMD ops, and coherence");
    println!("   Testing: Product catalog (hot), Sessions (warm), Analytics (cold)");
    println!("   Traffic patterns: Black Friday → Regular → Clearance Sale");
    println!();

    // Set up distributed cache nodes across multiple data centers
    let workload = setup::setup_distributed_nodes().await?;

    // Seed the cache with realistic e-commerce data
    println!("🌱 Seeding cache with realistic e-commerce data...");
    seed_product_catalog(&workload).await?;
    seed_user_sessions(&workload).await?;
    println!(
        "   ✅ Seeded {} nodes with product catalog and user sessions\n",
        workload.nodes.len()
    );

    // Mark workload as running
    workload.running.store(true, Ordering::Relaxed);

    // Start background processors for all caches before workload begins
    for node in &workload.nodes {
        if let Err(e) = node.product_cache.start_background_processor() {
            println!(
                "⚠️  Failed to start background processor for product cache: {:?}",
                e
            );
        }
        if let Err(e) = node.session_cache.start_background_processor() {
            println!(
                "⚠️  Failed to start background processor for session cache: {:?}",
                e
            );
        }
        if let Err(e) = node.analytics_cache.start_background_processor() {
            println!(
                "⚠️  Failed to start background processor for analytics cache: {:?}",
                e
            );
        }
    }

    // Phase 1: Black Friday Rush - High concurrency, hot products
    workload.phase.store(1, Ordering::Relaxed);
    println!("🛍️  PHASE 1: Black Friday Rush Workload");
    println!("   Testing hot tier performance with 50,000+ operations");

    // Force cache strategy to MLPredictive for high-traffic phase
    use goldylox::CacheStrategy;
    for node in &workload.nodes {
        node.product_cache
            .force_cache_strategy(CacheStrategy::MLPredictive);
    }
    generate_black_friday_rush(&workload).await?;

    display_cache_stats(&workload);
    println!();

    // Phase 2: Regular Browsing - Distributed access patterns
    workload.phase.store(2, Ordering::Relaxed);
    println!("🏠 PHASE 2: Regular Browsing Workload");
    println!("   Testing warm tier ML eviction with diverse access patterns");

    // Force cache strategy to AdaptiveLRU for regular browsing patterns
    for node in &workload.nodes {
        node.product_cache
            .force_cache_strategy(CacheStrategy::AdaptiveLRU);
    }
    generate_regular_browsing(&workload).await?;

    display_cache_stats(&workload);
    println!();

    // Phase 3: Clearance Sale - Different hot products, tier migration
    workload.phase.store(3, Ordering::Relaxed);
    println!("🏷️  PHASE 3: Clearance Sale Workload");
    println!("   Testing cold→warm→hot tier promotion with 20,000 operations");

    // Force cache strategy to ARC for tier migration testing
    for node in &workload.nodes {
        node.product_cache.force_cache_strategy(CacheStrategy::ARC);
    }
    generate_clearance_sale(&workload).await?;

    // Schedule async maintenance operations after intensive workload
    for node in &workload.nodes {
        let analytics_cache = &node.analytics_cache;
        let operation = |_ctx: &str| -> Result<u32, goldylox::prelude::CacheOperationError> {
            // Example async maintenance operation
            println!("🔧 Performing background analytics cache maintenance");
            Ok(42u32) // Return a result value
        };

        let result = analytics_cache.schedule_async_operation(
            operation,
            "maintenance".to_string(),
            Vec::<String>::new(),
        ).await;
        match result {
            Ok(task_id) => println!(
                "   ✅ Async maintenance task scheduled with ID: {}",
                task_id
            ),
            Err(e) => println!("   ⚠️  Async maintenance failed: {:?}", e),
        }
    }

    display_cache_stats(&workload);

    // Stop workload
    workload.running.store(false, Ordering::Relaxed);

    // Send shutdown command to worker threads
    for node in &workload.nodes {
        let shutdown_command = CacheCommand::Shutdown;
        if let Err(e) = node.command_sender.send(shutdown_command) {
            eprintln!("Failed to send shutdown command: {}", e);
        }
    }

    // Brief delay to allow worker thread to process shutdown
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Gracefully shutdown background processors after workload completes
    let handle = tokio::runtime::Handle::current();
    for node in &workload.nodes {
        // Shutdown policy engines first
        if let Err(e) = handle.block_on(node.product_cache.shutdown_policy_engine()) {
            println!(
                "⚠️  Failed to shutdown product cache policy engine: {:?}",
                e
            );
        }
        if let Err(e) = handle.block_on(node.session_cache.shutdown_policy_engine()) {
            println!(
                "⚠️  Failed to shutdown session cache policy engine: {:?}",
                e
            );
        }
        if let Err(e) = handle.block_on(node.analytics_cache.shutdown_policy_engine()) {
            println!(
                "⚠️  Failed to shutdown analytics cache policy engine: {:?}",
                e
            );
        }

        // Then shutdown the main cache systems
        if let Err(e) = handle.block_on(node.product_cache.shutdown_gracefully()) {
            println!("⚠️  Failed to shutdown product cache gracefully: {:?}", e);
        }
        if let Err(e) = handle.block_on(node.session_cache.shutdown_gracefully()) {
            println!("⚠️  Failed to shutdown session cache gracefully: {:?}", e);
        }
        if let Err(e) = handle.block_on(node.analytics_cache.shutdown_gracefully()) {
            println!("⚠️  Failed to shutdown analytics cache gracefully: {:?}", e);
        }
    }

    // Final comprehensive results
    display_final_results(&workload);

    Ok(())
}
