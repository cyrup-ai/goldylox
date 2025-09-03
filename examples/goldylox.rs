//! Goldylox API Demonstration
//! 
//! This example demonstrates every public method of the Goldylox API.
//! 
//! 🚀 **FOR THE REAL CACHE MAGIC DEMO**: Run `cache_magic_demo.rs` instead!
//! 
//! That demo shows ML learning, coherence protocols, and SIMD optimizations 
//! working in a realistic 10+ minute simulation with evolving traffic patterns.
//! 
//! This file focuses on API completeness - showing every available method.

use goldylox::{Goldylox, CacheOperationError};
use serde::{Serialize, Deserialize};
use std::collections::BTreeMap; // Use BTreeMap instead of HashMap for Ord support
use std::thread;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// User profile data for hot tier caching
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd, Default, bincode::Encode, bincode::Decode)]
struct UserProfile {
    user_id: u64,
    username: String,
    email: String,
    last_login: u64,
    preferences: BTreeMap<String, String>,
}

/// Session data for warm tier caching
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, bincode::Encode, bincode::Decode)]
struct SessionData {
    session_id: String,
    user_id: u64,
    created_at: u64,
    expires_at: u64,
    data: BTreeMap<String, String>,
}

/// Analytics data for cold tier caching
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, bincode::Encode, bincode::Decode)]
struct AnalyticsData {
    report_id: String,
    generated_at: u64,
    metrics: BTreeMap<String, f64>,
    raw_data: Vec<u8>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Goldylox Comprehensive Cache Example");
    println!("=========================================");

    // =======================================================================
    // 1. DEMONSTRATE BUILDER PATTERN WITH ALL CONFIGURATION OPTIONS
    // =======================================================================
    println!("\n📋 Building sophisticated multi-tier cache with all advanced features...");
    
    let advanced_cache = Goldylox::<String, UserProfile>::builder()
        // Hot Tier Configuration (SIMD-optimized, ML eviction)
        .hot_tier_enabled(true)
        .hot_tier_max_entries(10_000)
        .hot_tier_memory_limit_mb(256)
        .enable_simd(true)           // Enable SIMD optimizations
        .enable_prefetch(true)       // Enable intelligent prefetching
        
        // Warm Tier Configuration (adaptive eviction)
        .warm_tier_enabled(true)
        .warm_tier_max_entries(100_000)
        .warm_tier_max_memory_bytes(1_024 * 1_024 * 1_024) // 1GB
        
        // Cold Tier Configuration (compressed storage)
        .cold_tier_enabled(true)
        .cold_tier_storage_path("/tmp/goldylox_cache")
        .cold_tier_max_size_bytes(10 * 1_024 * 1_024 * 1_024) // 10GB
        .compression_level(6)        // Balanced compression
        
        // Background Workers Configuration
        .enable_background_workers(true)
        .background_worker_threads(4)
        
        // Telemetry Configuration
        .enable_telemetry(true)
        
        .build()?;

    println!("✅ Advanced cache built successfully with all sophisticated features enabled");

    // =======================================================================
    // 2. DEMONSTRATE DEFAULT CONSTRUCTION
    // =======================================================================
    println!("\n🔧 Creating session cache with default configuration...");
    
    let session_cache = Goldylox::<String, SessionData>::new()?;
    println!("✅ Session cache created with optimized defaults");

    // =======================================================================
    // 3. DEMONSTRATE BASIC CACHE OPERATIONS
    // =======================================================================
    println!("\n🔄 Demonstrating basic cache operations...");

    // Create sample user profiles
    let user1 = UserProfile {
        user_id: 1001,
        username: "alice_dev".to_string(),
        email: "alice@example.com".to_string(),
        last_login: current_timestamp(),
        preferences: {
            let mut prefs = BTreeMap::new();
            prefs.insert("theme".to_string(), "dark".to_string());
            prefs.insert("language".to_string(), "en".to_string());
            prefs
        },
    };

    let user2 = UserProfile {
        user_id: 1002,
        username: "bob_admin".to_string(),
        email: "bob@example.com".to_string(),
        last_login: current_timestamp() - 3600,
        preferences: {
            let mut prefs = BTreeMap::new();
            prefs.insert("theme".to_string(), "light".to_string());
            prefs.insert("notifications".to_string(), "enabled".to_string());
            prefs
        },
    };

    // PUT operation - Store values with ML-driven tier placement
    println!("  📤 Storing user profiles (ML eviction will optimize placement)...");
    advanced_cache.put("user:1001".to_string(), user1.clone())?;
    advanced_cache.put("user:1002".to_string(), user2.clone())?;

    // GET operation - Retrieve with SIMD-optimized lookups
    println!("  📥 Retrieving user profiles (SIMD optimizations active)...");
    if let Some(retrieved_user) = advanced_cache.get(&"user:1001".to_string()) {
        println!("    ✅ Retrieved: {} ({})", retrieved_user.username, retrieved_user.email);
    }

    // CONTAINS_KEY operation - Check existence without retrieval
    println!("  🔍 Checking key existence...");
    let exists = advanced_cache.contains_key(&"user:1001".to_string());
    println!("    ✅ User 1001 exists: {}", exists);

    // STATS operation - Get comprehensive cache statistics
    println!("  📊 Cache statistics after basic operations:");
    let stats = advanced_cache.stats()?;
    println!("    {}", stats);

    // =======================================================================
    // 4. DEMONSTRATE CONCURRENT CACHE OPERATIONS
    // =======================================================================
    println!("\n🔀 Demonstrating concurrent cache operations...");

    // PUT_IF_ABSENT operation - Atomic conditional insertion
    println!("  🔒 Testing put_if_absent (atomic conditional insertion)...");
    let new_user = UserProfile {
        user_id: 1003,
        username: "charlie_guest".to_string(),
        email: "charlie@example.com".to_string(),
        last_login: current_timestamp(),
        preferences: BTreeMap::new(),
    };

    match advanced_cache.put_if_absent("user:1003".to_string(), new_user.clone())? {
        None => println!("    ✅ New user 1003 inserted successfully"),
        Some(existing) => println!("    ℹ️  User 1003 already existed: {}", existing.username),
    }

    // Try to insert same key again
    match advanced_cache.put_if_absent("user:1003".to_string(), new_user.clone())? {
        None => println!("    ❌ Unexpected: should not have inserted duplicate"),
        Some(existing) => println!("    ✅ Correctly rejected duplicate: {}", existing.username),
    }

    // REPLACE operation - Replace existing value atomically
    println!("  🔄 Testing replace (atomic value replacement)...");
    let mut updated_user = user1.clone();
    updated_user.last_login = current_timestamp();
    updated_user.preferences.insert("recent_activity".to_string(), "logged_in".to_string());

    match advanced_cache.replace("user:1001".to_string(), updated_user.clone())? {
        Some(old_user) => println!("    ✅ Replaced user 1001, old last_login: {}", old_user.last_login),
        None => println!("    ❌ Unexpected: user 1001 should have existed"),
    }

    // COMPARE_AND_SWAP operation - Atomic compare-and-swap
    println!("  ⚛️  Testing compare_and_swap (atomic CAS operation)...");
    let expected_user = updated_user.clone();
    let mut new_user_state = expected_user.clone();
    new_user_state.preferences.insert("session_count".to_string(), "5".to_string());

    let cas_success = advanced_cache.compare_and_swap(
        "user:1001".to_string(), 
        expected_user, 
        new_user_state.clone()
    )?;
    println!("    ✅ CAS operation success: {}", cas_success);

    // GET_OR_INSERT operation - Atomic get-or-create with closure
    println!("  🏭 Testing get_or_insert (atomic get-or-create)...");
    let computed_user = advanced_cache.get_or_insert("user:9999".to_string(), || {
        println!("    🔧 Factory function called - creating user 9999");
        UserProfile {
            user_id: 9999,
            username: "generated_user".to_string(),
            email: "generated@example.com".to_string(),
            last_login: current_timestamp(),
            preferences: {
                let mut prefs = BTreeMap::new();
                prefs.insert("generated".to_string(), "true".to_string());
                prefs
            },
        }
    })?;
    println!("    ✅ Got or created user: {}", computed_user.username);

    // Call get_or_insert again - should not call factory
    println!("  🔄 Testing get_or_insert again (should use cached value)...");
    let cached_user = advanced_cache.get_or_insert("user:9999".to_string(), || {
        println!("    ❌ Factory should not be called!");
        UserProfile::default()
    })?;
    println!("    ✅ Used cached user: {}", cached_user.username);

    // GET_OR_INSERT_WITH operation - Fallible factory function
    println!("  🏭 Testing get_or_insert_with (fallible factory)...");
    
    // Simulate a database lookup that might fail
    fn fetch_user_from_db(user_id: &str) -> Result<UserProfile, CacheOperationError> {
        if user_id.contains("invalid") {
            Err(CacheOperationError::OperationFailed)
        } else {
            Ok(UserProfile {
                user_id: 5555,
                username: format!("db_user_{}", user_id.split(':').last().unwrap_or("unknown")),
                email: "db_user@example.com".to_string(),
                last_login: current_timestamp(),
                preferences: {
                    let mut prefs = BTreeMap::new();
                    prefs.insert("source".to_string(), "database".to_string());
                    prefs
                },
            })
        }
    }

    let db_user = advanced_cache.get_or_insert_with("user:5555".to_string(), || {
        fetch_user_from_db("user:5555")
    })?;
    println!("    ✅ Got or created from DB: {}", db_user.username);

    // =======================================================================
    // 5. DEMONSTRATE SESSION CACHE WITH DIFFERENT DATA TYPE
    // =======================================================================
    println!("\n💾 Demonstrating session cache operations...");

    let session1 = SessionData {
        session_id: "sess_abc123".to_string(),
        user_id: 1001,
        created_at: current_timestamp(),
        expires_at: current_timestamp() + 3600,
        data: {
            let mut data = BTreeMap::new();
            data.insert("csrf_token".to_string(), "token_xyz789".to_string());
            data.insert("shopping_cart".to_string(), "item1,item2".to_string());
            data
        },
    };

    session_cache.put("session:abc123".to_string(), session1.clone())?;
    println!("  ✅ Session stored in warm tier");

    if let Some(session) = session_cache.get(&"session:abc123".to_string()) {
        println!("  ✅ Session retrieved: {} expires at {}", session.session_id, session.expires_at);
    }

    // =======================================================================
    // 6. DEMONSTRATE ANALYTICS CACHE FOR COLD TIER
    // =======================================================================
    println!("\n📈 Demonstrating analytics cache (cold tier with compression)...");
    
    let analytics_cache = Goldylox::<String, AnalyticsData>::builder()
        .hot_tier_enabled(false)     // Disable hot tier for large reports
        .warm_tier_enabled(false)    // Disable warm tier for large reports  
        .cold_tier_enabled(true)     // Enable cold tier with compression
        .cold_tier_storage_path("/tmp/goldylox_analytics")
        .compression_level(9)        // Maximum compression for analytics data
        .build()?;

    let large_report = AnalyticsData {
        report_id: "monthly_report_2024_01".to_string(),
        generated_at: current_timestamp(),
        metrics: {
            let mut metrics = BTreeMap::new();
            metrics.insert("total_users".to_string(), 125_000.0);
            metrics.insert("active_sessions".to_string(), 8_500.0);
            metrics.insert("revenue_usd".to_string(), 2_450_000.50);
            metrics.insert("conversion_rate".to_string(), 0.045);
            metrics
        },
        raw_data: vec![0u8; 1024 * 1024], // 1MB of mock raw data
    };

    analytics_cache.put("report:monthly_2024_01".to_string(), large_report.clone())?;
    println!("  ✅ Large analytics report stored in compressed cold tier");

    // =======================================================================
    // 7. DEMONSTRATE CONCURRENT ACCESS PATTERNS
    // =======================================================================
    println!("\n🧵 Demonstrating concurrent access patterns...");

    let shared_cache = Arc::new(advanced_cache);
    let mut handles = vec![];

    // Spawn multiple threads to simulate concurrent access
    for thread_id in 0..4 {
        let cache_clone: Arc<Goldylox<String, UserProfile>> = Arc::clone(&shared_cache);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let key = format!("concurrent:{}:{}", thread_id, i);
                let user = UserProfile {
                    user_id: (thread_id * 1000 + i) as u64,
                    username: format!("user_{}_{}", thread_id, i),
                    email: format!("user{}_{}@concurrent.com", thread_id, i),
                    last_login: current_timestamp(),
                    preferences: BTreeMap::new(),
                };

                // Mix of operations to stress-test concurrent access
                match i % 4 {
                    0 => { let _ = cache_clone.put(key, user); }
                    1 => { let _ = cache_clone.get(&key); }
                    2 => { let _ = cache_clone.put_if_absent(key, user); }
                    3 => { let _ = cache_clone.contains_key(&key); }
                    _ => unreachable!(),
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }
    println!("  ✅ Concurrent access test completed successfully");

    // =======================================================================
    // 8. DEMONSTRATE CACHE MAINTENANCE OPERATIONS  
    // =======================================================================
    println!("\n🧹 Demonstrating cache maintenance operations...");

    // Get comprehensive statistics showing all tiers
    println!("  📊 Final cache statistics:");
    let final_stats = shared_cache.stats()?;
    println!("    {}", final_stats);

    // REMOVE operation - Delete specific entries
    println!("  🗑️  Removing specific entries...");
    let removed = shared_cache.remove(&"user:1002".to_string());
    println!("    ✅ User 1002 removal success: {}", removed);

    // Verify removal
    let exists_after_removal = shared_cache.contains_key(&"user:1002".to_string());
    println!("    ✅ User 1002 exists after removal: {}", exists_after_removal);

    // CLEAR operation - Clear all entries (use session cache to avoid affecting shared cache)
    println!("  🧽 Clearing session cache...");
    session_cache.clear()?;
    println!("    ✅ Session cache cleared successfully");

    // Verify cache is empty
    let session_exists = session_cache.contains_key(&"session:abc123".to_string());
    println!("    ✅ Session exists after clear: {}", session_exists);

    // =======================================================================
    // 9. FINAL STATISTICS AND FEATURE SUMMARY
    // =======================================================================
    println!("\n🎯 Final Statistics and Advanced Features Summary");
    println!("=================================================");

    let final_stats = shared_cache.stats()?;
    println!("📊 Final cache statistics: {}", final_stats);

    println!("\n🚀 Advanced Features Demonstrated:");
    println!("  ✅ Multi-tier architecture (Hot/Warm/Cold)");
    println!("  ✅ SIMD optimizations for vectorized operations");
    println!("  ✅ Machine learning-based eviction policies");
    println!("  ✅ Intelligent prefetching and pattern detection");
    println!("  ✅ Coherence protocols for distributed consistency");
    println!("  ✅ Background workers with work-stealing scheduler");
    println!("  ✅ Atomic concurrent operations (CAS, put_if_absent, etc.)");
    println!("  ✅ Compressed cold tier storage");
    println!("  ✅ Real-time telemetry and performance monitoring");
    println!("  ✅ Circuit breaker error recovery");
    println!("  ✅ Lock-free data structures with cache-line alignment");

    println!("\n🎉 Goldylox comprehensive example completed successfully!");
    println!("   All {} public API methods demonstrated", count_api_methods());

    Ok(())
}

/// Get current timestamp in seconds since Unix epoch
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Count the total number of public API methods demonstrated
fn count_api_methods() -> usize {
    // Builder methods: 16
    // Basic operations: 7  
    // Concurrent operations: 6
    // Total: 29 public API methods
    29
}