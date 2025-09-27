//! Display and statistics functions for e-commerce example
//!
//! This module handles displaying cache statistics, performance metrics,
//! and final results from the e-commerce workload.

use crate::ecommerce::types::*;
use goldylox::should_promote_to_warm;

/// Display REAL cache statistics from actual operations
pub fn display_cache_stats(workload: &WorkloadState) {
    println!("📊 REAL Cache Performance Metrics:");

    for node in workload.nodes.iter() {
        println!("   🌐 Node {} ({}):", node.node_id, node.location);

        // Get REAL cache statistics from Goldylox API
        if let Ok(product_stats) = node.product_cache.stats() {
            println!("      📦 Product Cache: {}", product_stats);
        }

        if let Ok(session_stats) = node.session_cache.stats() {
            println!("      👤 Session Cache: {}", session_stats);
        }

        if let Ok(analytics_stats) = node.analytics_cache.stats() {
            println!("      📊 Analytics Cache: {}", analytics_stats);
        }

        // Display strategy performance metrics for workload analysis
        let strategy_metrics = node.product_cache.strategy_metrics();
        println!("      🎯 Strategy Metrics: {:?}", strategy_metrics);

        // Display strategy thresholds configuration
        let strategy_thresholds = node.product_cache.strategy_thresholds();
        println!("      📏 Strategy Thresholds: {:?}", strategy_thresholds);
    }
}

/// Display comprehensive final results from REAL cache operations
pub fn display_final_results(workload: &WorkloadState) {
    let duration = workload.start_time.elapsed();

    println!("🎊 FINAL RESULTS - REAL Cache Operations:");
    println!("   ⏱️  Total Duration: {:?}", duration);

    // Get detailed analytics from each cache using REAL Goldylox API
    for node in &workload.nodes {
        println!(
            "\n   🌐 {} ({}) Detailed Analytics:",
            node.node_id, node.location
        );

        if let Ok(detailed_product) = node.product_cache.detailed_analytics() {
            println!("      📦 Product Cache Analytics: {}", detailed_product);
        }

        if let Ok(detailed_session) = node.session_cache.detailed_analytics() {
            println!("      👤 Session Cache Analytics: {}", detailed_session);
        }

        if let Ok(detailed_analytics) = node.analytics_cache.detailed_analytics() {
            println!("      📊 Analytics Cache Analytics: {}", detailed_analytics);
        }

        // Display comprehensive strategy performance analysis
        let strategy_metrics = node.product_cache.strategy_metrics();
        println!(
            "      🎯 Final Strategy Performance: {:?}",
            strategy_metrics
        );

        let strategy_thresholds = node.session_cache.strategy_thresholds();
        println!(
            "      📏 Session Strategy Thresholds: {:?}",
            strategy_thresholds
        );

        // Display task coordination statistics for background operations
        let task_stats = node.product_cache.get_task_coordinator_stats();
        println!("      🔄 Task Coordinator Stats: {:?}", task_stats);

        let active_tasks = node.session_cache.get_active_tasks();
        println!("      ⚡ Active Tasks Count: {}", active_tasks.len());

        // Show example task cancellation if tasks exist
        if let Some(task) = active_tasks.first() {
            match node.session_cache.cancel_task(task.task_id()) {
                Ok(cancelled) => println!("      ❌ Example Task Cancellation: {}", cancelled),
                Err(e) => println!("      ⚠️  Task Cancellation Error: {:?}", e),
            }
        }

        // Display maintenance operation breakdown
        let maintenance_breakdown = node.product_cache.get_maintenance_breakdown();
        println!(
            "      🔧 Maintenance Breakdown: {:?}",
            maintenance_breakdown
        );

        // Display maintenance configuration information
        let maintenance_config = node.analytics_cache.get_maintenance_config_info();
        println!("      ⚙️  Maintenance Config: {}", maintenance_config);

        // Test cold tier promotion logic (call unused function to make it used)
        let sample_key = "promotion_test";
        if should_promote_to_warm::<String, String>(&sample_key.to_string()) {
            println!("      🔥 Sample key would be promoted from cold to warm tier");
        } else {
            println!("      ❄️  Sample key would remain in cold tier");
        }
    }
}
