//! E-commerce data structures and types
//!
//! This module contains all data structures used in the e-commerce example,
//! including products, sessions, analytics, and node representations.

use crossbeam_channel::{Receiver, Sender};
use goldylox::prelude::*;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64};
use tokio::sync::oneshot;

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

/// Cache operation commands for worker communication
#[allow(dead_code)]
pub enum CacheCommand {
    ProductGet {
        key: String,
        response: oneshot::Sender<Option<Product>>,
    },
    ProductPut {
        key: String,
        value: Product,
        response: oneshot::Sender<Result<(), String>>,
    },
    SessionGet {
        key: String,
        response: oneshot::Sender<Option<UserSession>>,
    },
    SessionPut {
        key: String,
        value: UserSession,
        response: oneshot::Sender<Result<(), String>>,
    },
    AnalyticsGet {
        key: String,
        response: oneshot::Sender<Option<AnalyticsEvent>>,
    },
    AnalyticsPut {
        key: String,
        value: AnalyticsEvent,
        response: oneshot::Sender<Result<(), String>>,
    },
    GetStats {
        response: oneshot::Sender<String>,
    },
    Shutdown,
}

/// Cache worker that owns cache instances (currently unused in examples)
#[allow(dead_code)]
#[cfg(feature = "worker_based_cache")]
pub struct CacheWorker {
    receiver: Receiver<CacheCommand>,
    product_cache: Goldylox<String, Product>,
    session_cache: Goldylox<String, UserSession>,
    analytics_cache: Goldylox<String, AnalyticsEvent>,
}

#[cfg(feature = "worker_based_cache")]
impl CacheWorker {
    pub fn new(receiver: Receiver<CacheCommand>) -> Result<Self, Box<dyn std::error::Error>> {
        // Create separate builders for each cache type
        let product_config = GoldyloxBuilder::<String, Product>::new()
            .hot_tier_max_entries(10000)
            .warm_tier_max_entries(50000)
            .cold_tier_base_dir("./cache/products");

        let session_config = GoldyloxBuilder::<String, UserSession>::new()
            .hot_tier_max_entries(5000)
            .warm_tier_max_entries(25000)
            .cold_tier_base_dir("./cache/sessions");

        let analytics_config = GoldyloxBuilder::<String, AnalyticsEvent>::new()
            .hot_tier_max_entries(15000)
            .warm_tier_max_entries(75000)
            .cold_tier_base_dir("./cache/analytics");

        // Use tokio runtime handle to build caches synchronously in this context
        let rt = tokio::runtime::Handle::current();
        Ok(Self {
            receiver,
            product_cache: rt.block_on(product_config.build())?,
            session_cache: rt.block_on(session_config.build())?,
            analytics_cache: rt.block_on(analytics_config.build())?,
        })
    }

    pub fn run(self) {
        let mut panic_count = 0;
        const MAX_CONSECUTIVE_PANICS: u32 = 5;
        let mut should_shutdown = false;
        let rt = tokio::runtime::Handle::current();

        loop {
            if should_shutdown {
                log::info!("Cache worker shutting down gracefully");
                break;
            }

            match self.receiver.recv() {
                Ok(command) => {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        match command {
                            CacheCommand::ProductGet { key, response } => {
                                let result = rt.block_on(self.product_cache.get(&key));
                                if let Err(e) = response.send(result) {
                                    log::warn!("Failed to send ProductGet response: {:?}", e);
                                }
                            }
                            CacheCommand::ProductPut {
                                key,
                                value,
                                response,
                            } => {
                                let result = rt.block_on(self.product_cache.put(key, value))
                                    .map_err(|e| e.to_string());
                                if let Err(e) = response.send(result) {
                                    log::warn!("Failed to send ProductPut response: {:?}", e);
                                }
                            }
                            CacheCommand::SessionGet { key, response } => {
                                let result = rt.block_on(self.session_cache.get(&key));
                                if let Err(e) = response.send(result) {
                                    log::warn!("Failed to send SessionGet response: {:?}", e);
                                }
                            }
                            CacheCommand::SessionPut {
                                key,
                                value,
                                response,
                            } => {
                                let result = rt.block_on(self.session_cache.put(key, value))
                                    .map_err(|e| e.to_string());
                                if let Err(e) = response.send(result) {
                                    log::warn!("Failed to send SessionPut response: {:?}", e);
                                }
                            }
                            CacheCommand::AnalyticsGet { key, response } => {
                                let result = rt.block_on(self.analytics_cache.get(&key));
                                if let Err(e) = response.send(result) {
                                    log::warn!("Failed to send AnalyticsGet response: {:?}", e);
                                }
                            }
                            CacheCommand::AnalyticsPut {
                                key,
                                value,
                                response,
                            } => {
                                let result = rt.block_on(self.analytics_cache.put(key, value))
                                    .map_err(|e| e.to_string());
                                if let Err(e) = response.send(result) {
                                    log::warn!("Failed to send AnalyticsPut response: {:?}", e);
                                }
                            }
                            CacheCommand::GetStats { response } => {
                                let product_stats = self
                                    .product_cache
                                    .stats()
                                    .unwrap_or_else(|_| "error".to_string());
                                let session_stats = self
                                    .session_cache
                                    .stats()
                                    .unwrap_or_else(|_| "error".to_string());
                                let analytics_stats = self
                                    .analytics_cache
                                    .stats()
                                    .unwrap_or_else(|_| "error".to_string());
                                let combined_stats = format!(
                                    "Product: {}, Session: {}, Analytics: {}",
                                    product_stats, session_stats, analytics_stats
                                );
                                if let Err(e) = response.send(combined_stats) {
                                    log::warn!("Failed to send GetStats response: {}", e);
                                }
                            }
                            CacheCommand::Shutdown => {
                                should_shutdown = true;
                            }
                        }
                    }));

                    match result {
                        Ok(_) => {
                            panic_count = 0;
                        }
                        Err(panic_payload) => {
                            panic_count += 1;
                            log::error!(
                                "Cache worker command processing panicked: {:?} (consecutive: {}/{})",
                                panic_payload, panic_count, MAX_CONSECUTIVE_PANICS
                            );

                            if panic_count >= MAX_CONSECUTIVE_PANICS {
                                log::error!("Max consecutive panics reached, shutting down worker");
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Cache worker receiver error: {}", e);
                    break;
                }
            }
        }
        log::info!("Cache worker thread terminated");
    }
}

/// Cache node with messaging interface (no shared ownership)
#[derive(Debug, Clone)]
pub struct CacheNode {
    pub node_id: String,
    pub location: String,
    #[allow(dead_code)]
    pub command_sender: Sender<CacheCommand>,
    pub product_cache: goldylox::Goldylox<String, Product>,
    pub session_cache: goldylox::Goldylox<String, UserSession>,
    pub analytics_cache: goldylox::Goldylox<String, AnalyticsEvent>,
}

/// Global workload execution state (no Arc usage)
#[derive(Debug)]
pub struct WorkloadState {
    pub nodes: Vec<CacheNode>,
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
        std::mem::size_of::<Product>()
            + self.name.len()
            + self.description.len()
            + self.category.len()
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
        std::mem::size_of::<UserSession>()
            + self.session_id.len()
            + std::mem::size_of::<u64>()
            + self.location.len()
            + (self.shopping_cart.len() * std::mem::size_of::<CartItem>())
            + self.browsing_history.len() * std::mem::size_of::<u64>()
            + self
                .preferences
                .iter()
                .map(|(k, v)| k.len() + v.len())
                .sum::<usize>()
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
        std::mem::size_of::<AnalyticsEvent>()
            + self.event_type.len()
            + std::mem::size_of::<u64>()
            + self
                .properties
                .iter()
                .map(|(k, v)| k.len() + v.len())
                .sum::<usize>()
            + self.raw_data.len()
            + self.event_id.len()
            + self.session_id.len()
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
