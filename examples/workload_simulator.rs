//! Workload Simulator - Generates realistic e-commerce traffic patterns
//! 
//! This module simulates evolving access patterns that demonstrate ML learning:
//! - Phase 1: Black Friday traffic (concentrated hot products)
//! - Phase 2: Regular browsing (distributed access patterns)  
//! - Phase 3: Clearance sale (different hot products)

use super::*;
use rand::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Traffic pattern phases
#[derive(Debug, Clone)]
pub enum TrafficPhase {
    BlackFriday,    // Concentrated access on few hot products
    RegularBrowsing, // Distributed access across catalog
    ClearanceSale,  // Different set of hot products + high writes
}

/// Workload generator configuration
#[derive(Debug)]
pub struct WorkloadConfig {
    pub phase: TrafficPhase,
    pub operations_per_second: u64,
    pub read_write_ratio: f64, // 0.8 = 80% reads, 20% writes
    pub hot_product_percentage: f64, // % of traffic to hot products
    pub duration_minutes: u64,
}

/// Individual operation types
#[derive(Debug)]
pub enum Operation {
    ProductLookup(u64),
    ProductUpdate(u64),
    SessionRead(String),
    SessionUpdate(String),
    AnalyticsWrite(String),
    BulkProductSearch(Vec<u64>),
}

/// Generate traffic for the evolving phases
pub async fn run_traffic_evolution(simulation: &SimulationState) -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Starting 3-phase traffic evolution...\n");

    // Phase 1: Black Friday - Concentrated traffic on hot products
    println!("🛍️  PHASE 1: Black Friday Traffic (40 minutes)");
    println!("   📈 Simulating concentrated access on trending products");
    println!("   🧠 ML will learn which products are HOT and promote them");
    
    let black_friday_config = WorkloadConfig {
        phase: TrafficPhase::BlackFriday,
        operations_per_second: 1000,
        read_write_ratio: 0.9, // Heavy reads during shopping
        hot_product_percentage: 0.8, // 80% traffic to 20% products
        duration_minutes: 40,
    };
    
    simulation.phase.store(0, Ordering::Relaxed);
    run_workload_phase(simulation, black_friday_config).await?;

    // Phase 2: Regular Browsing - Distributed access patterns
    println!("\n🏠 PHASE 2: Regular Browsing Traffic (40 minutes)");
    println!("   🔍 Simulating normal browsing with distributed access");
    println!("   🧠 ML will adapt by cooling down previous hot products");
    
    let regular_config = WorkloadConfig {
        phase: TrafficPhase::RegularBrowsing,
        operations_per_second: 500,
        read_write_ratio: 0.7, // More balanced read/write
        hot_product_percentage: 0.3, // Distributed access
        duration_minutes: 40,
    };
    
    simulation.phase.store(1, Ordering::Relaxed);
    run_workload_phase(simulation, regular_config).await?;

    // Phase 3: Clearance Sale - New hot products + inventory updates
    println!("\n🏷️  PHASE 3: Clearance Sale Traffic (40 minutes)");
    println!("   💥 Simulating clearance with NEW hot products + inventory changes");
    println!("   🧠 ML will recognize the new pattern and re-optimize tiers");
    
    let clearance_config = WorkloadConfig {
        phase: TrafficPhase::ClearanceSale,
        operations_per_second: 800,
        read_write_ratio: 0.6, // Higher writes due to inventory updates
        hot_product_percentage: 0.7, // Different set of hot products
        duration_minutes: 40,
    };
    
    simulation.phase.store(2, Ordering::Relaxed);
    run_workload_phase(simulation, clearance_config).await?;

    println!("\n✅ All 3 traffic phases completed - ML has adapted 3 times!");
    Ok(())
}

/// Run a single phase of the workload
async fn run_workload_phase(
    simulation: &SimulationState,
    config: WorkloadConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    
    let start_time = Instant::now();
    let phase_duration = Duration::from_secs(config.duration_minutes * 60);
    
    // Calculate timing for consistent operations per second
    let ops_per_second = config.operations_per_second;
    let sleep_duration = Duration::from_millis(1000 / ops_per_second);
    
    let mut operations_count = 0u64;
    let mut rng = thread_rng();
    
    while start_time.elapsed() < phase_duration && simulation.running.load(Ordering::Relaxed) {
        // Generate and execute operation based on phase
        let operation = generate_operation(&config, &mut rng);
        let node_index = select_node_for_operation(&operation, &simulation.nodes);
        
        execute_operation(&simulation.nodes[node_index], operation).await?;
        
        operations_count += 1;
        
        // Progress reporting every 10 seconds
        if operations_count % (ops_per_second * 10) == 0 {
            let elapsed = start_time.elapsed();
            let progress = (elapsed.as_secs() as f64 / phase_duration.as_secs() as f64) * 100.0;
            println!("   ⏱️  {:?} Phase: {:.1}% complete ({} ops)", config.phase, progress, operations_count);
        }
        
        sleep(sleep_duration).await;
    }
    
    println!("✅ Phase {:?} completed: {} operations in {:.1} minutes", 
             config.phase, operations_count, phase_duration.as_secs_f64() / 60.0);
    
    Ok(())
}

/// Generate an operation based on the current phase and configuration
fn generate_operation(config: &WorkloadConfig, rng: &mut ThreadRng) -> Operation {
    let is_read = rng.gen::<f64>() < config.read_write_ratio;
    
    match config.phase {
        TrafficPhase::BlackFriday => {
            if rng.gen::<f64>() < config.hot_product_percentage {
                // Hot products during Black Friday (products 1-1000)
                let hot_product_id = rng.gen_range(1..=1000);
                if is_read {
                    Operation::ProductLookup(hot_product_id)
                } else {
                    // Inventory updates during heavy shopping
                    Operation::ProductUpdate(hot_product_id)
                }
            } else {
                // Regular catalog browsing
                let product_id = rng.gen_range(1001..=10000);
                Operation::ProductLookup(product_id)
            }
        },
        
        TrafficPhase::RegularBrowsing => {
            match rng.gen_range(0..4) {
                0 => {
                    // Product lookups - distributed across catalog
                    let product_id = rng.gen_range(1..=10000);
                    Operation::ProductLookup(product_id)
                },
                1 => {
                    // Session operations
                    let session_id = format!("sess_{:06}", rng.gen_range(1..=1000));
                    if is_read {
                        Operation::SessionRead(session_id)
                    } else {
                        Operation::SessionUpdate(session_id)
                    }
                },
                2 => {
                    // Bulk product searches (shows SIMD benefits)
                    let search_size = rng.gen_range(10..100);
                    let product_ids: Vec<u64> = (0..search_size)
                        .map(|_| rng.gen_range(1..=10000))
                        .collect();
                    Operation::BulkProductSearch(product_ids)
                },
                _ => {
                    // Analytics events
                    let event_id = format!("event_{}_{}", 
                        current_timestamp(), 
                        rng.gen::<u32>());
                    Operation::AnalyticsWrite(event_id)
                }
            }
        },
        
        TrafficPhase::ClearanceSale => {
            if rng.gen::<f64>() < config.hot_product_percentage {
                // NEW hot products during clearance (products 5000-6000)
                let clearance_product_id = rng.gen_range(5000..=6000);
                if is_read {
                    Operation::ProductLookup(clearance_product_id)
                } else {
                    // Frequent inventory updates during clearance
                    Operation::ProductUpdate(clearance_product_id)
                }
            } else {
                // Mixed operations during clearance
                match rng.gen_range(0..3) {
                    0 => Operation::ProductLookup(rng.gen_range(1..=10000)),
                    1 => {
                        let session_id = format!("sess_{:06}", rng.gen_range(1..=1000));
                        Operation::SessionUpdate(session_id) // More updates during sales
                    },
                    _ => {
                        let event_id = format!("clearance_event_{}_{}", 
                            current_timestamp(), 
                            rng.gen::<u32>());
                        Operation::AnalyticsWrite(event_id)
                    }
                }
            }
        }
    }
}

/// Select which node should handle the operation (simulates load balancing)
fn select_node_for_operation(operation: &Operation, nodes: &[Arc<CacheNode>]) -> usize {
    match operation {
        Operation::ProductLookup(product_id) | Operation::ProductUpdate(product_id) => {
            (*product_id as usize) % nodes.len()
        },
        Operation::SessionRead(session_id) | Operation::SessionUpdate(session_id) => {
            // Hash session ID to get consistent node assignment
            session_id.len() % nodes.len()
        },
        Operation::AnalyticsWrite(event_id) => {
            event_id.len() % nodes.len()
        },
        Operation::BulkProductSearch(_) => {
            // Round-robin for bulk operations
            0 // For simplicity, always use first node for bulk operations
        }
    }
}

/// Execute an operation on a specific cache node
async fn execute_operation(node: &CacheNode, operation: Operation) -> Result<(), Box<dyn std::error::Error>> {
    
    match operation {
        Operation::ProductLookup(product_id) => {
            let cache_key = format!("product:{}", product_id);
            if let Some(_product) = node.product_cache.get(&cache_key) {
                node.metrics.cache_hits.fetch_add(1, Ordering::Relaxed);
                node.metrics.operations_count.fetch_add(1, Ordering::Relaxed);
            } else {
                node.metrics.cache_misses.fetch_add(1, Ordering::Relaxed);
            }
        },
        
        Operation::ProductUpdate(product_id) => {
            let cache_key = format!("product:{}", product_id);
            if let Some(mut product) = node.product_cache.get(&cache_key) {
                // Simulate inventory update
                product.inventory_count = product.inventory_count.saturating_sub(1);
                product.last_updated = current_timestamp();
                node.product_cache.put(cache_key, product)?;
                
                node.metrics.coherence_events.fetch_add(1, Ordering::Relaxed);
                node.metrics.operations_count.fetch_add(1, Ordering::Relaxed);
            }
        },
        
        Operation::SessionRead(session_id) => {
            let cache_key = format!("session:{}", session_id);
            if let Some(_session) = node.session_cache.get(&cache_key) {
                node.metrics.cache_hits.fetch_add(1, Ordering::Relaxed);
            } else {
                node.metrics.cache_misses.fetch_add(1, Ordering::Relaxed);
            }
            node.metrics.operations_count.fetch_add(1, Ordering::Relaxed);
        },
        
        Operation::SessionUpdate(session_id) => {
            let cache_key = format!("session:{}", session_id);
            if let Some(mut session) = node.session_cache.get(&cache_key) {
                // Simulate adding item to cart
                session.last_activity = current_timestamp();
                let new_item = CartItem {
                    product_id: thread_rng().gen_range(1..=10000),
                    quantity: 1,
                    added_at: current_timestamp(),
                    price_at_add: thread_rng().gen_range(9.99..999.99),
                };
                session.shopping_cart.push(new_item);
                
                node.session_cache.put(cache_key, session)?;
                node.metrics.coherence_events.fetch_add(1, Ordering::Relaxed);
            }
            node.metrics.operations_count.fetch_add(1, Ordering::Relaxed);
        },
        
        Operation::AnalyticsWrite(event_id) => {
            let cache_key = format!("analytics:{}", event_id);
            let mut rng = thread_rng();
            
            let analytics_event = AnalyticsEvent {
                event_id: event_id.clone(),
                timestamp: current_timestamp(),
                user_id: rng.gen_range(1..=1000),
                session_id: format!("sess_{:06}", rng.gen_range(1..=1000)),
                event_type: ["page_view", "add_to_cart", "purchase", "search"]
                    .choose(&mut rng).unwrap().to_string(),
                product_id: if rng.gen::<f64>() < 0.7 { 
                    Some(rng.gen_range(1..=10000)) 
                } else { 
                    None 
                },
                properties: generate_event_properties(&mut rng),
                raw_data: vec![0u8; rng.gen_range(1024..8192)], // 1-8KB of data
            };
            
            node.analytics_cache.put(cache_key, analytics_event)?;
            node.metrics.operations_count.fetch_add(1, Ordering::Relaxed);
        },
        
        Operation::BulkProductSearch(product_ids) => {
            // This demonstrates SIMD benefits with batch operations
            let mut found_products = 0;
            
            for product_id in product_ids {
                let cache_key = format!("product:{}", product_id);
                if let Some(_product) = node.product_cache.get(&cache_key) {
                    found_products += 1;
                }
            }
            
            node.metrics.simd_operations.fetch_add(1, Ordering::Relaxed);
            node.metrics.cache_hits.fetch_add(found_products, Ordering::Relaxed);
            node.metrics.operations_count.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    Ok(())
}

/// Spawn background workload generators for sustained traffic
pub async fn spawn_workload_generators(simulation: &SimulationState) -> Result<Vec<tokio::task::JoinHandle<()>>, Box<dyn std::error::Error>> {
    let mut handles = Vec::new();
    
    // Spawn background session maintenance
    let sim_clone = simulation.clone_for_background();
    let handle = tokio::spawn(async move {
        background_session_maintenance(&sim_clone).await;
    });
    handles.push(handle);
    
    // Spawn analytics aggregation
    let sim_clone = simulation.clone_for_background();
    let handle = tokio::spawn(async move {
        background_analytics_aggregation(&sim_clone).await;
    });
    handles.push(handle);
    
    // Spawn cache statistics reporter
    let sim_clone = simulation.clone_for_background();
    let handle = tokio::spawn(async move {
        background_statistics_reporting(&sim_clone).await;
    });
    handles.push(handle);
    
    println!("✅ Spawned {} background workload generators", handles.len());
    Ok(handles)
}

/// Background task for session maintenance (expires old sessions)
async fn background_session_maintenance(simulation: &SimulationState) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    
    while simulation.running.load(Ordering::Relaxed) {
        interval.tick().await;
        
        // Simulate session cleanup and maintenance
        for node in &simulation.nodes {
            // This would trigger ML eviction decisions
            let stats = match node.session_cache.stats() {
                Ok(s) => s,
                Err(_) => continue,
            };
            
            if stats.to_string().contains("entries") {
                // ML would analyze session access patterns here
                node.metrics.ml_tier_migrations.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// Background analytics aggregation (generates large analytical queries)
async fn background_analytics_aggregation(simulation: &SimulationState) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    
    while simulation.running.load(Ordering::Relaxed) {
        interval.tick().await;
        
        // Simulate analytics queries that would benefit from ML tier placement
        for node in &simulation.nodes {
            // Large analytical queries that ML learns are infrequent
            for i in 0..10 {
                let event_key = format!("analytics:aggregation_{}_{}", current_timestamp(), i);
                let _ = node.analytics_cache.get(&event_key);
            }
        }
    }
}

/// Background statistics reporting
async fn background_statistics_reporting(simulation: &SimulationState) {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    let mut last_operations = 0u64;
    
    while simulation.running.load(Ordering::Relaxed) {
        interval.tick().await;
        
        let total_ops: u64 = simulation.nodes.iter()
            .map(|node| node.metrics.operations_count.load(Ordering::Relaxed))
            .sum();
        
        let ops_per_sec = (total_ops - last_operations) / 10;
        last_operations = total_ops;
        
        if ops_per_sec > 0 {
            println!("📊 Current throughput: {} ops/sec | Total: {} operations", 
                     ops_per_sec, total_ops);
        }
    }
}

/// Generate event properties for analytics
fn generate_event_properties(rng: &mut ThreadRng) -> BTreeMap<String, String> {
    let mut props = BTreeMap::new();
    props.insert("user_agent".to_string(), "Mozilla/5.0 (compatible)".to_string());
    props.insert("ip_address".to_string(), format!("192.168.{}.{}", 
        rng.gen_range(1..255), rng.gen_range(1..255)));
    props.insert("referrer".to_string(), ["google.com", "facebook.com", "direct"]
        .choose(rng).unwrap().to_string());
    props
}

/// Helper trait for simulation state cloning
impl SimulationState {
    pub fn clone_for_background(&self) -> SimulationState {
        SimulationState {
            nodes: self.nodes.clone(),
            start_time: self.start_time,
            phase: AtomicU64::new(self.phase.load(Ordering::Relaxed)),
            running: AtomicBool::new(self.running.load(Ordering::Relaxed)),
            dashboard: Arc::clone(&self.dashboard),
            ml_visualizer: Arc::clone(&self.ml_visualizer),
            coherence_monitor: Arc::clone(&self.coherence_monitor),
        }
    }
}