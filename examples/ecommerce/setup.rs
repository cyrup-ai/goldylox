//! Cache node setup with worker threads
//! 
//! This module creates cache nodes with dedicated worker threads that own cache instances.
//! All cache operations are performed through crossbeam messaging.

use super::types::*;
use crossbeam_channel::bounded;
use std::thread;
use goldylox::prelude::*;

/// Setup cache nodes with worker threads
pub fn setup_cache_nodes(node_count: usize) -> Result<Vec<CacheNode>, Box<dyn std::error::Error>> {
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
        
        thread::Builder::new()
            .name(worker_name)
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    worker.run();
                }));
                
                if let Err(e) = result {
                    eprintln!("Worker {} panicked: {:?}", panic_handler_name, e);
                    // In production, you might want to restart the worker here
                }
            })?;
        
        // Create node with messaging interface
        let node = CacheNode {
            node_id,
            location,
            command_sender,
        };
        
        nodes.push(node);
    }
    
    println!("✅ Created {} cache nodes with worker threads", node_count);
    Ok(nodes)
}

/// Shutdown all cache nodes gracefully
pub fn shutdown_cache_nodes(nodes: &[CacheNode]) {
    for (i, node) in nodes.iter().enumerate() {
        let shutdown_command = CacheCommand::Shutdown;
        if let Err(e) = node.command_sender.send(shutdown_command) {
            eprintln!("Failed to send shutdown command to node {}: {}", i, e);
        }
    }
    println!("✅ Sent shutdown commands to all cache nodes");
}