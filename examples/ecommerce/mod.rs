//! Goldylox E-commerce Cache Implementation
//! 
//! This implementation uses Goldylox's multi-tier cache system with
//! realistic e-commerce data patterns with products, user sessions, and analytics.

// Module declarations
pub mod types;
pub mod setup;
pub mod seeding;
pub mod workloads;
pub mod display;

// Re-export main functions for backwards compatibility
pub use setup::*;
pub use seeding::*;
pub use workloads::*;
pub use display::*;

use std::sync::atomic::Ordering;

/// Main entry point for the e-commerce implementation
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Goldylox Real-World E-commerce Cache Implementation");
    println!("═════════════════════════════════════════════════════════");
    println!("   Multi-tier cache with ML eviction, SIMD ops, and coherence");
    println!("   Testing: Product catalog (hot), Sessions (warm), Analytics (cold)");
    println!("   Traffic patterns: Black Friday → Regular → Clearance Sale");
    println!();

    // Set up distributed cache nodes across multiple data centers
    let workload = setup_distributed_nodes()?;
    
    // Seed the cache with realistic e-commerce data
    println!("🌱 Seeding cache with realistic e-commerce data...");
    seed_product_catalog(&workload)?;
    seed_user_sessions(&workload)?;
    println!("   ✅ Seeded {} nodes with product catalog and user sessions\n", 
             workload.nodes.len());

    // Mark workload as running
    workload.running.store(true, Ordering::Relaxed);

    // Phase 1: Black Friday Rush - High concurrency, hot products
    workload.phase.store(1, Ordering::Relaxed);
    println!("🛍️  PHASE 1: Black Friday Rush Workload");
    println!("   Testing hot tier performance with 50,000+ operations");
    generate_black_friday_rush(&workload)?;
    display_cache_stats(&workload);
    println!();

    // Phase 2: Regular Browsing - Distributed access patterns  
    workload.phase.store(2, Ordering::Relaxed);
    println!("🏠 PHASE 2: Regular Browsing Workload");
    println!("   Testing warm tier ML eviction with diverse access patterns");
    generate_regular_browsing(&workload)?;
    display_cache_stats(&workload);
    println!();

    // Phase 3: Clearance Sale - Different hot products, tier migration
    workload.phase.store(3, Ordering::Relaxed);
    println!("🏷️  PHASE 3: Clearance Sale Workload");
    println!("   Testing cold→warm→hot tier promotion with 20,000 operations");
    generate_clearance_sale(&workload)?;
    display_cache_stats(&workload);
    
    // Stop workload
    workload.running.store(false, Ordering::Relaxed);
    
    // Final comprehensive results
    display_final_results(&workload);
    
    Ok(())
}