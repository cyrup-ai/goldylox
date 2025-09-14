//! Workload Generator - Generates realistic e-commerce traffic patterns
//! 
//! This module generates evolving access patterns that enable ML learning:
//! - Phase 1: Black Friday traffic (concentrated hot products)
//! - Phase 2: Regular browsing (distributed access patterns)  
//! - Phase 3: Clearance sale (different hot products)

use super::*;
use rand::prelude::*;
use rand::{Rng, thread_rng};
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use crossbeam_channel::bounded;
use tokio::sync::oneshot;
use std::time::{Duration, Instant};
use std::collections::BTreeMap;
use tokio::time::sleep;
use crate::{current_timestamp, WorkloadState, CacheNode, CartItem, AnalyticsEvent};

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
pub async fn run_traffic_evolution(workload: &WorkloadState) -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Starting 3-phase traffic evolution...\n");

    // Phase 1: Black Friday - Concentrated traffic on hot products
    println!("🛍️  PHASE 1: Black Friday Traffic (40 minutes)");
    println!("   📈 Generating concentrated access on trending products");
    println!("   🧠 ML will learn which products are HOT and promote them");
    
    let black_friday_config = WorkloadConfig {
        phase: TrafficPhase::BlackFriday,
        operations_per_second: 1000,
        read_write_ratio: 0.9, // Heavy reads during shopping
        hot_product_percentage: 0.8, // 80% traffic to 20% products
        duration_minutes: 40,
    };
    
    workload.phase.store(0, Ordering::Relaxed);
    run_workload_phase(workload, black_friday_config).await?;

    // Phase 2: Regular Browsing - Distributed access patterns
    println!("\n🏠 PHASE 2: Regular Browsing Traffic (40 minutes)");
    println!("   🔍 Generating normal browsing with distributed access");
    println!("   🧠 ML will adapt by cooling down previous hot products");
    
    let regular_config = WorkloadConfig {
        phase: TrafficPhase::RegularBrowsing,
        operations_per_second: 500,
        read_write_ratio: 0.7, // More balanced read/write
        hot_product_percentage: 0.3, // Distributed access
        duration_minutes: 40,
    };
    
    workload.phase.store(1, Ordering::Relaxed);
    run_workload_phase(workload, regular_config).await?;

    // Phase 3: Clearance Sale - New hot products + inventory updates
    println!("\n🏷️  PHASE 3: Clearance Sale Traffic (40 minutes)");
    println!("   💥 Generating clearance with NEW hot products + inventory changes");
    println!("   🧠 ML will recognize the new pattern and re-optimize tiers");
    
    let clearance_config = WorkloadConfig {
        phase: TrafficPhase::ClearanceSale,
        operations_per_second: 800,
        read_write_ratio: 0.6, // Higher writes due to inventory updates
        hot_product_percentage: 0.7, // Different set of hot products
        duration_minutes: 40,
    };
    
    workload.phase.store(2, Ordering::Relaxed);
    run_workload_phase(workload, clearance_config).await?;

    println!("\n✅ All 3 traffic phases completed - ML has adapted 3 times!");
    Ok(())
}

/// Run a single phase of the workload
async fn run_workload_phase(
    workload: &WorkloadState,
    config: WorkloadConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    
    let start_time = Instant::now();
    let phase_duration = Duration::from_secs(config.duration_minutes * 60);
    
    // Calculate timing for consistent operations per second
    let ops_per_second = config.operations_per_second;
    let sleep_duration = Duration::from_millis(1000 / ops_per_second);
    
    let mut operations_count = 0u64;
    let mut rng = thread_rng();
    
    while start_time.elapsed() < phase_duration && workload.running.load(Ordering::Relaxed) {
        // Generate and execute operation based on phase
        let operation = generate_operation(&config, &mut rng);
        let node_index = select_node_for_operation(&operation, &workload.nodes);
        
        execute_operation(&workload.nodes[node_index], operation).await?;
        
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

/// Select which node should handle the operation (implements load balancing)
fn select_node_for_operation(operation: &Operation, nodes: &[CacheNode]) -> usize {
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
            let (response_tx, response_rx) = oneshot::channel();
            let command = CacheCommand::ProductGet { key: cache_key, response: response_tx };
            
            match node.command_sender.send(command) {
                Ok(_) => {
                    match tokio::time::timeout(Duration::from_millis(100), response_rx).await {
                        Ok(Ok(_result)) => {
                            // Operation completed successfully
                        },
                        Ok(Err(_)) => {
                            return Err("Worker response channel closed".into());
                        },
                        Err(_) => {
                            return Err("Operation timeout".into());
                        }
                    }
                },
                Err(e) => {
                    return Err(format!("Failed to send command to worker: {}", e).into());
                }
            }
        },
        
        Operation::ProductUpdate(product_id) => {
            let cache_key = format!("product:{}", product_id);
            // First get the product
            let (get_response_tx, get_response_rx) = oneshot::channel();
            let get_command = CacheCommand::ProductGet { key: cache_key.clone(), response: get_response_tx };
            
            match node.command_sender.send(get_command) {
                Ok(_) => {
                    match tokio::time::timeout(Duration::from_millis(100), get_response_rx).await {
                        Ok(Ok(Some(mut product))) => {
                            // Update product
                            product.inventory_count = product.inventory_count.saturating_sub(1);
                            product.last_updated = current_timestamp();
                            
                            // Send updated product back
                            let (put_response_tx, put_response_rx) = oneshot::channel();
                            let put_command = CacheCommand::ProductPut { key: cache_key, value: product, response: put_response_tx };
                            
                            match node.command_sender.send(put_command) {
                                Ok(_) => {
                                    match tokio::time::timeout(Duration::from_millis(100), put_response_rx).await {
                                        Ok(Ok(_)) => { /* Success */ },
                                        Ok(Err(e)) => return Err(format!("Product update failed: {}", e).into()),
                                        Err(_) => return Err("Product update timeout".into()),
                                    }
                                },
                                Err(e) => return Err(format!("Failed to send update command: {}", e).into()),
                            }
                        },
                        Ok(Ok(None)) => {
                            // Product not found, skip update
                        },
                        Ok(Err(_)) => return Err("Worker response channel closed".into()),
                        Err(_) => return Err("Product get timeout".into()),
                    }
                },
                Err(e) => return Err(format!("Failed to send get command: {}", e).into()),
            }
        },
        
        Operation::SessionRead(session_id) => {
            let cache_key = format!("session:{}", session_id);
            let (response_tx, response_rx) = oneshot::channel();
            let command = CacheCommand::SessionGet { key: cache_key, response: response_tx };
            
            match node.command_sender.send(command) {
                Ok(_) => {
                    match tokio::time::timeout(Duration::from_millis(100), response_rx).await {
                        Ok(Ok(_result)) => {
                            // Operation completed successfully
                        },
                        Ok(Err(_)) => {
                            return Err("Worker response channel closed".into());
                        },
                        Err(_) => {
                            return Err("Operation timeout".into());
                        }
                    }
                },
                Err(e) => {
                    return Err(format!("Failed to send command to worker: {}", e).into());
                }
            }
        },
        
        Operation::SessionUpdate(session_id) => {
            let cache_key = format!("session:{}", session_id);
            // First get the session
            let (get_response_tx, get_response_rx) = oneshot::channel();
            let get_command = CacheCommand::SessionGet { key: cache_key.clone(), response: get_response_tx };
            
            match node.command_sender.send(get_command) {
                Ok(_) => {
                    match tokio::time::timeout(Duration::from_millis(100), get_response_rx).await {
                        Ok(Ok(Some(mut session))) => {
                            // Update session
                            session.last_activity = current_timestamp();
                            let new_item = CartItem {
                                product_id: thread_rng().gen_range(1..=10000),
                                quantity: 1,
                                added_at: current_timestamp(),
                                price_at_add: thread_rng().gen_range(9.99..999.99),
                            };
                            session.shopping_cart.push(new_item);
                            
                            // Send updated session back
                            let (put_response_tx, put_response_rx) = oneshot::channel();
                            let put_command = CacheCommand::SessionPut { key: cache_key, value: session, response: put_response_tx };
                            
                            match node.command_sender.send(put_command) {
                                Ok(_) => {
                                    match tokio::time::timeout(Duration::from_millis(100), put_response_rx).await {
                                        Ok(Ok(_)) => { /* Success */ },
                                        Ok(Err(e)) => return Err(format!("Session update failed: {}", e).into()),
                                        Err(_) => return Err("Session update timeout".into()),
                                    }
                                },
                                Err(e) => return Err(format!("Failed to send update command: {}", e).into()),
                            }
                        },
                        Ok(Ok(None)) => {
                            // Session not found, skip update
                        },
                        Ok(Err(_)) => return Err("Worker response channel closed".into()),
                        Err(_) => return Err("Session get timeout".into()),
                    }
                },
                Err(e) => return Err(format!("Failed to send get command: {}", e).into()),
            }
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
            
            let (response_tx, response_rx) = oneshot::channel();
            let command = CacheCommand::AnalyticsPut { key: cache_key, value: analytics_event, response: response_tx };
            
            match node.command_sender.send(command) {
                Ok(_) => {
                    match tokio::time::timeout(Duration::from_millis(100), response_rx).await {
                        Ok(Ok(_result)) => {
                            // Operation completed successfully
                        },
                        Ok(Err(e)) => {
                            return Err(format!("Analytics write failed: {}", e).into());
                        },
                        Err(_) => {
                            return Err("Analytics write timeout".into());
                        }
                    }
                },
                Err(e) => {
                    return Err(format!("Failed to send command to worker: {}", e).into());
                }
            }
        },
        
        Operation::BulkProductSearch(product_ids) => {
            // REAL bulk search using messaging pattern with async handling
            for product_id in product_ids {
                let cache_key = format!("product:{}", product_id);
                let (response_tx, response_rx) = oneshot::channel();
                let command = CacheCommand::ProductGet { key: cache_key, response: response_tx };
                
                match node.command_sender.send(command) {
                    Ok(_) => {
                        match tokio::time::timeout(Duration::from_millis(50), response_rx).await {
                            Ok(Ok(_result)) => {
                                // Operation completed successfully
                            },
                            Ok(Err(_)) => {
                                return Err("Worker response channel closed during bulk search".into());
                            },
                            Err(_) => {
                                return Err("Bulk search operation timeout".into());
                            }
                        }
                    },
                    Err(e) => {
                        return Err(format!("Failed to send bulk search command: {}", e).into());
                    }
                }
            }
        }
    }
    
    Ok(())
}

/// Spawn background workload generators for sustained traffic
pub async fn spawn_workload_generators(workload: &WorkloadState) -> Result<Vec<tokio::task::JoinHandle<()>>, Box<dyn std::error::Error>> {
    let mut handles = Vec::new();
    
    // Spawn background session maintenance
    let sim_clone = workload.clone_for_background();
    let handle = tokio::spawn(async move {
        background_session_maintenance(&sim_clone).await;
    });
    handles.push(handle);
    
    // Spawn analytics aggregation
    let sim_clone = workload.clone_for_background();
    let handle = tokio::spawn(async move {
        background_analytics_aggregation(&sim_clone).await;
    });
    handles.push(handle);
    
    // Spawn cache statistics reporter
    let sim_clone = workload.clone_for_background();
    let handle = tokio::spawn(async move {
        background_statistics_reporting(&sim_clone).await;
    });
    handles.push(handle);
    
    println!("✅ Spawned {} background workload generators", handles.len());
    Ok(handles)
}

/// Background task for session maintenance (expires old sessions)
async fn background_session_maintenance(workload: &WorkloadState) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    
    while workload.running.load(Ordering::Relaxed) {
        interval.tick().await;
        
        // Request session cache statistics via messaging
        for node in &workload.nodes {
            let (response_tx, response_rx) = oneshot::channel();
            let command = CacheCommand::GetStats { response: response_tx };
            
            if node.command_sender.send(command).is_ok() {
                match tokio::time::timeout(Duration::from_millis(500), response_rx).await {
                    Ok(Ok(_stats)) => {
                        // Session maintenance based on stats could be implemented here
                    },
                    Ok(Err(_)) | Err(_) => {
                        log::warn!("Failed to get stats for session maintenance");
                    }
                }
            }
        }
    }
}

/// Background analytics aggregation (generates large analytical queries)
async fn background_analytics_aggregation(workload: &WorkloadState) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    
    while workload.running.load(Ordering::Relaxed) {
        interval.tick().await;
        
        // Analytics queries via messaging pattern
        for node in &workload.nodes {
            for i in 0..10 {
                let event_key = format!("analytics:aggregation_{}_{}", current_timestamp(), i);
                let (response_tx, response_rx) = oneshot::channel();
                let command = CacheCommand::AnalyticsGet { key: event_key, response: response_tx };
                
                if node.command_sender.send(command).is_ok() {
                    // Don't block on response for background aggregation
                    tokio::spawn(async move {
                        let _ = response_rx.await;
                    });
                }
            }
        }
    }
}

/// Background statistics reporting using async messaging pattern
async fn background_statistics_reporting(workload: &WorkloadState) {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    
    while workload.running.load(Ordering::Relaxed) {
        interval.tick().await;
        
        // Use async messaging to get cache statistics
        for (i, node) in workload.nodes.iter().enumerate() {
            let (response_tx, response_rx) = oneshot::channel();
            let command = CacheCommand::GetStats { response: response_tx };
            
            match node.command_sender.send(command) {
                Ok(_) => {
                    match tokio::time::timeout(Duration::from_millis(1000), response_rx).await {
                        Ok(Ok(stats)) => {
                            println!("📊 Node {}: Cache Stats: {}", i, stats);
                        },
                        Ok(Err(_)) => {
                            eprintln!("⚠️ Node {}: Stats channel closed", i);
                        },
                        Err(_) => {
                            eprintln!("⚠️ Node {}: Stats request timeout", i);
                        }
                    }
                },
                Err(e) => {
                    eprintln!("⚠️ Node {}: Failed to send stats command: {}", i, e);
                }
            }
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

/// Helper trait for workload state cloning
impl WorkloadState {
    pub fn clone_for_background(&self) -> WorkloadState {
        WorkloadState {
            nodes: self.nodes.clone(), // Now cloning CacheNode which contains only Sender (cheaply cloneable)
            start_time: self.start_time,
            phase: AtomicU64::new(self.phase.load(Ordering::Relaxed)),
            running: AtomicBool::new(self.running.load(Ordering::Relaxed)),
        }
    }
}