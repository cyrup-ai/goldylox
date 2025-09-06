//! E-commerce data structures and types
//! 
//! This module contains all data structures used in the e-commerce example,
//! including products, sessions, analytics, and node representations.

use goldylox::Goldylox;
use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;
use std::sync::{Arc, atomic::{AtomicU64, AtomicBool}};

/// Product in e-commerce catalog - designed for realistic caching patterns
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CartItem {
    pub product_id: u64,
    pub quantity: u32,
    pub added_at: u64,
    pub price_at_add: f64,
}

/// Analytics event - low-frequency, high-volume data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AnalyticsEvent {
    pub event_id: String,
    pub timestamp: u64,
    pub user_id: u64,
    pub session_id: String,
    pub event_type: String,
    pub product_id: Option<u64>,
    pub properties: BTreeMap<String, String>,
    pub raw_data: Vec<u8>,
}

/// Cache node representing a data center
#[derive(Debug)]
pub struct CacheNode {
    pub node_id: String,
    pub location: String,
    pub product_cache: Arc<Goldylox<String, Product>>,
    pub session_cache: Arc<Goldylox<String, UserSession>>,
    pub analytics_cache: Arc<Goldylox<String, AnalyticsEvent>>,
}

/// Global workload execution state
#[derive(Debug)]
pub struct WorkloadState {
    pub nodes: Vec<Arc<CacheNode>>,
    pub start_time: std::time::Instant,
    pub phase: AtomicU64,
    pub running: AtomicBool,
}

/// Get current timestamp in seconds since epoch
pub fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}