//! HTTP client for communicating with loxd daemon
//!
//! This module provides a high-performance HTTP client that communicates
//! with the loxd system daemon for all cache operations.

use std::time::Duration;

use base64::prelude::*;
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};

use crate::cli::errors::{CliError, CliResult};
use crate::goldylox::BatchOperationSummary;

/// HTTP client for daemon communication
#[derive(Debug, Clone)]
pub struct DaemonClient {
    client: Client,
    base_url: String,
}

/// Response wrapper from daemon HTTP API
#[derive(Debug, Deserialize)]
struct CacheResponse<T> {
    success: bool,
    data: Option<T>,
    message: Option<String>,
}

/// Cache value response from daemon
#[derive(Debug, Deserialize)]
struct CacheValue {
    value: String, // Base64 encoded
}

/// Request for setting values
#[derive(Debug, Serialize)]
struct SetValueRequest {
    value: String, // Base64 encoded
}

/// Request for compare and swap
#[derive(Debug, Serialize)]
struct CompareSwapRequest {
    expected: String,  // Base64 encoded
    new_value: String, // Base64 encoded
}

/// Batch get request
#[derive(Debug, Serialize)]
struct BatchGetRequest {
    keys: Vec<String>,
}

/// Batch put request
#[derive(Debug, Serialize)]
struct BatchPutRequest {
    entries: Vec<BatchPutEntry>,
}

/// Entry for batch put operations
#[derive(Debug, Serialize)]
struct BatchPutEntry {
    key: String,
    value: String, // Base64 encoded
}

/// Batch remove request
#[derive(Debug, Serialize)]
struct BatchRemoveRequest {
    keys: Vec<String>,
}

impl DaemonClient {
    /// Create a new daemon client
    pub fn new(base_url: String, timeout_ms: Option<u64>) -> CliResult<Self> {
        let timeout = Duration::from_millis(timeout_ms.unwrap_or(5000));

        let client = ClientBuilder::new()
            .timeout(timeout)
            .connect_timeout(Duration::from_millis(2000))
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(10)
            .tcp_keepalive(Duration::from_secs(60))
            .http2_prior_knowledge()
            .build()
            .map_err(|e| CliError::NetworkError(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { client, base_url })
    }

    /// Check if daemon is healthy
    pub async fn health_check(&self) -> CliResult<bool> {
        let url = format!("{}/health", self.base_url);

        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let health_response: CacheResponse<String> =
                        response.json().await.map_err(|e| {
                            CliError::NetworkError(format!(
                                "Failed to parse health response: {}",
                                e
                            ))
                        })?;
                    Ok(health_response.success)
                } else {
                    Ok(false)
                }
            }
            Err(_) => Ok(false),
        }
    }

    /// Get a value from cache
    pub async fn get(&self, key: &str) -> CliResult<Option<String>> {
        let url = format!("{}/cache/{}", self.base_url, urlencoding::encode(key));

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| CliError::NetworkError(format!("GET request failed: {}", e)))?;

        if response.status() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "GET failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<CacheValue> = response
            .json()
            .await
            .map_err(|e| CliError::NetworkError(format!("Failed to parse GET response: {}", e)))?;

        if !cache_response.success {
            return Ok(None);
        }

        if let Some(cache_value) = cache_response.data {
            let decoded = BASE64_STANDARD
                .decode(&cache_value.value)
                .map_err(|e| CliError::NetworkError(format!("Failed to decode value: {}", e)))?;
            let value_str = String::from_utf8(decoded)
                .map_err(|e| CliError::NetworkError(format!("Invalid UTF-8 in value: {}", e)))?;
            Ok(Some(value_str))
        } else {
            Ok(None)
        }
    }

    /// Put a value into cache
    pub async fn put(&self, key: &str, value: &str) -> CliResult<()> {
        let url = format!("{}/cache/{}", self.base_url, urlencoding::encode(key));
        let encoded_value = BASE64_STANDARD.encode(value.as_bytes());

        let request = SetValueRequest {
            value: encoded_value,
        };

        let response = self
            .client
            .put(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| CliError::NetworkError(format!("PUT request failed: {}", e)))?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "PUT failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<CacheValue> = response
            .json()
            .await
            .map_err(|e| CliError::NetworkError(format!("Failed to parse PUT response: {}", e)))?;

        if !cache_response.success {
            return Err(CliError::NetworkError(format!(
                "PUT operation failed: {}",
                cache_response.message.unwrap_or_default()
            )));
        }

        Ok(())
    }

    /// Remove a value from cache
    pub async fn remove(&self, key: &str) -> CliResult<bool> {
        let url = format!("{}/cache/{}", self.base_url, urlencoding::encode(key));

        let response = self
            .client
            .delete(&url)
            .send()
            .await
            .map_err(|e| CliError::NetworkError(format!("DELETE request failed: {}", e)))?;

        if response.status() == 404 {
            return Ok(false);
        }

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "DELETE failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<()> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!("Failed to parse DELETE response: {}", e))
        })?;

        Ok(cache_response.success)
    }

    /// Clear all cache entries
    pub async fn clear(&self) -> CliResult<()> {
        let url = format!("{}/cache/clear", self.base_url);

        let response = self
            .client
            .delete(&url)
            .send()
            .await
            .map_err(|e| CliError::NetworkError(format!("CLEAR request failed: {}", e)))?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "CLEAR failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<()> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!("Failed to parse CLEAR response: {}", e))
        })?;

        if !cache_response.success {
            return Err(CliError::NetworkError(format!(
                "CLEAR operation failed: {}",
                cache_response.message.unwrap_or_default()
            )));
        }

        Ok(())
    }

    /// Check if key exists in cache
    pub async fn contains_key(&self, key: &str) -> CliResult<bool> {
        let url = format!(
            "{}/cache/contains/{}",
            self.base_url,
            urlencoding::encode(key)
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| CliError::NetworkError(format!("CONTAINS request failed: {}", e)))?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "CONTAINS failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<bool> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!("Failed to parse CONTAINS response: {}", e))
        })?;

        Ok(cache_response.data.unwrap_or(false))
    }

    /// Get hash of key
    pub async fn hash_key(&self, key: &str) -> CliResult<u64> {
        let url = format!("{}/cache/hash/{}", self.base_url, urlencoding::encode(key));

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| CliError::NetworkError(format!("HASH request failed: {}", e)))?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "HASH failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<u64> = response
            .json()
            .await
            .map_err(|e| CliError::NetworkError(format!("Failed to parse HASH response: {}", e)))?;

        Ok(cache_response.data.unwrap_or(0))
    }

    /// Put if absent
    pub async fn put_if_absent(&self, key: &str, value: &str) -> CliResult<Option<String>> {
        let url = format!(
            "{}/cache/if-absent/{}",
            self.base_url,
            urlencoding::encode(key)
        );
        let encoded_value = BASE64_STANDARD.encode(value.as_bytes());

        let request = SetValueRequest {
            value: encoded_value,
        };

        let response = self
            .client
            .put(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| CliError::NetworkError(format!("PUT_IF_ABSENT request failed: {}", e)))?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "PUT_IF_ABSENT failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<CacheValue> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!("Failed to parse PUT_IF_ABSENT response: {}", e))
        })?;

        if let Some(cache_value) = cache_response.data {
            let decoded = BASE64_STANDARD
                .decode(&cache_value.value)
                .map_err(|e| CliError::NetworkError(format!("Failed to decode value: {}", e)))?;
            let value_str = String::from_utf8(decoded)
                .map_err(|e| CliError::NetworkError(format!("Invalid UTF-8 in value: {}", e)))?;
            Ok(Some(value_str))
        } else {
            Ok(None)
        }
    }

    /// Replace existing value
    pub async fn replace(&self, key: &str, value: &str) -> CliResult<Option<String>> {
        let url = format!(
            "{}/cache/replace/{}",
            self.base_url,
            urlencoding::encode(key)
        );
        let encoded_value = BASE64_STANDARD.encode(value.as_bytes());

        let request = SetValueRequest {
            value: encoded_value,
        };

        let response = self
            .client
            .put(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| CliError::NetworkError(format!("REPLACE request failed: {}", e)))?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "REPLACE failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<CacheValue> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!("Failed to parse REPLACE response: {}", e))
        })?;

        if let Some(cache_value) = cache_response.data {
            let decoded = BASE64_STANDARD
                .decode(&cache_value.value)
                .map_err(|e| CliError::NetworkError(format!("Failed to decode value: {}", e)))?;
            let value_str = String::from_utf8(decoded)
                .map_err(|e| CliError::NetworkError(format!("Invalid UTF-8 in value: {}", e)))?;
            Ok(Some(value_str))
        } else {
            Ok(None)
        }
    }

    /// Compare and swap operation
    pub async fn compare_and_swap(
        &self,
        key: &str,
        expected: &str,
        new_value: &str,
    ) -> CliResult<bool> {
        let url = format!(
            "{}/cache/compare-swap/{}",
            self.base_url,
            urlencoding::encode(key)
        );
        let encoded_expected = BASE64_STANDARD.encode(expected.as_bytes());
        let encoded_new = BASE64_STANDARD.encode(new_value.as_bytes());

        let request = CompareSwapRequest {
            expected: encoded_expected,
            new_value: encoded_new,
        };

        let response = self
            .client
            .put(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                CliError::NetworkError(format!("COMPARE_AND_SWAP request failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "COMPARE_AND_SWAP failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<bool> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!("Failed to parse COMPARE_AND_SWAP response: {}", e))
        })?;

        Ok(cache_response.data.unwrap_or(false))
    }

    /// Get or insert operation
    pub async fn get_or_insert(&self, key: &str, value: &str) -> CliResult<String> {
        let url = format!(
            "{}/cache/get-insert/{}",
            self.base_url,
            urlencoding::encode(key)
        );
        let encoded_value = BASE64_STANDARD.encode(value.as_bytes());

        let request = SetValueRequest {
            value: encoded_value,
        };

        let response = self
            .client
            .put(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| CliError::NetworkError(format!("GET_OR_INSERT request failed: {}", e)))?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "GET_OR_INSERT failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<CacheValue> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!("Failed to parse GET_OR_INSERT response: {}", e))
        })?;

        if let Some(cache_value) = cache_response.data {
            let decoded = BASE64_STANDARD
                .decode(&cache_value.value)
                .map_err(|e| CliError::NetworkError(format!("Failed to decode value: {}", e)))?;
            let value_str = String::from_utf8(decoded)
                .map_err(|e| CliError::NetworkError(format!("Invalid UTF-8 in value: {}", e)))?;
            Ok(value_str)
        } else {
            Err(CliError::NetworkError(
                "GET_OR_INSERT returned no data".to_string(),
            ))
        }
    }

    /// Batch get operation
    pub async fn batch_get(&self, keys: Vec<String>) -> CliResult<BatchOperationSummary<Vec<u8>>> {
        let url = format!("{}/cache/batch/get", self.base_url);

        let request = BatchGetRequest { keys };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| CliError::NetworkError(format!("BATCH_GET request failed: {}", e)))?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "BATCH_GET failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<BatchOperationSummary<Vec<u8>>> =
            response.json().await.map_err(|e| {
                CliError::NetworkError(format!("Failed to parse BATCH_GET response: {}", e))
            })?;

        cache_response
            .data
            .ok_or_else(|| CliError::NetworkError("BATCH_GET returned no data".to_string()))
    }

    /// Batch put operation
    pub async fn batch_put(
        &self,
        entries: Vec<(String, String)>,
    ) -> CliResult<BatchOperationSummary<()>> {
        let url = format!("{}/cache/batch/put", self.base_url);

        let encoded_entries: Vec<BatchPutEntry> = entries
            .into_iter()
            .map(|(key, value)| BatchPutEntry {
                key,
                value: BASE64_STANDARD.encode(value.as_bytes()),
            })
            .collect();

        let request = BatchPutRequest {
            entries: encoded_entries,
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| CliError::NetworkError(format!("BATCH_PUT request failed: {}", e)))?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "BATCH_PUT failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<BatchOperationSummary<()>> =
            response.json().await.map_err(|e| {
                CliError::NetworkError(format!("Failed to parse BATCH_PUT response: {}", e))
            })?;

        cache_response
            .data
            .ok_or_else(|| CliError::NetworkError("BATCH_PUT returned no data".to_string()))
    }

    /// Batch remove operation
    pub async fn batch_remove(&self, keys: Vec<String>) -> CliResult<BatchOperationSummary<bool>> {
        let url = format!("{}/cache/batch/remove", self.base_url);

        let request = BatchRemoveRequest { keys };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| CliError::NetworkError(format!("BATCH_REMOVE request failed: {}", e)))?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "BATCH_REMOVE failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<BatchOperationSummary<bool>> =
            response.json().await.map_err(|e| {
                CliError::NetworkError(format!("Failed to parse BATCH_REMOVE response: {}", e))
            })?;

        cache_response
            .data
            .ok_or_else(|| CliError::NetworkError("BATCH_REMOVE returned no data".to_string()))
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CliResult<String> {
        let url = format!("{}/stats", self.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| CliError::NetworkError(format!("STATS request failed: {}", e)))?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "STATS failed with status {}: {}",
                status_code, error_text
            )));
        }

        response.text().await.map_err(|e| {
            CliError::NetworkError(format!("Failed to get STATS response text: {}", e))
        })
    }

    /// Get detailed cache analytics
    pub async fn detailed_analytics(&self) -> CliResult<String> {
        let url = format!("{}/stats/detailed", self.base_url);

        let response =
            self.client.get(&url).send().await.map_err(|e| {
                CliError::NetworkError(format!("DETAILED_STATS request failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "DETAILED_STATS failed with status {}: {}",
                status_code, error_text
            )));
        }

        response.text().await.map_err(|e| {
            CliError::NetworkError(format!("Failed to get DETAILED_STATS response text: {}", e))
        })
    }

    // ===== COMPRESSION STATISTICS ENDPOINTS =====

    /// Get cold tier space saved by compression
    pub async fn get_cold_tier_space_saved(&self) -> CliResult<u64> {
        let url = format!("{}/compression/space-saved", self.base_url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            CliError::NetworkError(format!("COMPRESSION_SPACE_SAVED request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "COMPRESSION_SPACE_SAVED failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<u64> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!(
                "Failed to parse COMPRESSION_SPACE_SAVED response: {}",
                e
            ))
        })?;

        Ok(cache_response.data.unwrap_or(0))
    }

    /// Get cold tier compression effectiveness ratio
    pub async fn get_cold_tier_compression_effectiveness(&self) -> CliResult<f64> {
        let url = format!("{}/compression/effectiveness", self.base_url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            CliError::NetworkError(format!("COMPRESSION_EFFECTIVENESS request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "COMPRESSION_EFFECTIVENESS failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<f64> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!(
                "Failed to parse COMPRESSION_EFFECTIVENESS response: {}",
                e
            ))
        })?;

        Ok(cache_response.data.unwrap_or(0.0))
    }

    /// Get average cold tier compression time in nanoseconds
    pub async fn get_cold_tier_avg_compression_time(&self) -> CliResult<u64> {
        let url = format!("{}/compression/avg-compression-time", self.base_url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            CliError::NetworkError(format!("AVG_COMPRESSION_TIME request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "AVG_COMPRESSION_TIME failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<u64> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!(
                "Failed to parse AVG_COMPRESSION_TIME response: {}",
                e
            ))
        })?;

        Ok(cache_response.data.unwrap_or(0))
    }

    /// Get average cold tier decompression time in nanoseconds
    pub async fn get_cold_tier_avg_decompression_time(&self) -> CliResult<u64> {
        let url = format!("{}/compression/avg-decompression-time", self.base_url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            CliError::NetworkError(format!("AVG_DECOMPRESSION_TIME request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "AVG_DECOMPRESSION_TIME failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<u64> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!(
                "Failed to parse AVG_DECOMPRESSION_TIME response: {}",
                e
            ))
        })?;

        Ok(cache_response.data.unwrap_or(0))
    }

    /// Get cold tier compression throughput in MB/s
    pub async fn get_cold_tier_compression_throughput(&self) -> CliResult<f64> {
        let url = format!("{}/compression/compression-throughput", self.base_url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            CliError::NetworkError(format!("COMPRESSION_THROUGHPUT request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "COMPRESSION_THROUGHPUT failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<f64> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!(
                "Failed to parse COMPRESSION_THROUGHPUT response: {}",
                e
            ))
        })?;

        Ok(cache_response.data.unwrap_or(0.0))
    }

    /// Get cold tier decompression throughput in MB/s  
    pub async fn get_cold_tier_decompression_throughput(&self) -> CliResult<f64> {
        let url = format!("{}/compression/decompression-throughput", self.base_url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            CliError::NetworkError(format!("DECOMPRESSION_THROUGHPUT request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "DECOMPRESSION_THROUGHPUT failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<f64> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!(
                "Failed to parse DECOMPRESSION_THROUGHPUT response: {}",
                e
            ))
        })?;

        Ok(cache_response.data.unwrap_or(0.0))
    }

    /// Get current cold tier compression algorithm
    pub async fn get_cold_tier_compression_algorithm(&self) -> CliResult<String> {
        let url = format!("{}/compression/algorithm", self.base_url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            CliError::NetworkError(format!("COMPRESSION_ALGORITHM request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "COMPRESSION_ALGORITHM failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<String> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!(
                "Failed to parse COMPRESSION_ALGORITHM response: {}",
                e
            ))
        })?;

        Ok(cache_response.data.unwrap_or_default())
    }

    /// Get comprehensive compression statistics  
    pub async fn get_compression_stats(&self) -> CliResult<String> {
        let url = format!("{}/compression/stats", self.base_url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            CliError::NetworkError(format!("COMPRESSION_STATS request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "COMPRESSION_STATS failed with status {}: {}",
                status_code, error_text
            )));
        }

        response.text().await.map_err(|e| {
            CliError::NetworkError(format!(
                "Failed to get COMPRESSION_STATS response text: {}",
                e
            ))
        })
    }

    // ===== STRATEGY MANAGEMENT ENDPOINTS =====

    /// Get strategy performance metrics
    pub async fn get_strategy_metrics(&self) -> CliResult<String> {
        let url = format!("{}/strategy/metrics", self.base_url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            CliError::NetworkError(format!("STRATEGY_METRICS request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "STRATEGY_METRICS failed with status {}: {}",
                status_code, error_text
            )));
        }

        response.text().await.map_err(|e| {
            CliError::NetworkError(format!(
                "Failed to get STRATEGY_METRICS response text: {}",
                e
            ))
        })
    }

    /// Get strategy thresholds configuration
    pub async fn get_strategy_thresholds(&self) -> CliResult<String> {
        let url = format!("{}/strategy/thresholds", self.base_url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            CliError::NetworkError(format!("STRATEGY_THRESHOLDS request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "STRATEGY_THRESHOLDS failed with status {}: {}",
                status_code, error_text
            )));
        }

        response.text().await.map_err(|e| {
            CliError::NetworkError(format!(
                "Failed to get STRATEGY_THRESHOLDS response text: {}",
                e
            ))
        })
    }

    /// Force a specific cache strategy
    pub async fn force_cache_strategy(&self, strategy: &str) -> CliResult<()> {
        let url = format!("{}/strategy/force", self.base_url);

        #[derive(Serialize)]
        struct ForceStrategyRequest {
            strategy: String,
        }

        let request = ForceStrategyRequest {
            strategy: strategy.to_string(),
        };

        let response = self
            .client
            .put(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| CliError::NetworkError(format!("FORCE_STRATEGY request failed: {}", e)))?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "FORCE_STRATEGY failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<()> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!("Failed to parse FORCE_STRATEGY response: {}", e))
        })?;

        if !cache_response.success {
            return Err(CliError::NetworkError(format!(
                "FORCE_STRATEGY operation failed: {}",
                cache_response.message.unwrap_or_default()
            )));
        }

        Ok(())
    }

    // ===== TASK MANAGEMENT ENDPOINTS =====

    /// Get task coordinator statistics
    pub async fn get_task_coordinator_stats(&self) -> CliResult<String> {
        let url = format!("{}/tasks/coordinator-stats", self.base_url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            CliError::NetworkError(format!("TASK_COORDINATOR_STATS request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "TASK_COORDINATOR_STATS failed with status {}: {}",
                status_code, error_text
            )));
        }

        response.text().await.map_err(|e| {
            CliError::NetworkError(format!(
                "Failed to get TASK_COORDINATOR_STATS response text: {}",
                e
            ))
        })
    }

    /// Get list of active background tasks
    pub async fn get_active_tasks(&self) -> CliResult<String> {
        let url = format!("{}/tasks/active", self.base_url);

        let response =
            self.client.get(&url).send().await.map_err(|e| {
                CliError::NetworkError(format!("ACTIVE_TASKS request failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "ACTIVE_TASKS failed with status {}: {}",
                status_code, error_text
            )));
        }

        response.text().await.map_err(|e| {
            CliError::NetworkError(format!("Failed to get ACTIVE_TASKS response text: {}", e))
        })
    }

    /// Cancel a background task by ID
    pub async fn cancel_task(&self, task_id: u64) -> CliResult<bool> {
        let url = format!("{}/tasks/cancel/{}", self.base_url, task_id);

        let response =
            self.client.delete(&url).send().await.map_err(|e| {
                CliError::NetworkError(format!("CANCEL_TASK request failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "CANCEL_TASK failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<bool> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!("Failed to parse CANCEL_TASK response: {}", e))
        })?;

        Ok(cache_response.data.unwrap_or(false))
    }

    // ===== MAINTENANCE OPERATION ENDPOINTS =====

    /// Get maintenance operation breakdown
    pub async fn get_maintenance_breakdown(&self) -> CliResult<String> {
        let url = format!("{}/maintenance/breakdown", self.base_url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            CliError::NetworkError(format!("MAINTENANCE_BREAKDOWN request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "MAINTENANCE_BREAKDOWN failed with status {}: {}",
                status_code, error_text
            )));
        }

        response.text().await.map_err(|e| {
            CliError::NetworkError(format!(
                "Failed to get MAINTENANCE_BREAKDOWN response text: {}",
                e
            ))
        })
    }

    /// Get maintenance configuration information
    pub async fn get_maintenance_config_info(&self) -> CliResult<String> {
        let url = format!("{}/maintenance/config", self.base_url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            CliError::NetworkError(format!("MAINTENANCE_CONFIG request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "MAINTENANCE_CONFIG failed with status {}: {}",
                status_code, error_text
            )));
        }

        response.text().await.map_err(|e| {
            CliError::NetworkError(format!(
                "Failed to get MAINTENANCE_CONFIG response text: {}",
                e
            ))
        })
    }

    /// Start background processor for maintenance tasks
    pub async fn start_background_processor(&self) -> CliResult<()> {
        let url = format!("{}/maintenance/start", self.base_url);

        let response = self.client.post(&url).send().await.map_err(|e| {
            CliError::NetworkError(format!("START_BACKGROUND_PROCESSOR request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "START_BACKGROUND_PROCESSOR failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<()> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!(
                "Failed to parse START_BACKGROUND_PROCESSOR response: {}",
                e
            ))
        })?;

        if !cache_response.success {
            return Err(CliError::NetworkError(format!(
                "START_BACKGROUND_PROCESSOR operation failed: {}",
                cache_response.message.unwrap_or_default()
            )));
        }

        Ok(())
    }

    // ===== SYSTEM MANAGEMENT ENDPOINTS =====

    /// Start the cache system
    pub async fn start_system(&self) -> CliResult<()> {
        let url = format!("{}/system/start", self.base_url);

        let response =
            self.client.post(&url).send().await.map_err(|e| {
                CliError::NetworkError(format!("START_SYSTEM request failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "START_SYSTEM failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<()> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!("Failed to parse START_SYSTEM response: {}", e))
        })?;

        if !cache_response.success {
            return Err(CliError::NetworkError(format!(
                "START_SYSTEM operation failed: {}",
                cache_response.message.unwrap_or_default()
            )));
        }

        Ok(())
    }

    /// Stop the policy engine
    pub async fn stop_policy_engine(&self) -> CliResult<()> {
        let url = format!("{}/system/stop-policy-engine", self.base_url);

        let response = self.client.post(&url).send().await.map_err(|e| {
            CliError::NetworkError(format!("STOP_POLICY_ENGINE request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "STOP_POLICY_ENGINE failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<()> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!(
                "Failed to parse STOP_POLICY_ENGINE response: {}",
                e
            ))
        })?;

        if !cache_response.success {
            return Err(CliError::NetworkError(format!(
                "STOP_POLICY_ENGINE operation failed: {}",
                cache_response.message.unwrap_or_default()
            )));
        }

        Ok(())
    }

    /// Shutdown the cache system gracefully
    pub async fn shutdown_gracefully(&self) -> CliResult<()> {
        let url = format!("{}/system/shutdown", self.base_url);

        let response = self.client.post(&url).send().await.map_err(|e| {
            CliError::NetworkError(format!("SHUTDOWN_GRACEFULLY request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "SHUTDOWN_GRACEFULLY failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<()> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!(
                "Failed to parse SHUTDOWN_GRACEFULLY response: {}",
                e
            ))
        })?;

        if !cache_response.success {
            return Err(CliError::NetworkError(format!(
                "SHUTDOWN_GRACEFULLY operation failed: {}",
                cache_response.message.unwrap_or_default()
            )));
        }

        Ok(())
    }

    // ===== CONFIGURATION MANAGEMENT ENDPOINTS =====

    /// Show current configuration
    pub async fn get_config(&self) -> CliResult<String> {
        let url = format!("{}/config", self.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| CliError::NetworkError(format!("GET_CONFIG request failed: {}", e)))?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "GET_CONFIG failed with status {}: {}",
                status_code, error_text
            )));
        }

        response.text().await.map_err(|e| {
            CliError::NetworkError(format!("Failed to get GET_CONFIG response text: {}", e))
        })
    }

    /// Update configuration parameter
    pub async fn update_config(&self, parameter: &str, value: &str) -> CliResult<()> {
        let url = format!("{}/config/update", self.base_url);

        #[derive(Serialize)]
        struct ConfigUpdateRequest {
            parameter: String,
            value: String,
        }

        let request = ConfigUpdateRequest {
            parameter: parameter.to_string(),
            value: value.to_string(),
        };

        let response = self
            .client
            .put(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| CliError::NetworkError(format!("UPDATE_CONFIG request failed: {}", e)))?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "UPDATE_CONFIG failed with status {}: {}",
                status_code, error_text
            )));
        }

        let cache_response: CacheResponse<()> = response.json().await.map_err(|e| {
            CliError::NetworkError(format!("Failed to parse UPDATE_CONFIG response: {}", e))
        })?;

        if !cache_response.success {
            return Err(CliError::NetworkError(format!(
                "UPDATE_CONFIG operation failed: {}",
                cache_response.message.unwrap_or_default()
            )));
        }

        Ok(())
    }

    /// Get default configuration as TOML
    pub async fn get_default_config(&self) -> CliResult<String> {
        let url = format!("{}/config/default", self.base_url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            CliError::NetworkError(format!("GET_DEFAULT_CONFIG request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status_code = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(CliError::NetworkError(format!(
                "GET_DEFAULT_CONFIG failed with status {}: {}",
                status_code, error_text
            )));
        }

        response.text().await.map_err(|e| {
            CliError::NetworkError(format!(
                "Failed to get GET_DEFAULT_CONFIG response text: {}",
                e
            ))
        })
    }
}
