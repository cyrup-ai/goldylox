//! Display and statistics functions for e-commerce example
//!
//! This module handles displaying cache statistics, performance metrics,
//! and final results from the e-commerce workload.

use crate::ecommerce::types::*;

/// Display REAL cache statistics from actual operations
pub fn display_cache_stats(workload: &WorkloadState) {
    println!("📊 REAL Cache Performance Metrics:");
    
    for (_i, node) in workload.nodes.iter().enumerate() {
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
    }
}

/// Display comprehensive final results from REAL cache operations
pub fn display_final_results(workload: &WorkloadState) {
    let duration = workload.start_time.elapsed();
    
    println!("🎊 FINAL RESULTS - REAL Cache Operations:");
    println!("   ⏱️  Total Duration: {:?}", duration);
    
    // Get detailed analytics from each cache using REAL Goldylox API
    for node in &workload.nodes {
        println!("\n   🌐 {} ({}) Detailed Analytics:", node.node_id, node.location);
        
        if let Ok(detailed_product) = node.product_cache.detailed_analytics() {
            println!("      📦 Product Cache Analytics: {}", detailed_product);
        }
        
        if let Ok(detailed_session) = node.session_cache.detailed_analytics() {
            println!("      👤 Session Cache Analytics: {}", detailed_session);
        }
        
        if let Ok(detailed_analytics) = node.analytics_cache.detailed_analytics() {
            println!("      📊 Analytics Cache Analytics: {}", detailed_analytics);
        }
    }
}