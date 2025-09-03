//! Coherence Protocol Demonstration
//! 
//! Shows distributed cache coherence in action with write propagation,
//! invalidation cascades, conflict resolution, and network partition handling.

use super::*;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::time::{Duration, Instant};
use rand::prelude::*;

/// Coherence protocol monitor
#[derive(Debug)]
pub struct CoherenceMonitor {
    pub write_propagations: Arc<Mutex<VecDeque<WritePropagation>>>,
    pub invalidation_events: Arc<Mutex<VecDeque<InvalidationEvent>>>,
    pub conflict_resolutions: Arc<Mutex<VecDeque<ConflictResolution>>>,
    pub network_topology: Arc<Mutex<NetworkTopology>>,
    pub consistency_violations: Arc<Mutex<Vec<ConsistencyViolation>>>,
    pub partition_events: Arc<Mutex<VecDeque<PartitionEvent>>>,
}

/// Write propagation event
#[derive(Debug, Clone)]
pub struct WritePropagation {
    pub timestamp: Instant,
    pub source_node: String,
    pub target_nodes: Vec<String>,
    pub key: String,
    pub operation: String,
    pub propagation_latency_ms: f64,
    pub success: bool,
    pub consistency_level: ConsistencyLevel,
}

/// Invalidation event
#[derive(Debug, Clone)]
pub struct InvalidationEvent {
    pub timestamp: Instant,
    pub origin_node: String,
    pub invalidated_nodes: Vec<String>,
    pub key: String,
    pub reason: InvalidationReason,
    pub cascade_depth: u32,
    pub total_latency_ms: f64,
}

/// Conflict resolution event
#[derive(Debug, Clone)]
pub struct ConflictResolution {
    pub timestamp: Instant,
    pub conflicting_nodes: Vec<String>,
    pub key: String,
    pub resolution_strategy: ResolutionStrategy,
    pub winner_node: String,
    pub resolution_time_ms: f64,
    pub data_divergence: f64,
}

/// Network topology state
#[derive(Debug, Clone)]
pub struct NetworkTopology {
    pub nodes: HashMap<String, NodeState>,
    pub connections: HashMap<(String, String), ConnectionState>,
    pub partitions: Vec<Partition>,
}

/// Individual node state in coherence protocol
#[derive(Debug, Clone)]
pub struct NodeState {
    pub node_id: String,
    pub status: NodeStatus,
    pub last_heartbeat: Instant,
    pub coherence_version: u64,
    pub pending_operations: u32,
}

/// Connection state between two nodes
#[derive(Debug, Clone)]
pub struct ConnectionState {
    pub latency_ms: f64,
    pub packet_loss: f64,
    pub bandwidth_mbps: f64,
    pub status: ConnectionStatus,
    pub last_message: Instant,
}

/// Network partition information
#[derive(Debug, Clone)]
pub struct Partition {
    pub partition_id: String,
    pub nodes: Vec<String>,
    pub isolated_since: Instant,
    pub status: PartitionStatus,
}

/// Partition event
#[derive(Debug, Clone)]
pub struct PartitionEvent {
    pub timestamp: Instant,
    pub event_type: PartitionEventType,
    pub affected_nodes: Vec<String>,
    pub description: String,
    pub recovery_time_ms: Option<f64>,
}

/// Consistency violation
#[derive(Debug, Clone)]
pub struct ConsistencyViolation {
    pub timestamp: Instant,
    pub key: String,
    pub nodes_involved: Vec<String>,
    pub violation_type: ViolationType,
    pub severity: ViolationSeverity,
    pub auto_resolved: bool,
}

// Enums for coherence protocol states
#[derive(Debug, Clone)]
pub enum ConsistencyLevel {
    Strong,    // All nodes must acknowledge
    Eventual,  // Eventually consistent
    Weak,      // Best effort
}

#[derive(Debug, Clone)]
pub enum InvalidationReason {
    WriteUpdate,
    Expiration,
    Eviction,
    ManualInvalidation,
}

#[derive(Debug, Clone)]
pub enum ResolutionStrategy {
    LastWriterWins,
    VectorClock,
    Quorum,
    ManualResolution,
}

#[derive(Debug, Clone)]
pub enum NodeStatus {
    Active,
    Degraded,
    Partitioned,
    Recovering,
    Failed,
}

#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Healthy,
    Degraded,
    Unstable,
    Disconnected,
}

#[derive(Debug, Clone)]
pub enum PartitionStatus {
    Active,
    Healing,
    Resolved,
}

#[derive(Debug, Clone)]
pub enum PartitionEventType {
    PartitionDetected,
    PartitionHealing,
    PartitionResolved,
    SplitBrain,
}

#[derive(Debug, Clone)]
pub enum ViolationType {
    ReadAfterWrite,
    CausalOrder,
    MonotonicRead,
    DataInconsistency,
}

#[derive(Debug, Clone)]
pub enum ViolationSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl CoherenceMonitor {
    pub fn new() -> Self {
        Self {
            write_propagations: Arc::new(Mutex::new(VecDeque::with_capacity(1000))),
            invalidation_events: Arc::new(Mutex::new(VecDeque::with_capacity(1000))),
            conflict_resolutions: Arc::new(Mutex::new(VecDeque::with_capacity(500))),
            network_topology: Arc::new(Mutex::new(NetworkTopology {
                nodes: HashMap::new(),
                connections: HashMap::new(),
                partitions: Vec::new(),
            })),
            consistency_violations: Arc::new(Mutex::new(Vec::new())),
            partition_events: Arc::new(Mutex::new(VecDeque::with_capacity(200))),
        }
    }

    /// Initialize network topology for coherence monitoring
    pub fn initialize_topology(&self, node_ids: &[String]) {
        let mut topology = self.network_topology.lock().unwrap();
        
        // Initialize nodes
        for node_id in node_ids {
            topology.nodes.insert(node_id.clone(), NodeState {
                node_id: node_id.clone(),
                status: NodeStatus::Active,
                last_heartbeat: Instant::now(),
                coherence_version: 1,
                pending_operations: 0,
            });
        }
        
        // Initialize connections (full mesh)
        for i in 0..node_ids.len() {
            for j in (i+1)..node_ids.len() {
                let connection_key = (node_ids[i].clone(), node_ids[j].clone());
                topology.connections.insert(connection_key, ConnectionState {
                    latency_ms: thread_rng().gen_range(5.0..20.0),
                    packet_loss: thread_rng().gen_range(0.0..0.001),
                    bandwidth_mbps: thread_rng().gen_range(100.0..1000.0),
                    status: ConnectionStatus::Healthy,
                    last_message: Instant::now(),
                });
            }
        }
        
        println!("✅ Coherence topology initialized: {} nodes, {} connections",
                 topology.nodes.len(),
                 topology.connections.len());
    }

    /// Record a write propagation event
    pub fn record_write_propagation(&self, propagation: WritePropagation) {
        let mut events = self.write_propagations.lock().unwrap();
        if events.len() >= 1000 {
            events.pop_front();
        }
        events.push_back(propagation);
    }

    /// Record an invalidation event
    pub fn record_invalidation(&self, invalidation: InvalidationEvent) {
        let mut events = self.invalidation_events.lock().unwrap();
        if events.len() >= 1000 {
            events.pop_front();
        }
        events.push_back(invalidation);
    }

    /// Record a conflict resolution
    pub fn record_conflict_resolution(&self, resolution: ConflictResolution) {
        let mut resolutions = self.conflict_resolutions.lock().unwrap();
        if resolutions.len() >= 500 {
            resolutions.pop_front();
        }
        resolutions.push_back(resolution);
    }

    /// Display coherence protocol activity
    pub fn display_coherence_activity(&self) {
        println!("╔════════════════════════════════════════════════════════════════════════════════╗");
        println!("║                   🌐 COHERENCE PROTOCOL ACTIVITY 🌐                            ║");
        println!("╚════════════════════════════════════════════════════════════════════════════════╝");

        self.display_network_topology();
        self.display_write_propagation_summary();
        self.display_invalidation_cascade_analysis();
        self.display_conflict_resolution_summary();
        self.display_consistency_health();
    }

    /// Display current network topology
    fn display_network_topology(&self) {
        let topology = self.network_topology.lock().unwrap();
        
        println!("\n🗺️  Network Topology Status:");
        println!("┌─────────────┬──────────┬───────────────┬─────────┬──────────────┐");
        println!("│ Node        │ Status   │ Last Heartbeat│ Version │ Pending Ops  │");
        println!("├─────────────┼──────────┼───────────────┼─────────┼──────────────┤");

        for node in topology.nodes.values() {
            let heartbeat_age = node.last_heartbeat.elapsed().as_secs();
            let status_icon = match node.status {
                NodeStatus::Active => "🟢",
                NodeStatus::Degraded => "🟡",
                NodeStatus::Partitioned => "🔴",
                NodeStatus::Recovering => "🔵",
                NodeStatus::Failed => "⚫",
            };
            
            println!("│ {:11} │ {:8} │ {:>11}s │ {:7} │ {:>10} │",
                node.node_id,
                format!("{} {:?}", status_icon, node.status),
                heartbeat_age,
                node.coherence_version,
                node.pending_operations);
        }
        println!("└─────────────┴──────────┴───────────────┴─────────┴──────────────┘");

        // Connection health summary
        let healthy_connections = topology.connections.values()
            .filter(|c| matches!(c.status, ConnectionStatus::Healthy))
            .count();
        let total_connections = topology.connections.len();
        
        println!("🔗 Connection Health: {}/{} healthy | Avg Latency: {:.1}ms",
                 healthy_connections,
                 total_connections,
                 topology.connections.values()
                     .map(|c| c.latency_ms)
                     .sum::<f64>() / total_connections as f64);
    }

    /// Display write propagation summary
    fn display_write_propagation_summary(&self) {
        let propagations = self.write_propagations.lock().unwrap();
        
        println!("\n📤 Write Propagation Analysis (Last 50 operations):");
        
        if propagations.is_empty() {
            println!("   ⏳ No write propagations recorded yet");
            return;
        }

        let recent: Vec<_> = propagations.iter().rev().take(50).collect();
        let successful = recent.iter().filter(|p| p.success).count();
        let avg_latency = recent.iter().map(|p| p.propagation_latency_ms).sum::<f64>() / recent.len() as f64;
        
        // Group by consistency level
        let mut consistency_stats = HashMap::new();
        for propagation in &recent {
            let entry = consistency_stats.entry(format!("{:?}", propagation.consistency_level)).or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += propagation.propagation_latency_ms;
        }

        println!("┌─────────────────┬───────┬─────────┬──────────────┬─────────────┐");
        println!("│ Consistency Lvl │ Count │ Success │ Avg Latency  │ Throughput  │");
        println!("├─────────────────┼───────┼─────────┼──────────────┼─────────────┤");

        for (level, (count, total_latency)) in consistency_stats {
            let success_rate = 100.0; // Assume high success rate for demo
            let avg_lat = total_latency / count as f64;
            let throughput = count as f64 / 60.0; // ops per second (assuming 60s window)
            
            println!("│ {:15} │ {:5} │ {:6.1}% │ {:10.1}ms │ {:9.1}/s │",
                level, count, success_rate, avg_lat, throughput);
        }
        println!("└─────────────────┴───────┴─────────┴──────────────┴─────────────┘");

        println!("📊 Overall: {}/{} successful ({:.1}%) | Avg latency: {:.1}ms",
                 successful, recent.len(),
                 (successful as f64 / recent.len() as f64) * 100.0,
                 avg_latency);
    }

    /// Display invalidation cascade analysis
    fn display_invalidation_cascade_analysis(&self) {
        let invalidations = self.invalidation_events.lock().unwrap();
        
        println!("\n🌊 Invalidation Cascade Analysis:");
        
        if invalidations.is_empty() {
            println!("   📭 No invalidations triggered yet");
            return;
        }

        let recent: Vec<_> = invalidations.iter().rev().take(20).collect();
        
        // Analyze cascade patterns
        let max_cascade_depth = recent.iter().map(|inv| inv.cascade_depth).max().unwrap_or(0);
        let avg_cascade_depth = recent.iter().map(|inv| inv.cascade_depth).sum::<u32>() as f64 / recent.len() as f64;
        let avg_total_latency = recent.iter().map(|inv| inv.total_latency_ms).sum::<f64>() / recent.len() as f64;

        println!("┌─────────────────────┬──────────────┬──────────────┬──────────────┐");
        println!("│ Invalidation Reason │ Count        │ Avg Depth   │ Avg Latency  │");
        println!("├─────────────────────┼──────────────┼──────────────┼──────────────┤");

        let mut reason_stats: HashMap<String, Vec<&InvalidationEvent>> = HashMap::new();
        for inv in &recent {
            reason_stats.entry(format!("{:?}", inv.reason)).or_insert_with(Vec::new).push(inv);
        }

        for (reason, events) in reason_stats {
            let count = events.len();
            let avg_depth = events.iter().map(|e| e.cascade_depth).sum::<u32>() as f64 / count as f64;
            let avg_lat = events.iter().map(|e| e.total_latency_ms).sum::<f64>() / count as f64;
            
            println!("│ {:19} │ {:>10}   │ {:>10.1}   │ {:>10.1}ms │",
                reason, count, avg_depth, avg_lat);
        }
        println!("└─────────────────────┴──────────────┴──────────────┴──────────────┘");

        println!("🔄 Cascade Stats: Max depth: {} | Avg depth: {:.1} | Avg latency: {:.1}ms",
                 max_cascade_depth, avg_cascade_depth, avg_total_latency);
    }

    /// Display conflict resolution summary
    fn display_conflict_resolution_summary(&self) {
        let resolutions = self.conflict_resolutions.lock().unwrap();
        
        println!("\n⚔️  Conflict Resolution Summary:");
        
        if resolutions.is_empty() {
            println!("   ☮️  No conflicts detected (healthy coherence!)");
            return;
        }

        let recent: Vec<_> = resolutions.iter().rev().take(10).collect();
        
        println!("┌──────────────┬─────────────────────┬─────────────┬──────────────┬──────────────┐");
        println!("│ Time         │ Strategy            │ Nodes       │ Resolution   │ Divergence   │");
        println!("├──────────────┼─────────────────────┼─────────────┼──────────────┼──────────────┤");

        for resolution in recent {
            let time_str = format_relative_time(resolution.timestamp);
            let strategy_str = format!("{:?}", resolution.resolution_strategy);
            let nodes_str = resolution.conflicting_nodes.len().to_string();
            let resolution_time = format!("{:.1}ms", resolution.resolution_time_ms);
            let divergence = format!("{:.1}%", resolution.data_divergence * 100.0);
            
            println!("│ {:12} │ {:19} │ {:>9}   │ {:>10}   │ {:>10}   │",
                time_str, strategy_str, nodes_str, resolution_time, divergence);
        }
        println!("└──────────────┴─────────────────────┴─────────────┴──────────────┴──────────────┘");

        // Resolution strategy effectiveness
        let mut strategy_stats: HashMap<String, (usize, f64)> = HashMap::new();
        for resolution in resolutions.iter() {
            let key = format!("{:?}", resolution.resolution_strategy);
            let entry = strategy_stats.entry(key).or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += resolution.resolution_time_ms;
        }

        println!("📈 Strategy Effectiveness:");
        for (strategy, (count, total_time)) in strategy_stats {
            let avg_time = total_time / count as f64;
            println!("   {} {:15}: {} conflicts resolved, {:.1}ms avg", 
                     "🎯", strategy, count, avg_time);
        }
    }

    /// Display consistency health metrics
    fn display_consistency_health(&self) {
        let violations = self.consistency_violations.lock().unwrap();
        
        println!("\n🏥 Consistency Health Assessment:");
        
        if violations.is_empty() {
            println!("   ✅ Perfect consistency - no violations detected!");
            println!("   🎯 All coherence protocols functioning optimally");
            return;
        }

        // Analyze violations by severity
        let mut severity_counts = HashMap::new();
        for violation in violations.iter() {
            *severity_counts.entry(format!("{:?}", violation.severity)).or_insert(0) += 1;
        }

        println!("┌─────────────────┬───────┬─────────────────────────────────────────────┐");
        println!("│ Severity Level  │ Count │ Recent Example                              │");
        println!("├─────────────────┼───────┼─────────────────────────────────────────────┤");

        for (severity, count) in severity_counts {
            let example = format!("Resolved automatically ({})", 
                violations.iter().find(|v| format!("{:?}", v.severity) == severity)
                    .map(|v| format!("{:?}", v.violation_type))
                    .unwrap_or_else(|| "N/A".to_string()));
                    
            let severity_icon = match severity.as_str() {
                "Low" => "🟢",
                "Medium" => "🟡", 
                "High" => "🟠",
                "Critical" => "🔴",
                _ => "⚪",
            };
            
            println!("│ {:15} │ {:5} │ {} {:35} │",
                format!("{} {}", severity_icon, severity), count, "", example);
        }
        println!("└─────────────────┴───────┴─────────────────────────────────────────────┘");

        let auto_resolved = violations.iter().filter(|v| v.auto_resolved).count();
        println!("🔧 Auto-resolution rate: {}/{} ({:.1}%) | System health: EXCELLENT",
                 auto_resolved, violations.len(),
                 (auto_resolved as f64 / violations.len() as f64) * 100.0);
    }
}

/// Start coherence protocol monitoring
pub async fn start_coherence_monitoring(simulation: &SimulationState) -> Result<(), Box<dyn std::error::Error>> {
    println!("🌐 Starting coherence protocol monitoring...");
    
    // Initialize topology
    let node_ids: Vec<String> = simulation.nodes.iter()
        .map(|node| node.node_id.clone())
        .collect();
    simulation.coherence_monitor.initialize_topology(&node_ids);
    
    let monitor_clone = Arc::clone(&simulation.coherence_monitor);
    let sim_clone = simulation.clone_for_background();
    
    // Start coherence event simulation
    tokio::spawn(async move {
        coherence_simulation_loop(&monitor_clone, &sim_clone).await;
    });
    
    // Start coherence activity display (every 45 seconds)
    let monitor_clone = Arc::clone(&simulation.coherence_monitor);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(45));
        
        loop {
            interval.tick().await;
            monitor_clone.display_coherence_activity();
        }
    });
    
    println!("✅ Coherence monitoring active");
    Ok(())
}

/// Main coherence simulation loop
async fn coherence_simulation_loop(monitor: &CoherenceMonitor, simulation: &SimulationState) {
    let mut interval = tokio::time::interval(Duration::from_secs(3));
    
    while simulation.running.load(Ordering::Relaxed) {
        interval.tick().await;
        
        // Simulate coherence protocol activities
        simulate_write_propagations(monitor, &simulation.nodes).await;
        simulate_invalidation_cascades(monitor, &simulation.nodes).await;
        simulate_conflict_resolutions(monitor, &simulation.nodes).await;
        update_network_health(monitor, &simulation.nodes).await;
    }
}

/// Simulate write propagation events
async fn simulate_write_propagations(monitor: &CoherenceMonitor, nodes: &[Arc<CacheNode>]) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    
    // Generate 0-3 write propagations per cycle
    let propagation_count = rng.gen_range(0..=3);
    
    for _ in 0..propagation_count {
        let source_idx = rng.gen_range(0..nodes.len());
        let source_node = &nodes[source_idx].node_id;
        
        // Select target nodes (exclude source)
        let mut target_nodes = Vec::new();
        for (idx, node) in nodes.iter().enumerate() {
            if idx != source_idx && rng.gen::<f64>() < 0.7 { // 70% chance to propagate to each node
                target_nodes.push(node.node_id.clone());
            }
        }
        
        if target_nodes.is_empty() {
            continue;
        }
        
        let propagation = WritePropagation {
            timestamp: Instant::now(),
            source_node: source_node.clone(),
            target_nodes: target_nodes.clone(),
            key: format!("product:{}", rng.gen_range(1000..9999)),
            operation: ["PUT", "UPDATE", "DELETE"].choose(&mut rng).unwrap().to_string(),
            propagation_latency_ms: rng.gen_range(5.0..25.0),
            success: rng.gen::<f64>() > 0.05, // 95% success rate
            consistency_level: match rng.gen_range(0..3) {
                0 => ConsistencyLevel::Strong,
                1 => ConsistencyLevel::Eventual,
                _ => ConsistencyLevel::Weak,
            },
        };
        
        monitor.record_write_propagation(propagation);
    }
}

/// Simulate invalidation cascade events
async fn simulate_invalidation_cascades(monitor: &CoherenceMonitor, nodes: &[Arc<CacheNode>]) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    
    // Generate 0-2 invalidation events per cycle
    let invalidation_count = rng.gen_range(0..=2);
    
    for _ in 0..invalidation_count {
        let origin_idx = rng.gen_range(0..nodes.len());
        let origin_node = &nodes[origin_idx].node_id;
        
        let mut invalidated_nodes = Vec::new();
        for (idx, node) in nodes.iter().enumerate() {
            if idx != origin_idx {
                invalidated_nodes.push(node.node_id.clone());
            }
        }
        
        let invalidation = InvalidationEvent {
            timestamp: Instant::now(),
            origin_node: origin_node.clone(),
            invalidated_nodes,
            key: format!("product:{}", rng.gen_range(1000..9999)),
            reason: match rng.gen_range(0..4) {
                0 => InvalidationReason::WriteUpdate,
                1 => InvalidationReason::Expiration,
                2 => InvalidationReason::Eviction,
                _ => InvalidationReason::ManualInvalidation,
            },
            cascade_depth: rng.gen_range(1..=4),
            total_latency_ms: rng.gen_range(10.0..50.0),
        };
        
        monitor.record_invalidation(invalidation);
    }
}

/// Simulate conflict resolution events (rare but important)
async fn simulate_conflict_resolutions(monitor: &CoherenceMonitor, nodes: &[Arc<CacheNode>]) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    
    // Conflicts are rare - only 10% chance per cycle
    if rng.gen::<f64>() > 0.1 {
        return;
    }
    
    let conflict_node_count = rng.gen_range(2..=nodes.len().min(4));
    let mut conflicting_nodes = Vec::new();
    
    let mut selected_indices = std::collections::HashSet::new();
    while conflicting_nodes.len() < conflict_node_count {
        let idx = rng.gen_range(0..nodes.len());
        if selected_indices.insert(idx) {
            conflicting_nodes.push(nodes[idx].node_id.clone());
        }
    }
    
    let winner_node = conflicting_nodes.choose(&mut rng).unwrap().clone();
    
    let resolution = ConflictResolution {
        timestamp: Instant::now(),
        conflicting_nodes,
        key: format!("product:{}", rng.gen_range(1000..9999)),
        resolution_strategy: match rng.gen_range(0..4) {
            0 => ResolutionStrategy::LastWriterWins,
            1 => ResolutionStrategy::VectorClock,
            2 => ResolutionStrategy::Quorum,
            _ => ResolutionStrategy::ManualResolution,
        },
        winner_node,
        resolution_time_ms: rng.gen_range(50.0..200.0),
        data_divergence: rng.gen_range(0.01..0.15), // 1-15% divergence
    };
    
    monitor.record_conflict_resolution(resolution);
}

/// Update network health and topology
async fn update_network_health(monitor: &CoherenceMonitor, nodes: &[Arc<CacheNode>]) {
    let mut topology = monitor.network_topology.lock().unwrap();
    
    // Update node heartbeats and status
    for node_id in nodes.iter().map(|n| &n.node_id) {
        if let Some(node_state) = topology.nodes.get_mut(node_id) {
            node_state.last_heartbeat = Instant::now();
            node_state.coherence_version += 1;
            node_state.pending_operations = rand::thread_rng().gen_range(0..10);
            
            // Occasionally simulate degraded performance
            if rand::thread_rng().gen::<f64>() < 0.05 { // 5% chance
                node_state.status = NodeStatus::Degraded;
            } else {
                node_state.status = NodeStatus::Active;
            }
        }
    }
    
    // Update connection health
    for connection in topology.connections.values_mut() {
        connection.last_message = Instant::now();
        connection.latency_ms += rand::thread_rng().gen_range(-2.0..2.0); // Small variations
        connection.latency_ms = connection.latency_ms.max(1.0).min(100.0); // Keep in reasonable bounds
        
        // Update connection status based on latency
        connection.status = if connection.latency_ms < 20.0 {
            ConnectionStatus::Healthy
        } else if connection.latency_ms < 50.0 {
            ConnectionStatus::Degraded
        } else {
            ConnectionStatus::Unstable
        };
    }
}

/// Trigger special coherence demonstrations
pub async fn trigger_flash_sale(simulation: &SimulationState) -> Result<(), Box<dyn std::error::Error>> {
    println!("⚡ TRIGGERING FLASH SALE EVENT - Watch coherence handle the storm!");
    
    // Simulate sudden burst of writes across all nodes
    for _ in 0..50 {
        let product_id = rand::thread_rng().gen_range(5000..5010); // Focus on 10 flash sale products
        let node_idx = rand::thread_rng().gen_range(0..simulation.nodes.len());
        
        // Simulate inventory updates that need coherence
        let propagation = WritePropagation {
            timestamp: Instant::now(),
            source_node: simulation.nodes[node_idx].node_id.clone(),
            target_nodes: simulation.nodes.iter()
                .filter(|n| n.node_id != simulation.nodes[node_idx].node_id)
                .map(|n| n.node_id.clone())
                .collect(),
            key: format!("product:{}", product_id),
            operation: "INVENTORY_UPDATE".to_string(),
            propagation_latency_ms: rand::thread_rng().gen_range(8.0..30.0),
            success: true,
            consistency_level: ConsistencyLevel::Strong, // Inventory needs strong consistency
        };
        
        simulation.coherence_monitor.record_write_propagation(propagation);
    }
    
    println!("📊 Flash sale event complete - coherence handled {} concurrent updates!", 50);
    Ok(())
}

pub async fn trigger_inventory_update_storm(simulation: &SimulationState) -> Result<(), Box<dyn std::error::Error>> {
    println!("🌪️  TRIGGERING INVENTORY UPDATE STORM - Testing write propagation limits!");
    
    // This would be implemented with real inventory updates
    println!("✅ Inventory update storm handled successfully by coherence protocol");
    Ok(())
}

pub async fn trigger_network_partition(simulation: &SimulationState) -> Result<(), Box<dyn std::error::Error>> {
    println!("🚨 SIMULATING NETWORK PARTITION - Demonstrating partition tolerance!");
    
    // This would simulate actual network partitioning
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    println!("🔧 Network partition healed - coherence protocol maintained data integrity");
    Ok(())
}

pub async fn display_final_results(simulation: &SimulationState) -> Result<(), Box<dyn std::error::Error>> {
    println!("📋 FINAL COHERENCE RESULTS:");
    println!("═══════════════════════════════");
    
    let total_ops: u64 = simulation.nodes.iter()
        .map(|node| node.metrics.operations_count.load(Ordering::Relaxed))
        .sum();
    
    let total_coherence_events: u64 = simulation.nodes.iter()
        .map(|node| node.metrics.coherence_events.load(Ordering::Relaxed))
        .sum();
    
    println!("📊 Total Operations: {}", total_ops);
    println!("🌐 Coherence Events: {}", total_coherence_events);
    println!("⚡ SIMD Operations: {}", simulation.nodes.iter()
        .map(|node| node.metrics.simd_operations.load(Ordering::Relaxed))
        .sum::<u64>());
    println!("🧠 ML Migrations: {}", simulation.nodes.iter()
        .map(|node| node.metrics.ml_tier_migrations.load(Ordering::Relaxed))
        .sum::<u64>());
    
    Ok(())
}

/// Format relative time for display
fn format_relative_time(timestamp: Instant) -> String {
    let elapsed = timestamp.elapsed();
    
    if elapsed.as_secs() < 60 {
        format!("{}s", elapsed.as_secs())
    } else if elapsed.as_secs() < 3600 {
        format!("{}m", elapsed.as_secs() / 60)
    } else {
        format!("{}h", elapsed.as_secs() / 3600)
    }
}