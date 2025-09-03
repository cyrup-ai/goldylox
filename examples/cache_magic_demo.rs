//! Goldylox Cache Demo - Real-world E-commerce Workload
//! 
//! This demo showcases Goldylox cache performance under realistic e-commerce traffic.
//! All advanced features (ML, SIMD, coherence) operate transparently through the public API.

use goldylox::Goldylox;
use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::{Duration, Instant};
use std::thread;
use rand::{thread_rng, Rng};

/// E-commerce product for caching
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, bincode::Encode, bincode::Decode)]
pub struct Product {
    pub id: u64,
    pub name: String,
    pub category: String,
    pub price: f64,
    pub inventory: u32,
    pub popularity_score: f64,
    pub metadata: BTreeMap<String, String>,
}

/// Simple demo statistics
#[derive(Debug, Default)]
pub struct DemoStats {
    pub operations: AtomicU64,
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub updates: AtomicU64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 GOLDYLOX CACHE DEMO");
    println!("══════════════════════");
    println!("📦 Real e-commerce workload with 10,000 products");
    println!("🧠 All advanced features active transparently\n");

    // Create cache with all advanced features enabled by default
    let cache = Goldylox::<String, Product>::new()?;
    println!("✅ Goldylox cache initialized with all features active\n");

    // Seed cache with 10,000 products
    println!("📦 Loading 10,000 products...");
    seed_products(&cache)?;
    println!("✅ Product catalog loaded\n");

    let stats = Arc::new(DemoStats::default());

    // Run workload phases
    println!("🎬 Running 3-phase workload (9 minutes total)...\n");

    // Phase 1: Black Friday (concentrated load)
    println!("Phase 1: Black Friday - Heavy load on popular products");
    run_black_friday_phase(&cache, &stats)?;
    print_phase_stats("BLACK FRIDAY", &stats, &cache)?;

    // Phase 2: Regular browsing (distributed load)  
    println!("Phase 2: Regular browsing - Distributed catalog access");
    run_regular_browsing_phase(&cache, &stats)?;
    print_phase_stats("REGULAR BROWSING", &stats, &cache)?;

    // Phase 3: Clearance sale (new hot products)
    println!("Phase 3: Clearance sale - Different popular products");
    run_clearance_phase(&cache, &stats)?;
    print_phase_stats("CLEARANCE SALE", &stats, &cache)?;

    // Final results
    print_final_results(&stats, &cache)?;

    Ok(())
}

fn seed_products(cache: &Goldylox<String, Product>) -> Result<(), Box<dyn std::error::Error>> {
    let categories = vec!["Electronics", "Clothing", "Home", "Sports", "Books"];
    let mut rng = thread_rng();
    
    for id in 1..=10_000 {
        let category = &categories[rng.gen_range(0..categories.len())];
        
        let product = Product {
            id,
            name: format!("{} Product #{}", category, id),
            category: category.to_string(),
            price: rng.gen_range(10.0..1000.0),
            inventory: rng.gen_range(0..1000),
            popularity_score: rng.gen_range(0.0..1.0),
            metadata: BTreeMap::new(),
        };
        
        cache.put(format!("product:{}", id), product)?;
        
        if id % 2000 == 0 {
            println!("   {} products loaded...", id);
        }
    }
    
    Ok(())
}

fn run_black_friday_phase(
    cache: &Goldylox<String, Product>, 
    stats: &Arc<DemoStats>
) -> Result<(), Box<dyn std::error::Error>> {
    
    let duration = Duration::from_secs(180); // 3 minutes
    let start = Instant::now();
    let mut rng = thread_rng();
    
    while start.elapsed() < duration {
        // 80% access to popular products (1-1000)
        let product_id = if rng.gen_range(0.0..1.0) < 0.8 {
            rng.gen_range(1..=1000)
        } else {
            rng.gen_range(1001..=10000)
        };
        
        let key = format!("product:{}", product_id);
        if let Some(_product) = cache.get(&key) {
            stats.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            stats.misses.fetch_add(1, Ordering::Relaxed);
        }
        stats.operations.fetch_add(1, Ordering::Relaxed);
        
        // Some inventory updates
        if rng.gen_range(0.0..1.0) < 0.1 {
            if let Some(mut product) = cache.get(&key) {
                product.inventory = product.inventory.saturating_sub(1);
                cache.put(key, product)?;
                stats.updates.fetch_add(1, Ordering::Relaxed);
            }
        }
        
        thread::sleep(Duration::from_millis(10));
    }
    
    Ok(())
}

fn run_regular_browsing_phase(
    cache: &Goldylox<String, Product>,
    stats: &Arc<DemoStats>
) -> Result<(), Box<dyn std::error::Error>> {
    
    let duration = Duration::from_secs(180); // 3 minutes  
    let start = Instant::now();
    let mut rng = thread_rng();
    
    while start.elapsed() < duration {
        // Distributed access across entire catalog
        let product_id = rng.gen_range(1..=10000);
        let key = format!("product:{}", product_id);
        
        if let Some(_product) = cache.get(&key) {
            stats.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            stats.misses.fetch_add(1, Ordering::Relaxed);
        }
        stats.operations.fetch_add(1, Ordering::Relaxed);
        
        thread::sleep(Duration::from_millis(10));
    }
    
    Ok(())
}

fn run_clearance_phase(
    cache: &Goldylox<String, Product>,
    stats: &Arc<DemoStats>
) -> Result<(), Box<dyn std::error::Error>> {
    
    let duration = Duration::from_secs(180); // 3 minutes
    let start = Instant::now();
    let mut rng = thread_rng();
    
    while start.elapsed() < duration {
        // 70% access to clearance products (8000-9000)
        let product_id = if rng.gen_range(0.0..1.0) < 0.7 {
            rng.gen_range(8000..=9000)
        } else {
            rng.gen_range(1..=7999)
        };
        
        let key = format!("product:{}", product_id);
        if let Some(_product) = cache.get(&key) {
            stats.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            stats.misses.fetch_add(1, Ordering::Relaxed);
        }
        stats.operations.fetch_add(1, Ordering::Relaxed);
        
        // Frequent inventory updates for hot clearance items
        if rng.gen_range(0.0..1.0) < 0.2 {
            if let Some(mut product) = cache.get(&key) {
                product.inventory = product.inventory.saturating_sub(1);
                cache.put(key, product)?;
                stats.updates.fetch_add(1, Ordering::Relaxed);
            }
        }
        
        thread::sleep(Duration::from_millis(10));
    }
    
    Ok(())
}

fn print_phase_stats(
    phase: &str,
    stats: &Arc<DemoStats>,
    cache: &Goldylox<String, Product>
) -> Result<(), Box<dyn std::error::Error>> {
    
    let ops = stats.operations.load(Ordering::Relaxed);
    let hits = stats.hits.load(Ordering::Relaxed);
    let misses = stats.misses.load(Ordering::Relaxed);
    let updates = stats.updates.load(Ordering::Relaxed);
    let hit_rate = if hits + misses > 0 { hits as f64 / (hits + misses) as f64 * 100.0 } else { 0.0 };
    
    println!("✅ {} complete", phase);
    println!("   📊 Operations: {} | Hit rate: {:.1}% | Updates: {}", ops, hit_rate, updates);
    
    // Show cache stats if available
    if let Ok(cache_stats) = cache.stats() {
        println!("   🔍 Cache: {}", cache_stats);
    }
    
    println!();
    
    Ok(())
}

fn print_final_results(
    stats: &Arc<DemoStats>,
    cache: &Goldylox<String, Product>
) -> Result<(), Box<dyn std::error::Error>> {
    
    let total_ops = stats.operations.load(Ordering::Relaxed);
    let total_hits = stats.hits.load(Ordering::Relaxed);
    let total_misses = stats.misses.load(Ordering::Relaxed);
    let total_updates = stats.updates.load(Ordering::Relaxed);
    let final_hit_rate = if total_hits + total_misses > 0 { 
        total_hits as f64 / (total_hits + total_misses) as f64 * 100.0 
    } else { 
        0.0 
    };
    
    println!("🎉 DEMO COMPLETE!");
    println!("═════════════════");
    println!("📊 Total operations: {}", total_ops);
    println!("🎯 Overall hit rate: {:.1}%", final_hit_rate);
    println!("🔄 Updates processed: {}", total_updates);
    
    if let Ok(final_stats) = cache.stats() {
        println!("📈 Final cache state: {}", final_stats);
    }
    
    println!("\n🚀 Goldylox cache handled evolving workload patterns!");
    println!("   All advanced features operated transparently through simple API");
    
    Ok(())
}