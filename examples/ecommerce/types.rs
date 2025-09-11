//! E-commerce data structures and types
//! 
//! This module contains all data structures used in the e-commerce example,
//! including products, sessions, analytics, and node representations.

use goldylox::prelude::*;
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

// CacheValue implementations for all types
impl CacheValue for Product {
    type Metadata = CacheValueMetadata;

    fn estimated_size(&self) -> usize {
        std::mem::size_of::<Product>() + self.name.len() + self.description.len() + self.category.len()
    }

    fn is_expensive(&self) -> bool {
        self.price > 100.0 // High-value products are more expensive to compute
    }

    fn compression_hint(&self) -> CompressionHint {
        if self.description.len() > 512 {
            CompressionHint::Auto
        } else {
            CompressionHint::Disable
        }
    }

    fn metadata(&self) -> Self::Metadata {
        CacheValueMetadata::from_cache_value(self)
    }
}

impl CacheValue for UserSession {
    type Metadata = CacheValueMetadata;

    fn estimated_size(&self) -> usize {
        std::mem::size_of::<UserSession>() + self.session_id.len() + std::mem::size_of::<u64>() + 
        self.location.len() + (self.shopping_cart.len() * std::mem::size_of::<CartItem>()) +
        self.browsing_history.len() * std::mem::size_of::<u64>() +
        self.preferences.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>()
    }

    fn is_expensive(&self) -> bool {
        self.shopping_cart.len() > 10 // Sessions with large carts are expensive to recreate
    }

    fn compression_hint(&self) -> CompressionHint {
        if self.shopping_cart.len() > 50 {
            CompressionHint::Auto
        } else {
            CompressionHint::Disable
        }
    }

    fn metadata(&self) -> Self::Metadata {
        CacheValueMetadata::from_cache_value(self)
    }
}

impl CacheValue for AnalyticsEvent {
    type Metadata = CacheValueMetadata;

    fn estimated_size(&self) -> usize {
        std::mem::size_of::<AnalyticsEvent>() + self.event_type.len() + std::mem::size_of::<u64>() +
        self.properties.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>() +
        self.raw_data.len() + self.event_id.len() + self.session_id.len()
    }

    fn is_expensive(&self) -> bool {
        self.properties.len() > 20 // Events with lots of data are expensive
    }

    fn compression_hint(&self) -> CompressionHint {
        if self.properties.len() > 10 {
            CompressionHint::Auto
        } else {
            CompressionHint::Disable
        }
    }

    fn metadata(&self) -> Self::Metadata {
        CacheValueMetadata::from_cache_value(self)
    }
}