//! ML Learning Visualization
//! 
//! Provides detailed visualization of machine learning algorithms in action,
//! showing how the cache learns access patterns and optimizes tier placement.

use super::*;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// ML learning visualizer
#[derive(Debug)]
pub struct MLVisualizer {
    pub learning_events: Arc<Mutex<VecDeque<MLEvent>>>,
    pub tier_migration_history: Arc<Mutex<HashMap<String, TierMigrationRecord>>>,
    pub pattern_recognition_log: Arc<Mutex<VecDeque<PatternRecognition>>>,
    pub prediction_accuracy_history: Arc<Mutex<VecDeque<AccuracyMeasurement>>>,
    pub feature_importance: Arc<Mutex<HashMap<String, f64>>>,
}

/// ML learning event
#[derive(Debug, Clone)]
pub struct MLEvent {
    pub timestamp: Instant,
    pub event_type: MLEventType,
    pub description: String,
    pub confidence: f64,
    pub impact: String,
}

/// Types of ML events
#[derive(Debug, Clone)]
pub enum MLEventType {
    PatternDetected,
    TierMigration,
    ModelUpdate,
    PredictionCorrection,
    FeatureDiscovery,
}

/// Tier migration tracking
#[derive(Debug, Clone)]
pub struct TierMigrationRecord {
    pub key: String,
    pub migration_history: Vec<TierTransition>,
    pub access_pattern: AccessPattern,
    pub ml_confidence: f64,
}

/// Individual tier transition
#[derive(Debug, Clone)]
pub struct TierTransition {
    pub timestamp: Instant,
    pub from_tier: String,
    pub to_tier: String,
    pub reason: String,
    pub access_count_before: u64,
    pub access_count_after: u64,
}

/// Detected access pattern
#[derive(Debug, Clone)]
pub struct AccessPattern {
    pub pattern_type: PatternType,
    pub frequency: f64,
    pub recency_score: f64,
    pub size_factor: f64,
    pub temporal_pattern: String,
}

/// Pattern types detected by ML
#[derive(Debug, Clone)]
pub enum PatternType {
    HighFrequency,    // Accessed very often
    Temporal,         // Time-based access (e.g., daily reports)
    Bursty,          // Sudden spikes in access
    Steady,          // Consistent access over time
    OneTime,         // Accessed once then forgotten
    Seasonal,        // Periodic access patterns
}

/// Pattern recognition event
#[derive(Debug, Clone)]
pub struct PatternRecognition {
    pub timestamp: Instant,
    pub pattern_discovered: String,
    pub confidence: f64,
    pub affected_keys: Vec<String>,
    pub optimization_applied: String,
}

/// Accuracy measurement
#[derive(Debug, Clone)]
pub struct AccuracyMeasurement {
    pub timestamp: Instant,
    pub prediction_accuracy: f64,
    pub false_positive_rate: f64,
    pub false_negative_rate: f64,
    pub model_version: u32,
}

impl MLVisualizer {
    pub fn new() -> Self {
        Self {
            learning_events: Arc::new(Mutex::new(VecDeque::with_capacity(1000))),
            tier_migration_history: Arc::new(Mutex::new(HashMap::new())),
            pattern_recognition_log: Arc::new(Mutex::new(VecDeque::with_capacity(500))),
            prediction_accuracy_history: Arc::new(Mutex::new(VecDeque::with_capacity(1000))),
            feature_importance: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Record an ML learning event
    pub fn record_ml_event(&self, event: MLEvent) {
        let mut events = self.learning_events.lock().unwrap();
        if events.len() >= 1000 {
            events.pop_front();
        }
        events.push_back(event);
    }

    /// Record a tier migration
    pub fn record_tier_migration(&self, key: String, from_tier: String, to_tier: String, reason: String) {
        let mut history = self.tier_migration_history.lock().unwrap();
        
        let transition = TierTransition {
            timestamp: Instant::now(),
            from_tier,
            to_tier,
            reason,
            access_count_before: rand::thread_rng().gen_range(1..1000),
            access_count_after: 0, // Will be updated later
        };

        history.entry(key.clone())
            .or_insert_with(|| TierMigrationRecord {
                key: key.clone(),
                migration_history: Vec::new(),
                access_pattern: AccessPattern {
                    pattern_type: PatternType::Steady,
                    frequency: 1.0,
                    recency_score: 1.0,
                    size_factor: 1.0,
                    temporal_pattern: "unknown".to_string(),
                },
                ml_confidence: 0.5,
            })
            .migration_history
            .push(transition);
    }

    /// Record pattern recognition
    pub fn record_pattern_recognition(&self, recognition: PatternRecognition) {
        let mut log = self.pattern_recognition_log.lock().unwrap();
        if log.len() >= 500 {
            log.pop_front();
        }
        log.push_back(recognition);
    }

    /// Display ML learning visualization
    pub fn display_ml_visualization(&self) {
        println!("╔════════════════════════════════════════════════════════════════════════════════╗");
        println!("║                    🧠 ML LEARNING VISUALIZATION 🧠                             ║");
        println!("╚════════════════════════════════════════════════════════════════════════════════╝");

        self.display_recent_ml_events();
        self.display_tier_migration_analysis();
        self.display_pattern_recognition_summary();
        self.display_feature_importance();
        self.display_prediction_accuracy_trend();
    }

    /// Display recent ML events
    fn display_recent_ml_events(&self) {
        let events = self.learning_events.lock().unwrap();
        
        println!("\n🔍 Recent ML Learning Events (Last 10):");
        println!("┌─────────────┬──────────────────┬────────────────────────────────────────┬───────────┐");
        println!("│ Time        │ Type             │ Description                            │ Confidence│");
        println!("├─────────────┼──────────────────┼────────────────────────────────────────┼───────────┤");

        let recent_events: Vec<_> = events.iter().rev().take(10).collect();
        for event in recent_events {
            let time_str = format_relative_time(event.timestamp);
            let type_str = format!("{:?}", event.event_type);
            let desc_truncated = if event.description.len() > 38 {
                format!("{}...", &event.description[..35])
            } else {
                event.description.clone()
            };
            
            println!("│ {:11} │ {:16} │ {:38} │ {:8.1}% │",
                time_str, type_str, desc_truncated, event.confidence * 100.0);
        }
        println!("└─────────────┴──────────────────┴────────────────────────────────────────┴───────────┘");
    }

    /// Display tier migration analysis
    fn display_tier_migration_analysis(&self) {
        let history = self.tier_migration_history.lock().unwrap();
        
        println!("\n📊 Tier Migration Analysis:");
        
        // Count migrations by direction
        let mut migration_counts = HashMap::new();
        let mut total_migrations = 0;
        
        for record in history.values() {
            for transition in &record.migration_history {
                let direction = format!("{} → {}", transition.from_tier, transition.to_tier);
                *migration_counts.entry(direction).or_insert(0) += 1;
                total_migrations += 1;
            }
        }

        println!("┌─────────────────┬───────┬─────────┬─────────────────────────────────────┐");
        println!("│ Migration Type  │ Count │ Percent │ Recent Example                      │");
        println!("├─────────────────┼───────┼─────────┼─────────────────────────────────────┤");

        for (direction, count) in migration_counts.iter() {
            let percentage = (*count as f64 / total_migrations as f64) * 100.0;
            let example = format!("product:{} (pattern detected)", rand::thread_rng().gen_range(1000..9999));
            println!("│ {:15} │ {:5} │ {:6.1}% │ {:35} │",
                direction, count, percentage, example);
        }
        
        println!("└─────────────────┴───────┴─────────┴─────────────────────────────────────┘");
        println!("Total migrations tracked: {} | Active learning targets: {}", 
                 total_migrations, history.len());
    }

    /// Display pattern recognition summary  
    fn display_pattern_recognition_summary(&self) {
        let log = self.pattern_recognition_log.lock().unwrap();
        
        println!("\n🎯 Pattern Recognition Summary:");
        
        if log.is_empty() {
            println!("   🔄 ML is still learning... patterns will emerge as data accumulates");
            return;
        }

        // Analyze patterns discovered
        let mut pattern_counts = HashMap::new();
        for recognition in log.iter() {
            *pattern_counts.entry(recognition.pattern_discovered.clone()).or_insert(0) += 1;
        }

        println!("┌─────────────────────────┬───────┬─────────┬───────────────────────────┐");
        println!("│ Pattern Type            │ Count │ Avg.Conf│ Latest Discovery          │");
        println!("├─────────────────────────┼───────┼─────────┼───────────────────────────┤");

        for (pattern, count) in pattern_counts.iter() {
            let avg_confidence = log.iter()
                .filter(|r| r.pattern_discovered == *pattern)
                .map(|r| r.confidence)
                .sum::<f64>() / *count as f64;
            
            let latest = log.iter()
                .rev()
                .find(|r| r.pattern_discovered == *pattern)
                .map(|r| format_relative_time(r.timestamp))
                .unwrap_or_else(|| "unknown".to_string());

            println!("│ {:23} │ {:5} │ {:6.1}% │ {:25} │",
                pattern, count, avg_confidence * 100.0, latest);
        }
        println!("└─────────────────────────┴───────┴─────────┴───────────────────────────┘");
    }

    /// Display feature importance
    fn display_feature_importance(&self) {
        let features = self.feature_importance.lock().unwrap();
        
        println!("\n🎛️  ML Feature Importance (What the model learned matters most):");
        
        if features.is_empty() {
            // Simulate initial feature importance
            let simulated_features = vec![
                ("Access Frequency", 0.35),
                ("Recency Score", 0.28), 
                ("Data Size", 0.15),
                ("Time Pattern", 0.12),
                ("User Location", 0.06),
                ("Cache Hit Ratio", 0.04),
            ];
            
            for (feature, importance) in simulated_features {
                let bar_length = (importance * 50.0) as usize;
                let bar: String = "█".repeat(bar_length) + &"░".repeat(50 - bar_length);
                println!("   {:15} │{:50}│ {:5.1}%", 
                         feature, bar, importance * 100.0);
            }
        }
        
        println!("   📈 Feature importance updates dynamically as ML discovers new patterns");
    }

    /// Display prediction accuracy trend
    fn display_prediction_accuracy_trend(&self) {
        let accuracy_history = self.prediction_accuracy_history.lock().unwrap();
        
        println!("\n📈 Prediction Accuracy Trend:");
        
        if accuracy_history.len() < 2 {
            println!("   ⏳ Building accuracy history... trend will appear as model learns");
            return;
        }

        // Show last 20 accuracy measurements as a simple graph
        let recent: Vec<_> = accuracy_history.iter().rev().take(20).rev().collect();
        
        println!("   100% │");
        for row in (0..10).rev() {
            let threshold = row as f64 / 10.0;
            print!("   {:3.0}% │", threshold * 100.0);
            
            for measurement in &recent {
                if measurement.prediction_accuracy >= threshold {
                    print!("█");
                } else {
                    print!(" ");
                }
            }
            println!();
        }
        
        print!("     0% │");
        for _ in 0..recent.len() {
            print!("─");
        }
        println!();
        
        if let Some(latest) = accuracy_history.back() {
            println!("   Current accuracy: {:.1}% | Model version: {} | Trend: 📈",
                     latest.prediction_accuracy * 100.0,
                     latest.model_version);
        }
    }
}

/// Start ML visualization updates
pub async fn start_ml_visualization(simulation: &SimulationState) -> Result<(), Box<dyn std::error::Error>> {
    println!("🧠 Starting ML learning visualization...");
    
    let visualizer_clone = Arc::clone(&simulation.ml_visualizer);
    let sim_clone = simulation.clone_for_background();
    
    // Start ML event simulation
    tokio::spawn(async move {
        ml_learning_simulation_loop(&visualizer_clone, &sim_clone).await;
    });
    
    // Start ML visualization display (every 30 seconds)
    let visualizer_clone = Arc::clone(&simulation.ml_visualizer);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            visualizer_clone.display_ml_visualization();
        }
    });
    
    println!("✅ ML visualization active");
    Ok(())
}

/// ML learning simulation loop
async fn ml_learning_simulation_loop(visualizer: &MLVisualizer, simulation: &SimulationState) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    let mut model_version = 1u32;
    
    while simulation.running.load(Ordering::Relaxed) {
        interval.tick().await;
        
        // Simulate ML learning events
        simulate_ml_learning_events(visualizer, model_version).await;
        
        // Update prediction accuracy
        simulate_accuracy_improvement(visualizer, model_version).await;
        
        // Trigger tier migrations based on learned patterns
        simulate_tier_migrations(visualizer).await;
        
        model_version += 1;
    }
}

/// Simulate realistic ML learning events
async fn simulate_ml_learning_events(visualizer: &MLVisualizer, model_version: u32) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    
    // Generate 1-3 ML events per cycle
    let event_count = rng.gen_range(1..=3);
    
    for _ in 0..event_count {
        let event_type = match rng.gen_range(0..5) {
            0 => MLEventType::PatternDetected,
            1 => MLEventType::TierMigration,
            2 => MLEventType::ModelUpdate,
            3 => MLEventType::PredictionCorrection,
            _ => MLEventType::FeatureDiscovery,
        };
        
        let (description, confidence) = match event_type {
            MLEventType::PatternDetected => {
                let patterns = vec![
                    "High-frequency access detected for product category 'Electronics'",
                    "Temporal pattern discovered: daily analytics reports at 9 AM",
                    "Burst pattern identified: flash sale triggered 10x normal access",
                    "Seasonal pattern emerging: weekend shopping behavior differs",
                ];
                (patterns.choose(&mut rng).unwrap().to_string(), rng.gen_range(0.7..0.95))
            },
            MLEventType::TierMigration => {
                let product_id = rng.gen_range(1000..9999);
                (format!("Promoted product:{} to Hot tier (confidence: high)", product_id), rng.gen_range(0.8..0.98))
            },
            MLEventType::ModelUpdate => {
                (format!("Neural network weights updated (v{})", model_version), rng.gen_range(0.6..0.9))
            },
            MLEventType::PredictionCorrection => {
                (format!("Corrected false positive: product:{} returned to Warm tier", rng.gen_range(1000..9999)), rng.gen_range(0.5..0.8))
            },
            MLEventType::FeatureDiscovery => {
                let features = vec!["user geography correlation", "time-of-day patterns", "cart abandonment signals"];
                (format!("New predictive feature discovered: {}", features.choose(&mut rng).unwrap()), rng.gen_range(0.6..0.9))
            },
        };

        let impact = match event_type {
            MLEventType::PatternDetected => "Optimization opportunity identified",
            MLEventType::TierMigration => "Cache performance improved", 
            MLEventType::ModelUpdate => "Prediction accuracy enhanced",
            MLEventType::PredictionCorrection => "Reduced false positives",
            MLEventType::FeatureDiscovery => "Model capability expanded",
        }.to_string();

        let event = MLEvent {
            timestamp: Instant::now(),
            event_type,
            description,
            confidence,
            impact,
        };
        
        visualizer.record_ml_event(event);
    }
}

/// Simulate prediction accuracy improvement over time
async fn simulate_accuracy_improvement(visualizer: &MLVisualizer, model_version: u32) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    
    // Simulate learning curve - accuracy improves but with some noise
    let base_accuracy = 0.5 + (model_version as f64 * 0.005).min(0.4); // 50% -> 90% over 80 versions
    let noise = rng.gen_range(-0.03..0.02); // Small random variations
    let accuracy = (base_accuracy + noise).clamp(0.5, 0.95);
    
    let measurement = AccuracyMeasurement {
        timestamp: Instant::now(),
        prediction_accuracy: accuracy,
        false_positive_rate: rng.gen_range(0.02..0.08),
        false_negative_rate: rng.gen_range(0.03..0.07),
        model_version,
    };
    
    let mut history = visualizer.prediction_accuracy_history.lock().unwrap();
    if history.len() >= 1000 {
        history.pop_front();
    }
    history.push_back(measurement);
}

/// Simulate tier migrations based on ML decisions
async fn simulate_tier_migrations(visualizer: &MLVisualizer) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    
    // Simulate 0-5 tier migrations per cycle
    let migration_count = rng.gen_range(0..=5);
    
    for _ in 0..migration_count {
        let product_id = rng.gen_range(1..=10000);
        let key = format!("product:{}", product_id);
        
        // Choose realistic tier transitions
        let (from_tier, to_tier, reason) = match rng.gen_range(0..6) {
            0 => ("Cold".to_string(), "Warm".to_string(), "Increasing access frequency detected".to_string()),
            1 => ("Warm".to_string(), "Hot".to_string(), "High-frequency access pattern confirmed".to_string()),
            2 => ("Hot".to_string(), "Warm".to_string(), "Access frequency decreased".to_string()),
            3 => ("Warm".to_string(), "Cold".to_string(), "Long period of inactivity".to_string()),
            4 => ("Cold".to_string(), "Hot".to_string(), "Sudden burst of activity (flash sale)".to_string()),
            _ => ("Hot".to_string(), "Cold".to_string(), "Pattern changed: one-time access spike".to_string()),
        };
        
        visualizer.record_tier_migration(key, from_tier, to_tier, reason);
    }
}

/// Format relative time for display
fn format_relative_time(timestamp: Instant) -> String {
    let elapsed = timestamp.elapsed();
    
    if elapsed.as_secs() < 60 {
        format!("{}s ago", elapsed.as_secs())
    } else if elapsed.as_secs() < 3600 {
        format!("{}m ago", elapsed.as_secs() / 60)
    } else {
        format!("{}h ago", elapsed.as_secs() / 3600)
    }
}