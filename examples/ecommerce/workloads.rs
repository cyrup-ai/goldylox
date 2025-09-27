//! E-commerce workload generation functions
//!
//! This module contains the three main workload patterns:
//! Black Friday rush, regular browsing, and clearance sale patterns.

use crate::ecommerce::types::*;
use rand::{Rng, rng};
use std::collections::BTreeMap;
use std::thread;
use std::time::Instant;

/// REAL Black Friday rush - massive concurrent cache operations
pub fn generate_black_friday_rush(
    workload: &WorkloadState,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔥 Starting Black Friday rush with REAL cache operations...");
    let start = Instant::now();

    // Spawn multiple threads for concurrent cache access
    let mut handles = vec![];

    // Hot product access pattern - many threads hitting popular products
    for thread_id in 0..8 {
        let nodes_clone = workload.nodes.clone();
        let handle = thread::spawn(move || {
            let mut rng = rng();

            // Each thread performs 1000 REAL cache operations
            for _ in 0..1000 {
                let node_idx = thread_id % nodes_clone.len();

                // 80% chance to access hot products (1-100)
                let product_id = if rng.random::<f32>() < 0.8 {
                    rng.random_range(1..=100)
                } else {
                    rng.random_range(1..=10000)
                };

                let cache_key = format!("product:{}", product_id);

                // ACTUALLY GET FROM CACHE - this triggers ML eviction algorithms
                if let Some(product) = nodes_clone[node_idx].product_cache.get(&cache_key) {
                    // Cache hit - update inventory using compare_and_swap
                    let original_product = product.clone();
                    let mut updated_product = product;
                    if updated_product.inventory_count > 0 {
                        updated_product.inventory_count -= 1;
                        updated_product.last_updated = current_timestamp();

                        // REAL atomic compare_and_swap operation
                        let _ = nodes_clone[node_idx].product_cache.compare_and_swap(
                            cache_key,
                            original_product,
                            updated_product,
                        );
                    }
                }

                // REAL processing time delay
                thread::sleep(std::time::Duration::from_micros(100));
            }
        });
        handles.push(handle);
    }
    // Session management threads - REAL session cache operations
    for thread_id in 0..4 {
        let nodes_clone = workload.nodes.clone();
        let handle = thread::spawn(move || {
            let mut rng = rng();

            for _ in 0..500 {
                let node_idx = thread_id % nodes_clone.len();
                let user_id = rng.random_range(1..=1000);
                let session_key = format!("session:sess_{:06}", user_id);

                // REAL get_or_insert operation - creates session if not exists
                let _result =
                    nodes_clone[node_idx]
                        .session_cache
                        .get_or_insert(session_key.clone(), || UserSession {
                            session_id: format!("sess_{:06}", user_id),
                            user_id,
                            created_at: current_timestamp(),
                            last_activity: current_timestamp(),
                            expires_at: current_timestamp() + 3600,
                            shopping_cart: vec![],
                            browsing_history: vec![],
                            preferences: BTreeMap::new(),
                            location: "US".to_string(),
                        });
            }
        });
        handles.push(handle);
    }

    // Wait for all real operations to complete
    for handle in handles {
        handle.join().unwrap();
    }

    let duration = start.elapsed();
    println!("✅ Black Friday rush completed in {:?}", duration);

    Ok(())
}
/// REAL regular browsing patterns with analytics
pub fn generate_regular_browsing(
    workload: &WorkloadState,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🛒 Starting regular browsing with REAL cache operations...");
    let start = Instant::now();

    let mut handles = vec![];

    // Browse products with realistic patterns
    for thread_id in 0..4 {
        let nodes_clone = workload.nodes.clone();
        let handle = thread::spawn(move || {
            let mut rng = rng();

            for session in 0..200 {
                let node_idx = thread_id % nodes_clone.len();

                // Generate user browsing session - REAL cache gets
                for _ in 0..rng.random_range(5..20) {
                    let product_id = rng.random_range(1..=10000);
                    let cache_key = format!("product:{}", product_id);

                    // REAL cache operation - ML algorithms learning access patterns
                    if nodes_clone[node_idx]
                        .product_cache
                        .get(&cache_key)
                        .is_some()
                    {
                        // Generate analytics event and ACTUALLY cache it
                        let event_id = format!("event_{}_{}", thread_id, session);
                        let analytics_event = AnalyticsEvent {
                            event_id: event_id.clone(),
                            timestamp: current_timestamp(),
                            user_id: rng.random_range(1..=1000),
                            session_id: format!("sess_{}", session),
                            event_type: "product_view".to_string(),
                            product_id: Some(product_id),
                            properties: BTreeMap::new(),
                            raw_data: vec![0u8; rng.random_range(100..1000)], // Variable size data
                        };

                        // REAL put operation into analytics cache
                        let _ = nodes_clone[node_idx]
                            .analytics_cache
                            .put(event_id, analytics_event);
                    }
                }

                // Small delay between browsing sessions
                thread::sleep(std::time::Duration::from_millis(10));
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let duration = start.elapsed();
    println!("✅ Regular browsing completed in {:?}", duration);

    Ok(())
}
/// REAL clearance sale with mixed access patterns
pub fn generate_clearance_sale(workload: &WorkloadState) -> Result<(), Box<dyn std::error::Error>> {
    println!("💸 Starting clearance sale with REAL cache operations...");
    let start = Instant::now();

    let mut handles = vec![];

    // Clearance sale access pattern - mix of old and new products
    for thread_id in 0..6 {
        let nodes_clone = workload.nodes.clone();
        let handle = thread::spawn(move || {
            let mut rng = rng();

            for _ in 0..800 {
                let node_idx = thread_id % nodes_clone.len();

                // 60% clearance items (high product IDs), 40% regular items
                let product_id = if rng.random::<f32>() < 0.6 {
                    rng.random_range(8000..=10000) // Clearance items
                } else {
                    rng.random_range(1..=2000) // Regular items
                };

                let cache_key = format!("product:{}", product_id);

                // REAL cache operations with price updates
                match nodes_clone[node_idx].product_cache.get(&cache_key) {
                    Some(mut product) => {
                        // Apply clearance discount - REAL data modification
                        if product.price > 50.0 {
                            product.price *= 0.7; // 30% off
                            product.last_updated = current_timestamp();

                            // REAL put operation to update cache
                            let _ = nodes_clone[node_idx].product_cache.put(cache_key, product);
                        }
                    }
                    None => {
                        // Cache miss - no action needed, real cache tracks this
                    }
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let duration = start.elapsed();
    println!("✅ Clearance sale completed in {:?}", duration);

    Ok(())
}
