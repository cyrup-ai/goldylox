//! Cache node setup with worker threads
//!
//! This module creates cache nodes with dedicated worker threads that own cache instances.
//! All cache operations are performed through crossbeam messaging.

use super::types::*;
use crossbeam_channel::bounded;
use goldylox::prelude::*;
use std::thread;

/// Setup cache nodes with worker threads (currently unused)
#[allow(dead_code)]
#[cfg(feature = "worker_based_cache")]
pub async fn setup_cache_nodes(node_count: usize) -> Result<Vec<CacheNode>, Box<dyn std::error::Error>> {
    let mut nodes = Vec::new();

    for i in 0..node_count {
        let node_id = format!("node_{:03}", i);
        let location = format!("datacenter_{}", i % 3);

        // Create command channel
        let (command_sender, command_receiver) = bounded::<CacheCommand>(1000);

        // Spawn worker thread with panic recovery
        let worker = CacheWorker::new(command_receiver)?;
        let worker_name = format!("cache-worker-{}", node_id);
        let panic_handler_name = worker_name.clone();

        thread::Builder::new().name(worker_name).spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                worker.run();
            }));

            if let Err(e) = result {
                eprintln!("Worker {} terminated with unrecoverable panic: {:?}", panic_handler_name, e);
                eprintln!("This indicates a critical bug in CacheWorker::run() itself");
                // Note: Individual command panics are handled inside worker.run()
            }
        })?;

        // Create cache instances for this node
        let product_cache = GoldyloxBuilder::<String, Product>::new()
            .hot_tier_max_entries(1000)
            .warm_tier_max_entries(5000)
            .cold_tier_base_dir(format!("./cache/node_{}/products", i))
            .build().await?;

        let session_cache = GoldyloxBuilder::<String, UserSession>::new()
            .hot_tier_max_entries(500)
            .warm_tier_max_entries(2500)
            .cold_tier_base_dir(format!("./cache/node_{}/sessions", i))
            .build().await?;

        let analytics_cache = GoldyloxBuilder::<String, AnalyticsEvent>::new()
            .hot_tier_max_entries(1500)
            .warm_tier_max_entries(7500)
            .cold_tier_base_dir(format!("./cache/node_{}/analytics", i))
            .build().await?;

        // Create node with messaging interface
        let node = CacheNode {
            node_id,
            location,
            command_sender,
            product_cache,
            session_cache,
            analytics_cache,
        };

        nodes.push(node);
    }

    println!("✅ Created {} cache nodes with worker threads", node_count);
    Ok(nodes)
}

/// Setup distributed cache nodes with actual cache instances
pub async fn setup_distributed_nodes() -> Result<WorkloadState, Box<dyn std::error::Error>> {
    // Create cache instances using the builders
    let product_cache = GoldyloxBuilder::<String, Product>::new()
        .hot_tier_max_entries(10000)
        .warm_tier_max_entries(50000)
        .cold_tier_base_dir("./cache/products")
        .build().await?;

    let session_cache = GoldyloxBuilder::<String, UserSession>::new()
        .hot_tier_max_entries(5000)
        .warm_tier_max_entries(25000)
        .cold_tier_base_dir("./cache/sessions")
        .build().await?;

    let analytics_cache = GoldyloxBuilder::<String, AnalyticsEvent>::new()
        .hot_tier_max_entries(15000)
        .warm_tier_max_entries(75000)
        .cold_tier_base_dir("./cache/analytics")
        .build().await?;

    // Create command channel (unused but required by CacheNode structure)
    let (command_sender, _command_receiver) = bounded::<CacheCommand>(1000);

    // Create cache node with actual cache instances (direct access, no worker)
    let node = CacheNode {
        node_id: "node-1".to_string(),
        location: "us-east-1".to_string(),
        command_sender,
        product_cache,
        session_cache,
        analytics_cache,
    };

    let nodes = vec![node];

    Ok(WorkloadState {
        nodes,
        start_time: std::time::Instant::now(),
        phase: std::sync::atomic::AtomicU64::new(0),
        running: std::sync::atomic::AtomicBool::new(true),
    })
}

/// Shutdown all cache nodes gracefully
#[allow(dead_code)]
pub fn shutdown_cache_nodes(nodes: &[CacheNode]) {
    for (i, node) in nodes.iter().enumerate() {
        let shutdown_command = CacheCommand::Shutdown;
        if let Err(e) = node.command_sender.send(shutdown_command) {
            eprintln!("Failed to send shutdown command to node {}: {}", i, e);
        }
    }
    println!("✅ Sent shutdown commands to all cache nodes");
}
