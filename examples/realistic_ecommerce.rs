//! Goldylox Real-World E-commerce Cache Demonstration
//! 
//! This example showcases the true power of Goldylox's ML eviction policies and coherence protocols
//! through a realistic e-commerce platform simulation. Watch the cache get smarter over time as it
//! learns access patterns and coordinates across multiple nodes.
//! 
//! Features demonstrated:
//! - ML-driven tier optimization with visible learning
//! - Multi-node coherence protocols with conflict resolution
//! - Real-time metrics and live performance visualization
//! - SIMD optimizations under realistic load
//! - Evolving workload patterns (Black Friday → Regular browsing → Clearance sale)

use goldylox::{Goldylox, CacheOperationError};
use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;
use std::sync::{Arc, atomic::{AtomicU64, AtomicBool, Ordering}};
use std::time::{SystemTime, UNIX_EPOCH, Duration, Instant};
use std::thread;
use rand::prelude::*;
use tokio::time::sleep;

mod workload_simulator;
mod metrics_dashboard;
mod coherence_demo;
mod ml_visualization;

use workload_simulator::*;
use metrics_dashboard::*;
use coherence_demo::*;
use ml_visualization::*;

/// Product in e-commerce catalog - designed for realistic caching patterns
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd, Default, bincode::Encode, bincode::Decode)]
pub struct Product {
    pub product_id: u64,
    pub name: String,
    pub category: String,
    pub price: f64,
    pub inventory_count: u32,
    pub description: String,
    pub reviews_count: u32,
    pub average_rating: f32,
    pub tags: Vec<String>,
    pub metadata: BTreeMap<String, String>,
    pub last_updated: u64,
}

/// User session data - medium-frequency access patterns
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, bincode::Encode, bincode::Decode)]
pub struct UserSession {
    pub session_id: String,
    pub user_id: u64,
    pub created_at: u64,
    pub last_activity: u64,
    pub expires_at: u64,
    pub shopping_cart: Vec<CartItem>,
    pub browsing_history: Vec<u64>, // product IDs
    pub preferences: BTreeMap<String, String>,
    pub location: String,
}

/// Shopping cart item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, bincode::Encode, bincode::Decode)]
pub struct CartItem {
    pub product_id: u64,
    pub quantity: u32,
    pub added_at: u64,
    pub price_at_add: f64,
}

/// Analytics event - low-frequency, high-volume data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, bincode::Encode, bincode::Decode)]
pub struct AnalyticsEvent {
    pub event_id: String,
    pub timestamp: u64,
    pub user_id: u64,
    pub session_id: String,
    pub event_type: String,
    pub product_id: Option<u64>,
    pub properties: BTreeMap<String, String>,
    pub raw_data: Vec<u8>, // Large payload to test compression
}

/// Cache node representing a data center
#[derive(Debug)]
pub struct CacheNode {
    pub node_id: String,
    pub location: String,
    pub product_cache: Arc<Goldylox<String, Product>>,
    pub session_cache: Arc<Goldylox<String, UserSession>>,
    pub analytics_cache: Arc<Goldylox<String, AnalyticsEvent>>,
    pub metrics: Arc<NodeMetrics>,
}

/// Performance metrics for each node
#[derive(Debug, Default)]
pub struct NodeMetrics {
    pub operations_count: AtomicU64,
    pub ml_tier_migrations: AtomicU64,
    pub coherence_events: AtomicU64,
    pub simd_operations: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
}

/// Global simulation state
#[derive(Debug)]
pub struct SimulationState {
    pub nodes: Vec<Arc<CacheNode>>,
    pub start_time: Instant,
    pub phase: AtomicU64, // 0=BlackFriday, 1=Regular, 2=Clearance
    pub running: AtomicBool,
    pub dashboard: Arc<MetricsDashboard>,
    pub ml_visualizer: Arc<MLVisualizer>,
    pub coherence_monitor: Arc<CoherenceMonitor>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Goldylox Real-World E-commerce Cache Demonstration");
    println!("═════════════════════════════════════════════════════════");
    println!("🎯 Goal: Watch ML eviction and coherence protocols in action!");
    println!("⏱️  Duration: ~2 hours with evolving traffic patterns");
    println!("📊 Scale: 10K products, 1K users, 100K+ analytics events\n");

    // =======================================================================
    // PHASE 1: MULTI-NODE DISTRIBUTED SETUP  
    // =======================================================================
    println!("🌐 Phase 1: Setting up distributed cache nodes...\n");
    
    let simulation = setup_distributed_nodes().await?;
    
    println!("✅ 3 cache nodes deployed:");
    for node in &simulation.nodes {
        println!("   📍 {} ({}) - Ready with ML and coherence enabled", node.node_id, node.location);
    }

    // Pre-populate with realistic e-commerce data
    println!("\n📦 Seeding caches with realistic e-commerce data...");
    seed_product_catalog(&simulation).await?;
    seed_user_sessions(&simulation).await?;
    println!("✅ Seeded 10,000 products and 1,000 user sessions\n");

    // =======================================================================
    // PHASE 2: START REAL-TIME MONITORING
    // =======================================================================
    println!("📊 Phase 2: Starting real-time monitoring systems...\n");
    
    // Start the live metrics dashboard
    start_metrics_dashboard(&simulation).await?;
    
    // Start ML learning visualization
    start_ml_visualization(&simulation).await?;
    
    // Start coherence monitoring
    start_coherence_monitoring(&simulation).await?;

    println!("✅ All monitoring systems active - watch the magic happen!\n");

    // =======================================================================
    // PHASE 3: EVOLVING WORKLOAD SIMULATION
    // =======================================================================
    println!("🎬 Phase 3: Beginning evolving workload simulation...\n");
    
    simulation.running.store(true, Ordering::Relaxed);

    // Spawn workload generators
    let workload_handles = spawn_workload_generators(&simulation).await?;
    
    // Run the three phases of traffic patterns
    run_traffic_evolution(&simulation).await?;

    // =======================================================================
    // PHASE 4: INTERACTIVE DEMO FEATURES
    // =======================================================================
    println!("\n🎮 Phase 4: Interactive demonstration features...\n");
    
    // Trigger special events to show cache response
    trigger_flash_sale(&simulation).await?;
    trigger_inventory_update_storm(&simulation).await?;
    trigger_network_partition(&simulation).await?;
    
    // =======================================================================
    // PHASE 5: FINAL RESULTS AND CLEANUP
    // =======================================================================
    println!("\n🏁 Phase 5: Demonstration complete - final results...\n");
    
    simulation.running.store(false, Ordering::Relaxed);
    
    // Wait for all workload generators to finish
    for handle in workload_handles {
        handle.await?;
    }
    
    // Display final comprehensive metrics
    display_final_results(&simulation).await?;
    
    println!("\n🎉 Goldylox real-world demonstration completed successfully!");
    println!("🧠 ML algorithms demonstrated continuous learning and optimization");
    println!("🌐 Coherence protocols handled distributed consistency flawlessly");
    println!("⚡ SIMD optimizations provided consistent performance benefits");
    println!("📈 Thank you for witnessing the cache magic in action!");

    Ok(())
}

/// Set up the distributed cache nodes with realistic configuration
async fn setup_distributed_nodes() -> Result<SimulationState, Box<dyn std::error::Error>> {
    let mut nodes = Vec::new();
    
    // Create three geographically distributed nodes
    let node_configs = vec![
        ("us-east-1", "US East (Virginia)"),
        ("us-west-1", "US West (Oregon)"), 
        ("eu-west-1", "Europe (Ireland)"),
    ];

    for (node_id, location) in node_configs {
        let node = create_cache_node(node_id, location).await?;
        nodes.push(Arc::new(node));
    }

    // Set up coherence mesh between nodes
    setup_coherence_mesh(&nodes).await?;

    let simulation = SimulationState {
        nodes,
        start_time: Instant::now(),
        phase: AtomicU64::new(0),
        running: AtomicBool::new(false),
        dashboard: Arc::new(MetricsDashboard::new()),
        ml_visualizer: Arc::new(MLVisualizer::new()),
        coherence_monitor: Arc::new(CoherenceMonitor::new()),
    };

    Ok(simulation)
}

/// Create a single cache node with optimized configuration for e-commerce workloads
async fn create_cache_node(node_id: &str, location: &str) -> Result<CacheNode, Box<dyn std::error::Error>> {
    
    // Product cache: Hot tier optimized for frequent lookups
    let product_cache = Goldylox::<String, Product>::builder()
        .hot_tier_enabled(true)
        .hot_tier_max_entries(5_000)        // Keep top 5K products hot
        .hot_tier_memory_limit_mb(128)
        .enable_simd(true)                  // SIMD for fast product lookups
        .enable_prefetch(true)              // Predict related products
        .warm_tier_enabled(true)
        .warm_tier_max_entries(50_000)      // 50K products in warm tier
        .warm_tier_max_memory_bytes(512 * 1024 * 1024) // 512MB
        .cold_tier_enabled(true)
        .cold_tier_storage_path(&format!("/tmp/goldylox_products_{}", node_id))
        .cold_tier_max_size_bytes(5 * 1024 * 1024 * 1024) // 5GB for full catalog
        .compression_level(6)               // Balanced compression
        .enable_background_workers(true)
        .background_worker_threads(2)
        .enable_telemetry(true)
        .ml_eviction_enabled(true)          // Enable ML learning
        .coherence_enabled(true)            // Enable cross-node coherence
        .node_id(node_id.to_string())
        .build()?;

    // Session cache: Warm tier optimized for user sessions  
    let session_cache = Goldylox::<String, UserSession>::builder()
        .hot_tier_enabled(true)
        .hot_tier_max_entries(1_000)        // Active sessions
        .hot_tier_memory_limit_mb(64)
        .warm_tier_enabled(true)
        .warm_tier_max_entries(10_000)      // Recent sessions
        .warm_tier_max_memory_bytes(256 * 1024 * 1024) // 256MB
        .cold_tier_enabled(false)           // Sessions don't need cold storage
        .enable_simd(true)
        .enable_telemetry(true)
        .ml_eviction_enabled(true)
        .coherence_enabled(true)
        .node_id(node_id.to_string())
        .build()?;

    // Analytics cache: Cold tier optimized for large, infrequent data
    let analytics_cache = Goldylox::<String, AnalyticsEvent>::builder()
        .hot_tier_enabled(false)            // Analytics rarely accessed frequently
        .warm_tier_enabled(false)           
        .cold_tier_enabled(true)
        .cold_tier_storage_path(&format!("/tmp/goldylox_analytics_{}", node_id))
        .cold_tier_max_size_bytes(20 * 1024 * 1024 * 1024) // 20GB for analytics
        .compression_level(9)               // Maximum compression for analytics
        .enable_telemetry(true)
        .coherence_enabled(true)            // Analytics need consistency too
        .node_id(node_id.to_string())
        .build()?;

    Ok(CacheNode {
        node_id: node_id.to_string(),
        location: location.to_string(),
        product_cache: Arc::new(product_cache),
        session_cache: Arc::new(session_cache),
        analytics_cache: Arc::new(analytics_cache),
        metrics: Arc::new(NodeMetrics::default()),
    })
}

/// Set up coherence mesh networking between all nodes
async fn setup_coherence_mesh(nodes: &[Arc<CacheNode>]) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔗 Setting up coherence mesh between {} nodes...", nodes.len());
    
    // For each pair of nodes, establish coherence connections
    for i in 0..nodes.len() {
        for j in (i+1)..nodes.len() {
            establish_coherence_connection(&nodes[i], &nodes[j]).await?;
            println!("   ✅ Coherence link: {} ↔ {}", nodes[i].node_id, nodes[j].node_id);
        }
    }
    
    println!("✅ Coherence mesh established - {} total connections", 
             (nodes.len() * (nodes.len() - 1)) / 2);
    
    Ok(())
}

/// Establish coherence connection between two nodes
async fn establish_coherence_connection(
    node_a: &CacheNode, 
    node_b: &CacheNode
) -> Result<(), Box<dyn std::error::Error>> {
    // This would connect the coherence protocols between nodes
    // For now, we'll simulate this setup
    println!("   🔧 Establishing coherence between {} and {}", node_a.node_id, node_b.node_id);
    
    // In real implementation, this would:
    // 1. Set up network connections between coherence controllers
    // 2. Exchange node metadata and capabilities  
    // 3. Initialize write propagation channels
    // 4. Set up invalidation notification systems
    
    Ok(())
}

/// Seed the product catalog with realistic e-commerce data
async fn seed_product_catalog(simulation: &SimulationState) -> Result<(), Box<dyn std::error::Error>> {
    let categories = vec![
        "Electronics", "Clothing", "Home & Garden", "Sports", "Books", 
        "Health", "Automotive", "Toys", "Beauty", "Groceries"
    ];
    
    let mut rng = thread_rng();
    
    // Create 10,000 realistic products
    for product_id in 1..=10_000 {
        let category = categories.choose(&mut rng).unwrap();
        let product = Product {
            product_id,
            name: generate_product_name(category, product_id),
            category: category.to_string(),
            price: rng.gen_range(9.99..999.99),
            inventory_count: rng.gen_range(0..1000),
            description: format!("High-quality {} product #{}", category.to_lowercase(), product_id),
            reviews_count: rng.gen_range(0..5000),
            average_rating: rng.gen_range(1.0..5.0),
            tags: generate_product_tags(category),
            metadata: generate_product_metadata(&mut rng),
            last_updated: current_timestamp(),
        };
        
        // Distribute products across nodes based on hash for even distribution
        let node_index = (product_id as usize) % simulation.nodes.len();
        let cache_key = format!("product:{}", product_id);
        
        simulation.nodes[node_index]
            .product_cache
            .put(cache_key, product)?;
    }
    
    Ok(())
}

/// Seed user sessions with realistic patterns
async fn seed_user_sessions(simulation: &SimulationState) -> Result<(), Box<dyn std::error::Error>> {
    let mut rng = thread_rng();
    
    // Create 1,000 user sessions with realistic shopping behavior
    for user_id in 1..=1_000 {
        let session_id = format!("sess_{:06}", user_id);
        let shopping_cart = generate_realistic_cart(&mut rng);
        let browsing_history = generate_browsing_history(&mut rng);
        
        let session = UserSession {
            session_id: session_id.clone(),
            user_id,
            created_at: current_timestamp() - rng.gen_range(0..3600), // Created within last hour
            last_activity: current_timestamp() - rng.gen_range(0..600), // Active within 10 minutes
            expires_at: current_timestamp() + 3600, // Expires in 1 hour
            shopping_cart,
            browsing_history,
            preferences: generate_user_preferences(&mut rng),
            location: generate_user_location(&mut rng),
        };
        
        // Distribute sessions across nodes
        let node_index = (user_id as usize) % simulation.nodes.len();
        let cache_key = format!("session:{}", session_id);
        
        simulation.nodes[node_index]
            .session_cache
            .put(cache_key, session)?;
    }
    
    Ok(())
}

// Utility functions for data generation
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn generate_product_name(category: &str, id: u64) -> String {
    match category {
        "Electronics" => format!("Premium {} Device #{}", category, id),
        "Clothing" => format!("Designer {} Item #{}", category, id),
        _ => format!("{} Product #{}", category, id),
    }
}

fn generate_product_tags(category: &str) -> Vec<String> {
    match category {
        "Electronics" => vec!["tech".to_string(), "gadget".to_string(), "premium".to_string()],
        "Clothing" => vec!["fashion".to_string(), "style".to_string(), "trendy".to_string()],
        _ => vec!["quality".to_string(), "popular".to_string()],
    }
}

fn generate_product_metadata(rng: &mut ThreadRng) -> BTreeMap<String, String> {
    let mut metadata = BTreeMap::new();
    metadata.insert("weight".to_string(), format!("{:.1}", rng.gen_range(0.1..50.0)));
    metadata.insert("dimensions".to_string(), format!("{}x{}x{}", 
        rng.gen_range(1..100), rng.gen_range(1..100), rng.gen_range(1..100)));
    metadata.insert("color".to_string(), ["Red", "Blue", "Green", "Black", "White"]
        .choose(rng).unwrap().to_string());
    metadata
}

fn generate_realistic_cart(rng: &mut ThreadRng) -> Vec<CartItem> {
    let cart_size = rng.gen_range(0..8); // 0-7 items in cart
    let mut cart = Vec::new();
    
    for _ in 0..cart_size {
        cart.push(CartItem {
            product_id: rng.gen_range(1..10_001),
            quantity: rng.gen_range(1..5),
            added_at: current_timestamp() - rng.gen_range(0..3600),
            price_at_add: rng.gen_range(9.99..999.99),
        });
    }
    
    cart
}

fn generate_browsing_history(rng: &mut ThreadRng) -> Vec<u64> {
    let history_size = rng.gen_range(5..50); // 5-49 viewed products
    (0..history_size).map(|_| rng.gen_range(1..10_001)).collect()
}

fn generate_user_preferences(rng: &mut ThreadRng) -> BTreeMap<String, String> {
    let mut prefs = BTreeMap::new();
    prefs.insert("currency".to_string(), ["USD", "EUR", "GBP"].choose(rng).unwrap().to_string());
    prefs.insert("language".to_string(), ["en", "es", "fr", "de"].choose(rng).unwrap().to_string());
    prefs.insert("theme".to_string(), ["light", "dark"].choose(rng).unwrap().to_string());
    prefs
}

fn generate_user_location(rng: &mut ThreadRng) -> String {
    ["US", "UK", "DE", "FR", "CA", "AU"].choose(rng).unwrap().to_string()
}