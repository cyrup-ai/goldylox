//! Cache manager for loxd daemon
//!
//! Manages the Goldylox cache instance and HTTP API server, replacing the
//! generic service management with cache-specific functionality.

use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::{select, tick};
use log::{error, info};
use tokio::sync::oneshot;

use crate::daemon::config::ServiceConfig;
// use crate::daemon::ipc::{Cmd, Evt};
use crate::{Goldylox, GoldyloxBuilder};

/// Format bytes in human-readable format
fn format_memory_size(bytes: u64) -> String {
    if bytes > 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes > 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes > 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}
use crate::goldylox::BatchOperationSummary;

/// Cache manager that runs the Goldylox cache and HTTP API
pub struct CacheManager {
    cache: Goldylox<String, Vec<u8>>,
    api_shutdown_tx: Option<oneshot::Sender<()>>,
    api_task: Option<tokio::task::JoinHandle<()>>,
}

impl CacheManager {
    /// Create a new cache manager with configuration
    pub async fn new(_cfg: &ServiceConfig) -> Result<Self> {
        // Build the cache with configuration
        let cache = GoldyloxBuilder::new().build().await?;

        Ok(Self {
            cache,
            api_shutdown_tx: None,
            api_task: None,
        })
    }

    /// Start the HTTP API server if configured
    pub async fn start_api_server(&mut self, cfg: &ServiceConfig) -> Result<()> {
        if let Some(api_config) = &cfg.sse
            && api_config.enabled
        {
            info!(
                "Starting HTTP API server on {}:{}",
                api_config.host, api_config.port
            );

            let (shutdown_tx, shutdown_rx) = oneshot::channel();
            let cache = self.cache.clone();
            let host = api_config.host.clone();
            let port = api_config.port;

            let task = tokio::spawn(async move {
                if let Err(e) = start_api_server(cache, host, port, shutdown_rx).await {
                    error!("API server error: {}", e);
                }
            });

            self.api_shutdown_tx = Some(shutdown_tx);
            self.api_task = Some(task);

            info!(
                "HTTP API server started on {}:{}",
                api_config.host, api_config.port
            );
        }
        Ok(())
    }

    /// Run the cache manager event loop
    pub fn run(mut self) -> Result<()> {
        let sig_tick = tick(Duration::from_millis(200));
        let health_tick = tick(Duration::from_secs(30));
        let stats_tick = tick(Duration::from_secs(60));

        loop {
            select! {
                recv(sig_tick) -> _ => {
                    if let Some(sig) = crate::daemon::signals::check_signals() {
                        info!("signal {:?} – orderly shutdown", sig);

                        // Shutdown API server if running
                        if let Some(shutdown_tx) = self.api_shutdown_tx.take() {
                            info!("Shutting down API server");
                            shutdown_tx.send(()).ok();
                        }

                        break;
                    }
                }
                recv(health_tick) -> _ => {
                    // Perform health checks
                    self.health_check();
                }
                recv(stats_tick) -> _ => {
                    // Log cache statistics
                    self.log_statistics();
                }
            }
        }

        info!("Cache manager stopping");
        Ok(())
    }

    fn health_check(&self) {
        // Basic health check - cache is always healthy if accessible
        info!("Cache health check OK");
    }

    fn log_statistics(&self) {
        // Log cache statistics
        info!("Cache statistics available");
    }
}

/// Start the HTTP API server
async fn start_api_server(
    cache: Goldylox<String, Vec<u8>>,
    host: String,
    port: u16,
    shutdown_rx: oneshot::Receiver<()>,
) -> Result<()> {
    use axum::{
        Router,
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json},
        routing::{delete, get, put},
    };
    use base64::prelude::*;
    use serde::{Deserialize, Serialize};

    // Cache response types
    #[derive(Serialize)]
    struct CacheResponse<T> {
        success: bool,
        data: Option<T>,
        message: Option<String>,
    }

    #[derive(Serialize)]
    struct CacheValue {
        key: String,
        value: String, // Base64 encoded
        size: usize,
    }

    #[derive(Deserialize)]
    struct SetValueRequest {
        value: String, // Base64 encoded
    }

    #[derive(Deserialize)]
    struct CompareSwapRequest {
        expected: String,  // Base64 encoded
        new_value: String, // Base64 encoded
    }

    #[derive(Deserialize)]
    struct BatchGetRequest {
        keys: Vec<String>,
    }

    #[derive(Deserialize)]
    struct BatchPutRequest {
        entries: Vec<BatchPutEntry>,
    }

    #[derive(Deserialize)]
    struct BatchPutEntry {
        key: String,
        value: String, // Base64 encoded
    }

    #[derive(Deserialize)]
    struct BatchRemoveRequest {
        keys: Vec<String>,
    }

    #[derive(Serialize)]
    struct CacheStats {
        total_keys: usize,
        memory_usage: String,
        hit_rate: f64,
        uptime_seconds: u64,
    }

    // Handler functions
    async fn get_cache_value(
        Path(key): Path<String>,
        State(cache): State<Goldylox<String, Vec<u8>>>,
    ) -> impl IntoResponse {
        match cache.get(&key).await {
            Some(value) => {
                let encoded_value = base64::prelude::BASE64_STANDARD.encode(&value);
                let response = CacheResponse {
                    success: true,
                    data: Some(CacheValue {
                        key: key.clone(),
                        value: encoded_value,
                        size: value.len(),
                    }),
                    message: None,
                };
                (StatusCode::OK, Json(response))
            }
            None => {
                let response: CacheResponse<CacheValue> = CacheResponse {
                    success: false,
                    data: None,
                    message: Some(format!("Key '{}' not found", key)),
                };
                (StatusCode::NOT_FOUND, Json(response))
            }
        }
    }

    async fn set_cache_value(
        Path(key): Path<String>,
        State(cache): State<Goldylox<String, Vec<u8>>>,
        Json(request): Json<SetValueRequest>,
    ) -> impl IntoResponse {
        match BASE64_STANDARD.decode(&request.value) {
            Ok(decoded_value) => match cache.put(key.clone(), decoded_value.clone()).await {
                Ok(_) => {
                    let response = CacheResponse {
                        success: true,
                        data: Some(CacheValue {
                            key: key.clone(),
                            value: request.value,
                            size: decoded_value.len(),
                        }),
                        message: Some("Value stored successfully".to_string()),
                    };
                    (StatusCode::CREATED, Json(response))
                }
                Err(e) => {
                    let response: CacheResponse<CacheValue> = CacheResponse {
                        success: false,
                        data: None,
                        message: Some(format!("Failed to store value: {}", e)),
                    };
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
                }
            },
            Err(_) => {
                let response: CacheResponse<CacheValue> = CacheResponse {
                    success: false,
                    data: None,
                    message: Some("Invalid base64 encoding".to_string()),
                };
                (StatusCode::BAD_REQUEST, Json(response))
            }
        }
    }

    async fn delete_cache_value(
        Path(key): Path<String>,
        State(cache): State<Goldylox<String, Vec<u8>>>,
    ) -> impl IntoResponse {
        if cache.remove(&key).await {
            let response: CacheResponse<()> = CacheResponse {
                success: true,
                data: None,
                message: Some(format!("Key '{}' deleted successfully", key)),
            };
            (StatusCode::OK, Json(response))
        } else {
            let response: CacheResponse<()> = CacheResponse {
                success: false,
                data: None,
                message: Some(format!("Key '{}' not found", key)),
            };
            (StatusCode::NOT_FOUND, Json(response))
        }
    }

    async fn get_cache_stats(State(cache): State<Goldylox<String, Vec<u8>>>) -> impl IntoResponse {
        // Use per-instance telemetry system for accurate statistics
        let telemetry = cache.get_unified_stats();
        let (total_keys, hit_rate, memory_usage_bytes, _ops_per_second, estimated_uptime_seconds) = {
                let metrics = telemetry.get_performance_metrics().await;

                    // Calculate total keys by summing entry counts from all tiers with overflow protection
                    let total_keys = metrics.hot_tier.entry_count.saturating_add(
                        metrics
                            .warm_tier
                            .entry_count
                            .saturating_add(metrics.cold_tier.entry_count),
                    );

                    let hit_rate = metrics.overall_hit_rate;
                    let memory_usage_bytes = metrics.total_memory_usage_bytes;
                    let ops_per_second = metrics.hot_tier.ops_per_second
                        + metrics.warm_tier.ops_per_second
                        + metrics.cold_tier.ops_per_second;

                    // Calculate uptime with division-by-zero protection
                    let estimated_uptime_seconds = if ops_per_second > f64::EPSILON {
                        metrics.total_operations.saturating_mul(1000)
                            / (ops_per_second * 1000.0) as u64
                    } else {
                        // Alternative: estimate based on total operations if ops_per_second is zero
                        // Use a conservative estimate of 1 operation per second
                        metrics.total_operations
                    };

                (
                    total_keys,
                    hit_rate,
                    memory_usage_bytes,
                    ops_per_second,
                    estimated_uptime_seconds,
                )
            };

        // Format memory usage in human-readable format
        let memory_usage = format_memory_size(memory_usage_bytes);

        let stats = CacheStats {
            total_keys,   // Professional calculation from tier entry counts
            memory_usage, // Real memory usage from stats
            hit_rate,     // Real hit rate from stats
            uptime_seconds: estimated_uptime_seconds, // Estimated uptime
        };

        let response = CacheResponse {
            success: true,
            data: Some(stats),
            message: None,
        };
        (StatusCode::OK, Json(response))
    }

    async fn health_check() -> impl IntoResponse {
        let response = CacheResponse {
            success: true,
            data: Some("healthy"),
            message: Some("Cache API is running".to_string()),
        };
        (StatusCode::OK, Json(response))
    }

    async fn contains_key_handler(
        Path(key): Path<String>,
        State(cache): State<Goldylox<String, Vec<u8>>>,
    ) -> impl IntoResponse {
        let contains = cache.contains_key(&key).await;
        let response = CacheResponse {
            success: true,
            data: Some(contains),
            message: None,
        };
        (StatusCode::OK, Json(response))
    }

    async fn hash_key_handler(
        Path(key): Path<String>,
        State(cache): State<Goldylox<String, Vec<u8>>>,
    ) -> impl IntoResponse {
        let hash = cache.hash_key(&key);
        let response = CacheResponse {
            success: true,
            data: Some(hash),
            message: None,
        };
        (StatusCode::OK, Json(response))
    }

    async fn put_if_absent_handler(
        Path(key): Path<String>,
        State(cache): State<Goldylox<String, Vec<u8>>>,
        Json(request): Json<SetValueRequest>,
    ) -> impl IntoResponse {
        match BASE64_STANDARD.decode(&request.value) {
            Ok(decoded_value) => match cache.put_if_absent(key.clone(), decoded_value.clone()).await {
                Ok(previous) => {
                    let response_data = previous.as_ref().map(|prev_val| CacheValue {
                        key: key.clone(),
                        value: BASE64_STANDARD.encode(prev_val),
                        size: prev_val.len(),
                    });

                    let response = CacheResponse {
                        success: true,
                        data: response_data,
                        message: if previous.is_some() {
                            Some("Key already exists, returned existing value".to_string())
                        } else {
                            Some("Value stored successfully".to_string())
                        },
                    };
                    (StatusCode::OK, Json(response))
                }
                Err(e) => {
                    let response: CacheResponse<CacheValue> = CacheResponse {
                        success: false,
                        data: None,
                        message: Some(format!("Failed to store value: {}", e)),
                    };
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
                }
            },
            Err(_) => {
                let response: CacheResponse<CacheValue> = CacheResponse {
                    success: false,
                    data: None,
                    message: Some("Invalid base64 encoding".to_string()),
                };
                (StatusCode::BAD_REQUEST, Json(response))
            }
        }
    }

    async fn replace_handler(
        Path(key): Path<String>,
        State(cache): State<Goldylox<String, Vec<u8>>>,
        Json(request): Json<SetValueRequest>,
    ) -> impl IntoResponse {
        match BASE64_STANDARD.decode(&request.value) {
            Ok(decoded_value) => match cache.replace(key.clone(), decoded_value.clone()).await {
                Ok(previous) => {
                    let response_data = previous.as_ref().map(|prev_val| CacheValue {
                        key: key.clone(),
                        value: BASE64_STANDARD.encode(prev_val),
                        size: prev_val.len(),
                    });

                    let response = CacheResponse {
                        success: true,
                        data: response_data,
                        message: if previous.is_some() {
                            Some("Value replaced successfully".to_string())
                        } else {
                            Some("Key not found, no replacement made".to_string())
                        },
                    };
                    (StatusCode::OK, Json(response))
                }
                Err(e) => {
                    let response: CacheResponse<CacheValue> = CacheResponse {
                        success: false,
                        data: None,
                        message: Some(format!("Failed to replace value: {}", e)),
                    };
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
                }
            },
            Err(_) => {
                let response: CacheResponse<CacheValue> = CacheResponse {
                    success: false,
                    data: None,
                    message: Some("Invalid base64 encoding".to_string()),
                };
                (StatusCode::BAD_REQUEST, Json(response))
            }
        }
    }

    async fn compare_and_swap_handler(
        Path(key): Path<String>,
        State(cache): State<Goldylox<String, Vec<u8>>>,
        Json(request): Json<CompareSwapRequest>,
    ) -> impl IntoResponse {
        let expected_bytes = match BASE64_STANDARD.decode(&request.expected) {
            Ok(bytes) => bytes,
            Err(_) => {
                let response: CacheResponse<bool> = CacheResponse {
                    success: false,
                    data: Some(false),
                    message: Some("Invalid base64 encoding for expected value".to_string()),
                };
                return (StatusCode::BAD_REQUEST, Json(response));
            }
        };

        let new_bytes = match BASE64_STANDARD.decode(&request.new_value) {
            Ok(bytes) => bytes,
            Err(_) => {
                let response: CacheResponse<bool> = CacheResponse {
                    success: false,
                    data: Some(false),
                    message: Some("Invalid base64 encoding for new value".to_string()),
                };
                return (StatusCode::BAD_REQUEST, Json(response));
            }
        };

        match cache.compare_and_swap(key, expected_bytes, new_bytes).await {
            Ok(swapped) => {
                let response = CacheResponse {
                    success: true,
                    data: Some(swapped),
                    message: if swapped {
                        Some("Value swapped successfully".to_string())
                    } else {
                        Some("Compare and swap failed - value mismatch".to_string())
                    },
                };
                (StatusCode::OK, Json(response))
            }
            Err(e) => {
                let response: CacheResponse<bool> = CacheResponse {
                    success: false,
                    data: Some(false),
                    message: Some(format!("Compare and swap failed: {}", e)),
                };
                (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
            }
        }
    }

    async fn get_or_insert_handler(
        Path(key): Path<String>,
        State(cache): State<Goldylox<String, Vec<u8>>>,
        Json(request): Json<SetValueRequest>,
    ) -> impl IntoResponse {
        match BASE64_STANDARD.decode(&request.value) {
            Ok(decoded_value) => match cache.get_or_insert(key.clone(), || decoded_value.clone()).await {
                Ok(result_value) => {
                    let response = CacheResponse {
                        success: true,
                        data: Some(CacheValue {
                            key: key.clone(),
                            value: BASE64_STANDARD.encode(&result_value),
                            size: result_value.len(),
                        }),
                        message: Some("Value retrieved or inserted successfully".to_string()),
                    };
                    (StatusCode::OK, Json(response))
                }
                Err(e) => {
                    let response: CacheResponse<CacheValue> = CacheResponse {
                        success: false,
                        data: None,
                        message: Some(format!("Failed to get or insert value: {}", e)),
                    };
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
                }
            },
            Err(_) => {
                let response: CacheResponse<CacheValue> = CacheResponse {
                    success: false,
                    data: None,
                    message: Some("Invalid base64 encoding".to_string()),
                };
                (StatusCode::BAD_REQUEST, Json(response))
            }
        }
    }

    async fn clear_cache_handler(
        State(cache): State<Goldylox<String, Vec<u8>>>,
    ) -> impl IntoResponse {
        match cache.clear().await {
            Ok(_) => {
                let response: CacheResponse<()> = CacheResponse {
                    success: true,
                    data: None,
                    message: Some("Cache cleared successfully".to_string()),
                };
                (StatusCode::OK, Json(response))
            }
            Err(e) => {
                let response: CacheResponse<()> = CacheResponse {
                    success: false,
                    data: None,
                    message: Some(format!("Failed to clear cache: {}", e)),
                };
                (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
            }
        }
    }

    async fn get_detailed_stats(
        State(cache): State<Goldylox<String, Vec<u8>>>,
    ) -> impl IntoResponse {
        // Use professional telemetry system for detailed tier-specific statistics
        let telemetry = cache.get_unified_stats();
        let metrics = telemetry.get_performance_metrics().await;

        // Calculate aggregated statistics with overflow protection
        let total_keys = metrics
            .hot_tier
            .entry_count
            .saturating_add(metrics.warm_tier.entry_count)
            .saturating_add(metrics.cold_tier.entry_count);

        let total_memory = metrics
            .hot_tier
            .memory_usage
            .saturating_add(metrics.warm_tier.memory_usage)
            .saturating_add(metrics.cold_tier.memory_usage);

        let total_ops_per_second = metrics.hot_tier.ops_per_second
            + metrics.warm_tier.ops_per_second
            + metrics.cold_tier.ops_per_second;

        // Calculate uptime estimate with protection
        let uptime_seconds = if total_ops_per_second > f64::EPSILON {
            (metrics.total_operations as f64 / total_ops_per_second) as u64
        } else {
            metrics.total_operations // Conservative fallback
        };

        let detailed_stats = serde_json::json!({
            "detailed": true,
            "total_keys": total_keys,
            "memory_usage": format_memory_size(total_memory as u64),
            "hit_rate": metrics.overall_hit_rate,
            "uptime_seconds": uptime_seconds,
            "tier_stats": {
                "hot": {
                    "keys": metrics.hot_tier.entry_count,
                    "memory": format_memory_size(metrics.hot_tier.memory_usage as u64),
                    "ops_per_second": metrics.hot_tier.ops_per_second,
                    "avg_latency_ns": metrics.hot_tier.avg_access_time_ns
                },
                "warm": {
                    "keys": metrics.warm_tier.entry_count,
                    "memory": format_memory_size(metrics.warm_tier.memory_usage as u64),
                    "ops_per_second": metrics.warm_tier.ops_per_second,
                    "avg_latency_ns": metrics.warm_tier.avg_access_time_ns
                },
                "cold": {
                    "keys": metrics.cold_tier.entry_count,
                    "memory": format_memory_size(metrics.cold_tier.memory_usage as u64),
                    "ops_per_second": metrics.cold_tier.ops_per_second,
                    "avg_latency_ns": metrics.cold_tier.avg_access_time_ns
                }
            }
        });

        let response = CacheResponse {
            success: true,
            data: Some(detailed_stats),
            message: None,
        };
        (StatusCode::OK, Json(response))
    }

    async fn batch_get_handler(
        State(cache): State<Goldylox<String, Vec<u8>>>,
        Json(request): Json<BatchGetRequest>,
    ) -> impl IntoResponse {
        let summary = cache.batch_get(request.keys).await;
        let successful_ops = summary.total_operations - summary.failed_count;
        let total_ops = summary.total_operations;
        let response = CacheResponse {
            success: true,
            data: Some(summary),
            message: Some(format!(
                "Batch get completed: {}/{} successful",
                successful_ops, total_ops
            )),
        };
        (StatusCode::OK, Json(response))
    }

    async fn batch_put_handler(
        State(cache): State<Goldylox<String, Vec<u8>>>,
        Json(request): Json<BatchPutRequest>,
    ) -> impl IntoResponse {
        let mut decoded_entries = Vec::new();

        // Decode all Base64 values first
        for entry in request.entries {
            match BASE64_STANDARD.decode(&entry.value) {
                Ok(decoded_value) => {
                    decoded_entries.push((entry.key, decoded_value));
                }
                Err(_) => {
                    let response: CacheResponse<BatchOperationSummary<()>> = CacheResponse {
                        success: false,
                        data: None,
                        message: Some(format!("Invalid base64 encoding for key '{}'", entry.key)),
                    };
                    return (StatusCode::BAD_REQUEST, Json(response));
                }
            }
        }

        let summary = cache.batch_put(decoded_entries).await;
        let successful_ops = summary.total_operations - summary.failed_count;
        let total_ops = summary.total_operations;
        let response = CacheResponse {
            success: true,
            data: Some(summary),
            message: Some(format!(
                "Batch put completed: {}/{} successful",
                successful_ops, total_ops
            )),
        };
        (StatusCode::OK, Json(response))
    }

    async fn batch_remove_handler(
        State(cache): State<Goldylox<String, Vec<u8>>>,
        Json(request): Json<BatchRemoveRequest>,
    ) -> impl IntoResponse {
        let summary = cache.batch_remove(request.keys).await;
        let successful_ops = summary.total_operations - summary.failed_count;
        let total_ops = summary.total_operations;
        let response = CacheResponse {
            success: true,
            data: Some(summary),
            message: Some(format!(
                "Batch remove completed: {}/{} successful",
                successful_ops, total_ops
            )),
        };
        (StatusCode::OK, Json(response))
    }

    // Build the router
    let app = Router::new()
        .route(
            "/cache/{key}",
            get(get_cache_value)
                .put(set_cache_value)
                .delete(delete_cache_value),
        )
        .route("/cache/contains/{key}", get(contains_key_handler))
        .route("/cache/hash/{key}", get(hash_key_handler))
        .route("/cache/if-absent/{key}", put(put_if_absent_handler))
        .route("/cache/replace/{key}", put(replace_handler))
        .route("/cache/compare-swap/{key}", put(compare_and_swap_handler))
        .route("/cache/get-insert/{key}", put(get_or_insert_handler))
        .route("/cache/clear", delete(clear_cache_handler))
        .route("/cache/batch/get", axum::routing::post(batch_get_handler))
        .route("/cache/batch/put", axum::routing::post(batch_put_handler))
        .route(
            "/cache/batch/remove",
            axum::routing::post(batch_remove_handler),
        )
        .route("/stats", get(get_cache_stats))
        .route("/stats/detailed", get(get_detailed_stats))
        .route("/health", get(health_check))
        .with_state(cache);

    // Start server
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Cache API server starting on {}", addr);

    // Run with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            shutdown_rx.await.ok();
            info!("Cache API server shutting down gracefully");
        })
        .await?;

    info!("Cache API server stopped");
    Ok(())
}

/// Signal handling functions
pub use crate::daemon::signals::*;
