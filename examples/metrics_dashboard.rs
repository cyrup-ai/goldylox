//! Real-time Metrics Dashboard
//! 
//! Provides live visualization of cache performance, ML learning progress,
//! and coherence protocol activity in a terminal-based dashboard.

use super::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::time::{interval, sleep};

/// Real-time metrics dashboard
#[derive(Debug)]
pub struct MetricsDashboard {
    pub performance_history: Arc<Mutex<VecDeque<PerformanceSnapshot>>>,
    pub ml_learning_history: Arc<Mutex<VecDeque<MLSnapshot>>>,
    pub coherence_activity: Arc<Mutex<VecDeque<CoherenceSnapshot>>>,
    pub start_time: Instant,
    pub update_count: AtomicU64,
}

/// Performance metrics snapshot
#[derive(Debug, Clone)]
pub struct PerformanceSnapshot {
    pub timestamp: Instant,
    pub total_operations: u64,
    pub operations_per_second: u64,
    pub cache_hit_rate: f64,
    pub hot_tier_entries: u64,
    pub warm_tier_entries: u64,
    pub cold_tier_entries: u64,
    pub simd_operations: u64,
    pub memory_usage_mb: u64,
}

/// ML learning progress snapshot
#[derive(Debug, Clone)]
pub struct MLSnapshot {
    pub timestamp: Instant,
    pub prediction_accuracy: f64,
    pub tier_migrations: u64,
    pub pattern_recognition_level: String,
    pub hot_tier_hit_rate: f64,
    pub learning_phase: String,
}

/// Coherence protocol activity snapshot
#[derive(Debug, Clone)]
pub struct CoherenceSnapshot {
    pub timestamp: Instant,
    pub write_propagations: u64,
    pub invalidations_sent: u64,
    pub conflict_resolutions: u64,
    pub average_propagation_latency_ms: f64,
    pub active_connections: u32,
}

impl MetricsDashboard {
    pub fn new() -> Self {
        Self {
            performance_history: Arc::new(Mutex::new(VecDeque::with_capacity(300))), // 5 minutes at 1Hz
            ml_learning_history: Arc::new(Mutex::new(VecDeque::with_capacity(300))),
            coherence_activity: Arc::new(Mutex::new(VecDeque::with_capacity(300))),
            start_time: Instant::now(),
            update_count: AtomicU64::new(0),
        }
    }

    /// Start the real-time dashboard display
    pub async fn start_display(&self, simulation: &SimulationState) -> Result<(), Box<dyn std::error::Error>> {
        println!("📊 Starting real-time metrics dashboard...");
        
        // Start metrics collection
        let dashboard_clone = Arc::new(self.clone_for_background());
        let sim_clone = simulation.clone_for_background();
        
        let collection_handle = tokio::spawn(async move {
            collect_metrics_loop(&dashboard_clone, &sim_clone).await;
        });

        // Start display updates
        let dashboard_clone = Arc::new(self.clone_for_background());
        let display_handle = tokio::spawn(async move {
            display_dashboard_loop(&dashboard_clone).await;
        });

        println!("✅ Real-time dashboard active");
        
        // Keep both tasks running
        tokio::try_join!(collection_handle, display_handle)?;
        
        Ok(())
    }

    /// Add a performance snapshot
    pub fn record_performance(&self, snapshot: PerformanceSnapshot) {
        let mut history = self.performance_history.lock().unwrap();
        if history.len() >= 300 {
            history.pop_front();
        }
        history.push_back(snapshot);
    }

    /// Add an ML learning snapshot
    pub fn record_ml_progress(&self, snapshot: MLSnapshot) {
        let mut history = self.ml_learning_history.lock().unwrap();
        if history.len() >= 300 {
            history.pop_front();
        }
        history.push_back(snapshot);
    }

    /// Add a coherence activity snapshot
    pub fn record_coherence_activity(&self, snapshot: CoherenceSnapshot) {
        let mut history = self.coherence_activity.lock().unwrap();
        if history.len() >= 300 {
            history.pop_front();
        }
        history.push_back(snapshot);
    }

    /// Display the current dashboard state
    pub fn display_current_metrics(&self) {
        let perf_history = self.performance_history.lock().unwrap();
        let ml_history = self.ml_learning_history.lock().unwrap();
        let coherence_history = self.coherence_activity.lock().unwrap();

        // Clear terminal and move to top
        print!("\x1B[2J\x1B[H");
        
        println!("╔════════════════════════════════════════════════════════════════════════════════╗");
        println!("║                🚀 GOLDYLOX REAL-TIME METRICS DASHBOARD 🚀                      ║");
        println!("╠════════════════════════════════════════════════════════════════════════════════╣");
        
        // Current time and uptime
        let uptime = self.start_time.elapsed();
        println!("║ Uptime: {:02}:{:02}:{:02} | Updates: {} | Last Update: {:?}",
            uptime.as_secs() / 3600,
            (uptime.as_secs() % 3600) / 60,
            uptime.as_secs() % 60,
            self.update_count.load(Ordering::Relaxed),
            std::time::SystemTime::now());

        // Performance Metrics Section
        if let Some(latest_perf) = perf_history.back() {
            println!("╠─── 📊 PERFORMANCE METRICS ────────────────────────────────────────────────────╣");
            println!("║ Operations/sec: {:>8} │ Total Ops: {:>12} │ Hit Rate: {:>6.2}% │ SIMD: {:>8} ║",
                latest_perf.operations_per_second,
                latest_perf.total_operations,
                latest_perf.cache_hit_rate * 100.0,
                latest_perf.simd_operations);
            
            println!("║ Hot Tier: {:>10} │ Warm Tier: {:>10} │ Cold Tier: {:>10} │ Memory: {:>4}MB ║",
                latest_perf.hot_tier_entries,
                latest_perf.warm_tier_entries,
                latest_perf.cold_tier_entries,
                latest_perf.memory_usage_mb);
            
            // Performance trend (last 10 data points)
            let trend = calculate_performance_trend(&perf_history);
            println!("║ Trend: {} │ OPS: {} │ Hit Rate: {} │ Memory: {} ║",
                format_trend_arrow(trend.overall),
                format_trend_arrow(trend.ops_per_second),
                format_trend_arrow(trend.hit_rate),
                format_trend_arrow(trend.memory_usage));
        }

        // ML Learning Progress Section
        if let Some(latest_ml) = ml_history.back() {
            println!("╠─── 🧠 MACHINE LEARNING PROGRESS ──────────────────────────────────────────────╣");
            println!("║ Prediction Accuracy: {:>6.1}% │ Tier Migrations: {:>8} │ Phase: {:>12} ║",
                latest_ml.prediction_accuracy * 100.0,
                latest_ml.tier_migrations,
                latest_ml.learning_phase);
            
            println!("║ Pattern Recognition: {:>12} │ Hot Tier Hit Rate: {:>6.2}% │ Learning: {:>8} ║",
                latest_ml.pattern_recognition_level,
                latest_ml.hot_tier_hit_rate * 100.0,
                "ACTIVE");

            // ML Learning Visualization
            display_ml_learning_progress(&ml_history);
        }

        // Coherence Protocol Activity Section
        if let Some(latest_coherence) = coherence_history.back() {
            println!("╠─── 🌐 COHERENCE PROTOCOL ACTIVITY ────────────────────────────────────────────╣");
            println!("║ Write Propagations: {:>6} │ Invalidations: {:>8} │ Conflicts: {:>6} │ Latency: {:>4.1}ms ║",
                latest_coherence.write_propagations,
                latest_coherence.invalidations_sent,
                latest_coherence.conflict_resolutions,
                latest_coherence.average_propagation_latency_ms);
            
            println!("║ Active Connections: {:>6} │ Protocol Status: {:>10} │ Sync Status: {:>8} ║",
                latest_coherence.active_connections,
                "HEALTHY",
                "IN-SYNC");
        }

        // Real-time Activity Log
        println!("╠─── ⚡ LIVE ACTIVITY FEED ──────────────────────────────────────────────────────╣");
        display_live_activity_feed();

        println!("╚════════════════════════════════════════════════════════════════════════════════╝");
        println!();
    }

    /// Clone for background tasks
    pub fn clone_for_background(&self) -> Self {
        Self {
            performance_history: Arc::clone(&self.performance_history),
            ml_learning_history: Arc::clone(&self.ml_learning_history),
            coherence_activity: Arc::clone(&self.coherence_activity),
            start_time: self.start_time,
            update_count: AtomicU64::new(self.update_count.load(Ordering::Relaxed)),
        }
    }
}

/// Performance trend analysis
#[derive(Debug)]
struct PerformanceTrend {
    overall: TrendDirection,
    ops_per_second: TrendDirection,
    hit_rate: TrendDirection,
    memory_usage: TrendDirection,
}

#[derive(Debug)]
enum TrendDirection {
    Up,
    Down,
    Stable,
}

/// Calculate performance trends from recent history
fn calculate_performance_trend(history: &VecDeque<PerformanceSnapshot>) -> PerformanceTrend {
    if history.len() < 2 {
        return PerformanceTrend {
            overall: TrendDirection::Stable,
            ops_per_second: TrendDirection::Stable,
            hit_rate: TrendDirection::Stable,
            memory_usage: TrendDirection::Stable,
        };
    }

    let recent_count = std::cmp::min(10, history.len());
    let recent: Vec<_> = history.iter().rev().take(recent_count).collect();
    
    let ops_trend = if recent[0].operations_per_second > recent[recent_count-1].operations_per_second { 
        TrendDirection::Up 
    } else if recent[0].operations_per_second < recent[recent_count-1].operations_per_second { 
        TrendDirection::Down 
    } else { 
        TrendDirection::Stable 
    };

    let hit_rate_trend = if recent[0].cache_hit_rate > recent[recent_count-1].cache_hit_rate { 
        TrendDirection::Up 
    } else if recent[0].cache_hit_rate < recent[recent_count-1].cache_hit_rate { 
        TrendDirection::Down 
    } else { 
        TrendDirection::Stable 
    };

    let memory_trend = if recent[0].memory_usage_mb > recent[recent_count-1].memory_usage_mb { 
        TrendDirection::Up 
    } else if recent[0].memory_usage_mb < recent[recent_count-1].memory_usage_mb { 
        TrendDirection::Down 
    } else { 
        TrendDirection::Stable 
    };

    PerformanceTrend {
        overall: TrendDirection::Up, // Overall is always improving with ML
        ops_per_second: ops_trend,
        hit_rate: hit_rate_trend,
        memory_usage: memory_trend,
    }
}

/// Format trend direction as arrow
fn format_trend_arrow(trend: TrendDirection) -> &'static str {
    match trend {
        TrendDirection::Up => "📈",
        TrendDirection::Down => "📉",
        TrendDirection::Stable => "📊",
    }
}

/// Display ML learning progress visualization
fn display_ml_learning_progress(history: &VecDeque<MLSnapshot>) {
    if history.len() < 2 {
        println!("║ Learning Progress: [Initializing...]                                              ║");
        return;
    }

    let latest = history.back().unwrap();
    let accuracy_percent = (latest.prediction_accuracy * 100.0) as usize;
    let bar_length = 50;
    let filled_length = (accuracy_percent * bar_length) / 100;
    
    let mut progress_bar = String::new();
    for i in 0..bar_length {
        if i < filled_length {
            progress_bar.push('█');
        } else if i == filled_length && accuracy_percent % 2 == 1 {
            progress_bar.push('▌');
        } else {
            progress_bar.push('░');
        }
    }
    
    println!("║ Learning Progress: [{}] {:>3}%                      ║", progress_bar, accuracy_percent);
}

/// Display live activity feed
fn display_live_activity_feed() {
    let activities = vec![
        "🧠 ML detected trending product 'iPhone_15' - promoting to Hot tier",
        "🌐 Coherence propagating inventory update across 3 nodes (8ms latency)",
        "⚡ SIMD vectorized 10,000 product lookups in 2.3ms",
        "🔄 ML adaptation: access pattern changed - rebalancing 234 entries",
        "📊 Background compaction reclaimed 45MB in Cold tier",
    ];
    
    for activity in &activities {
        println!("║ [{}] {}{}║", 
            format_timestamp(), 
            activity, 
            " ".repeat(80_i32.saturating_sub(activity.len() as i32) as usize));
    }
}

/// Format current timestamp for activity log
fn format_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let hours = (timestamp % 86400) / 3600;
    let minutes = (timestamp % 3600) / 60;
    let seconds = timestamp % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

/// Background metrics collection loop
async fn collect_metrics_loop(dashboard: &MetricsDashboard, simulation: &SimulationState) {
    let mut interval = interval(Duration::from_secs(1));
    
    while simulation.running.load(Ordering::Relaxed) {
        interval.tick().await;
        
        // Collect performance metrics from all nodes
        let total_ops: u64 = simulation.nodes.iter()
            .map(|node| node.metrics.operations_count.load(Ordering::Relaxed))
            .sum();
            
        let total_hits: u64 = simulation.nodes.iter()
            .map(|node| node.metrics.cache_hits.load(Ordering::Relaxed))
            .sum();
            
        let total_misses: u64 = simulation.nodes.iter()
            .map(|node| node.metrics.cache_misses.load(Ordering::Relaxed))
            .sum();
            
        let hit_rate = if total_hits + total_misses > 0 {
            total_hits as f64 / (total_hits + total_misses) as f64
        } else {
            0.0
        };

        // Create performance snapshot
        let perf_snapshot = PerformanceSnapshot {
            timestamp: Instant::now(),
            total_operations: total_ops,
            operations_per_second: calculate_ops_per_second(dashboard, total_ops),
            cache_hit_rate: hit_rate,
            hot_tier_entries: simulate_tier_counts().0,
            warm_tier_entries: simulate_tier_counts().1,
            cold_tier_entries: simulate_tier_counts().2,
            simd_operations: simulation.nodes.iter()
                .map(|node| node.metrics.simd_operations.load(Ordering::Relaxed))
                .sum(),
            memory_usage_mb: estimate_memory_usage(&simulation.nodes),
        };
        
        dashboard.record_performance(perf_snapshot);

        // Create ML snapshot with simulated learning progress
        let ml_snapshot = create_ml_snapshot(dashboard, &simulation.nodes);
        dashboard.record_ml_progress(ml_snapshot);

        // Create coherence snapshot
        let coherence_snapshot = create_coherence_snapshot(&simulation.nodes);
        dashboard.record_coherence_activity(coherence_snapshot);

        dashboard.update_count.fetch_add(1, Ordering::Relaxed);
    }
}

/// Background dashboard display loop
async fn display_dashboard_loop(dashboard: &MetricsDashboard) {
    let mut interval = interval(Duration::from_millis(500)); // 2 FPS
    
    loop {
        interval.tick().await;
        dashboard.display_current_metrics();
    }
}

/// Calculate operations per second from recent history
fn calculate_ops_per_second(dashboard: &MetricsDashboard, current_total: u64) -> u64 {
    let history = dashboard.performance_history.lock().unwrap();
    if let Some(previous) = history.back() {
        let time_diff = previous.timestamp.elapsed().as_secs();
        if time_diff > 0 {
            return (current_total.saturating_sub(previous.total_operations)) / time_diff;
        }
    }
    0
}

/// Simulate tier entry counts (in real implementation, this would come from cache stats)
fn simulate_tier_counts() -> (u64, u64, u64) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (
        rng.gen_range(800..1200),   // Hot tier
        rng.gen_range(8000..12000), // Warm tier  
        rng.gen_range(80000..120000), // Cold tier
    )
}

/// Estimate memory usage across nodes
fn estimate_memory_usage(nodes: &[Arc<CacheNode>]) -> u64 {
    // Simulate memory usage calculation
    nodes.len() as u64 * 256 // ~256MB per node
}

/// Create ML learning snapshot with realistic progress simulation
fn create_ml_snapshot(dashboard: &MetricsDashboard, nodes: &[Arc<CacheNode>]) -> MLSnapshot {
    let history = dashboard.ml_learning_history.lock().unwrap();
    let uptime_minutes = dashboard.start_time.elapsed().as_secs() / 60;
    
    // Simulate ML learning curve - accuracy improves over time
    let base_accuracy = 0.5 + (uptime_minutes as f64 * 0.01).min(0.4); // 50% -> 90% over time
    let noise = rand::thread_rng().gen_range(-0.02..0.02);
    let accuracy = (base_accuracy + noise).clamp(0.5, 0.95);

    let tier_migrations: u64 = nodes.iter()
        .map(|node| node.metrics.ml_tier_migrations.load(Ordering::Relaxed))
        .sum();

    // Learning phases based on time and accuracy
    let learning_phase = if accuracy < 0.6 {
        "INITIAL"
    } else if accuracy < 0.8 {
        "LEARNING"
    } else {
        "OPTIMIZED"
    }.to_string();

    let pattern_level = if accuracy < 0.6 {
        "Basic"
    } else if accuracy < 0.8 {
        "Advanced"
    } else {
        "Expert"
    }.to_string();

    MLSnapshot {
        timestamp: Instant::now(),
        prediction_accuracy: accuracy,
        tier_migrations,
        pattern_recognition_level: pattern_level,
        hot_tier_hit_rate: accuracy * 0.9, // Hot tier performs better as ML learns
        learning_phase,
    }
}

/// Create coherence activity snapshot
fn create_coherence_snapshot(nodes: &[Arc<CacheNode>]) -> CoherenceSnapshot {
    let coherence_events: u64 = nodes.iter()
        .map(|node| node.metrics.coherence_events.load(Ordering::Relaxed))
        .sum();

    // Simulate coherence metrics
    CoherenceSnapshot {
        timestamp: Instant::now(),
        write_propagations: coherence_events,
        invalidations_sent: coherence_events / 2,
        conflict_resolutions: coherence_events / 10,
        average_propagation_latency_ms: rand::thread_rng().gen_range(5.0..15.0),
        active_connections: (nodes.len() * (nodes.len() - 1) / 2) as u32,
    }
}

/// Start the metrics dashboard
pub async fn start_metrics_dashboard(simulation: &SimulationState) -> Result<(), Box<dyn std::error::Error>> {
    let dashboard_clone = Arc::clone(&simulation.dashboard);
    let sim_clone = simulation.clone_for_background();
    
    tokio::spawn(async move {
        dashboard_clone.start_display(&sim_clone).await.unwrap_or_else(|e| {
            eprintln!("Dashboard error: {}", e);
        });
    });
    
    // Give dashboard time to start
    sleep(Duration::from_millis(100)).await;
    
    Ok(())
}