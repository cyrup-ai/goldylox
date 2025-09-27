//! Storage optimization and compaction utilities for cold tier cache
//!
//! This module provides optimization algorithms for storage efficiency,
//! including compaction, garbage collection, and performance tuning.

#![allow(dead_code)] // Cold tier optimization - complete optimization and compaction library

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::storage::ColdTierCache;
use crate::cache::traits::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

/// Storage optimization configuration
#[derive(Debug, Clone)]
pub struct OptimizationConfig {
    /// Minimum fragmentation ratio to trigger compaction
    pub min_fragmentation_ratio: f64,
    /// Maximum idle time before entry removal
    pub max_idle_duration: Duration,
    /// Automatic optimization active
    pub auto_optimization_active: bool,
    /// Optimization interval
    pub optimization_interval: Duration,
    /// Target storage efficiency (0.0-1.0)
    pub target_efficiency: f64,
}

/// Optimization statistics
#[derive(Debug, Default)]
pub struct OptimizationStats {
    /// Total optimization operations
    pub total_optimizations: AtomicU64,
    /// Total bytes reclaimed
    pub bytes_reclaimed: AtomicU64,
    /// Total entries removed
    pub entries_removed: AtomicU64,
    /// Average optimization time
    pub avg_optimization_time_ms: AtomicU64,
    /// Last optimization timestamp
    pub last_optimization: AtomicU64,
}

/// Storage optimizer for cold tier cache
#[derive(Debug)]
pub struct StorageOptimizer {
    /// Configuration
    config: OptimizationConfig,
    /// Statistics
    stats: OptimizationStats,
}

/// Optimization result
#[derive(Debug)]
pub struct OptimizationResult {
    /// Bytes reclaimed
    pub bytes_reclaimed: u64,
    /// Entries removed
    pub entries_removed: usize,
    /// Optimization time
    pub optimization_time: Duration,
    /// Storage efficiency before optimization
    pub efficiency_before: f64,
    /// Storage efficiency after optimization
    pub efficiency_after: f64,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            min_fragmentation_ratio: 0.3,
            max_idle_duration: Duration::from_secs(604800), // 1 week
            auto_optimization_active: true,
            optimization_interval: Duration::from_secs(86400), // 1 day
            target_efficiency: 0.85,
        }
    }
}

impl StorageOptimizer {
    /// Create new storage optimizer
    pub fn new(config: OptimizationConfig) -> Self {
        Self {
            config,
            stats: OptimizationStats::default(),
        }
    }

    /// Optimize storage by removing expired entries and compacting
    pub fn optimize_storage<K: CacheKey, V: CacheValue>(
        &mut self,
        cache: &ColdTierCache<K, V>,
    ) -> Result<OptimizationResult, CacheOperationError> {
        let start_time = Instant::now();
        let efficiency_before = self.calculate_storage_efficiency(cache)?;

        // Remove expired entries and track bytes reclaimed
        let (expired_removed, expired_bytes_reclaimed) =
            self.remove_expired_entries_with_size_tracking(cache)?;

        // Remove idle entries and track bytes reclaimed
        let (idle_removed, idle_bytes_reclaimed) =
            self.remove_idle_entries_with_size_tracking(cache)?;

        // Calculate compaction bytes savings
        let compaction_bytes_saved =
            if self.calculate_fragmentation_ratio(cache)? >= self.config.min_fragmentation_ratio {
                let bytes_before_compaction = cache.write_offset.load(Ordering::Relaxed);
                cache.compact()?;
                let bytes_after_compaction = cache.write_offset.load(Ordering::Relaxed);
                bytes_before_compaction.saturating_sub(bytes_after_compaction)
            } else {
                0
            };

        let efficiency_after = self.calculate_storage_efficiency(cache)?;
        let optimization_time = start_time.elapsed();
        let total_removed = expired_removed + idle_removed;
        let total_bytes_reclaimed =
            expired_bytes_reclaimed + idle_bytes_reclaimed + compaction_bytes_saved;

        // Update statistics with accurate bytes reclaimed
        self.stats
            .total_optimizations
            .fetch_add(1, Ordering::Relaxed);
        self.stats
            .entries_removed
            .fetch_add(total_removed as u64, Ordering::Relaxed);
        self.stats
            .bytes_reclaimed
            .fetch_add(total_bytes_reclaimed, Ordering::Relaxed);
        self.stats
            .avg_optimization_time_ms
            .store(optimization_time.as_millis() as u64, Ordering::Relaxed);
        self.stats.last_optimization.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            Ordering::Relaxed,
        );

        Ok(OptimizationResult {
            bytes_reclaimed: total_bytes_reclaimed,
            entries_removed: total_removed,
            optimization_time,
            efficiency_before,
            efficiency_after,
        })
    }

    /// Remove expired entries based on access patterns
    pub fn remove_expired_entries<
        K: crate::cache::traits::CacheKey,
        V: crate::cache::traits::CacheValue,
    >(
        &self,
        cache: &ColdTierCache<K, V>,
    ) -> Result<usize, CacheOperationError> {
        let mut removed_count = 0;
        let _now = Instant::now();

        // DashMap provides lock-free concurrent access - no lock() needed
        let expired_keys: Vec<_> = cache
            .index
            .iter()
            .filter_map(|entry_ref| {
                let (key, entry) = entry_ref.pair();
                if entry.last_access.elapsed() > self.config.max_idle_duration {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();

        // Remove expired entries
        for key in expired_keys {
            if cache.remove(&key).is_ok() {
                removed_count += 1;
            }
        }

        Ok(removed_count)
    }

    /// Remove idle entries that haven't been accessed recently
    fn remove_idle_entries<
        K: crate::cache::traits::CacheKey,
        V: crate::cache::traits::CacheValue,
    >(
        &self,
        cache: &ColdTierCache<K, V>,
    ) -> Result<usize, CacheOperationError> {
        let mut removed_count = 0;
        let idle_threshold = self.config.max_idle_duration / 2; // More aggressive threshold

        // DashMap provides lock-free concurrent access - no lock() needed
        let idle_keys: Vec<_> = cache
            .index
            .iter()
            .filter_map(|entry_ref| {
                let (key, entry) = entry_ref.pair();
                if entry.access_count < 2 && entry.last_access.elapsed() > idle_threshold {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();

        // Remove idle entries
        for key in idle_keys {
            if cache.remove(&key).is_ok() {
                removed_count += 1;
            }
        }

        Ok(removed_count)
    }

    /// Remove expired entries with size tracking for accurate bytes reclaimed calculation
    fn remove_expired_entries_with_size_tracking<
        K: crate::cache::traits::CacheKey,
        V: crate::cache::traits::CacheValue,
    >(
        &self,
        cache: &ColdTierCache<K, V>,
    ) -> Result<(usize, u64), CacheOperationError> {
        let mut removed_count = 0;
        let mut bytes_reclaimed = 0u64;

        // Collect expired keys with their sizes for accurate tracking
        let expired_entries: Vec<_> = cache
            .index
            .iter()
            .filter_map(|entry_ref| {
                let (key, entry) = entry_ref.pair();
                if entry.last_access.elapsed() > self.config.max_idle_duration {
                    Some((key.clone(), entry.data_size))
                } else {
                    None
                }
            })
            .collect();

        // Remove expired entries and accumulate bytes reclaimed
        for (key, entry_size) in expired_entries {
            if cache.remove(&key).is_ok() {
                removed_count += 1;
                bytes_reclaimed += entry_size as u64;
            }
        }

        Ok((removed_count, bytes_reclaimed))
    }

    /// Remove idle entries with size tracking for accurate bytes reclaimed calculation
    fn remove_idle_entries_with_size_tracking<
        K: crate::cache::traits::CacheKey,
        V: crate::cache::traits::CacheValue,
    >(
        &self,
        cache: &ColdTierCache<K, V>,
    ) -> Result<(usize, u64), CacheOperationError> {
        let mut removed_count = 0;
        let mut bytes_reclaimed = 0u64;
        let idle_threshold = self.config.max_idle_duration / 2;

        // Collect idle keys with their sizes for accurate tracking
        let idle_entries: Vec<_> = cache
            .index
            .iter()
            .filter_map(|entry_ref| {
                let (key, entry) = entry_ref.pair();
                if entry.access_count < 2 && entry.last_access.elapsed() > idle_threshold {
                    Some((key.clone(), entry.data_size))
                } else {
                    None
                }
            })
            .collect();

        // Remove idle entries and accumulate bytes reclaimed
        for (key, entry_size) in idle_entries {
            if cache.remove(&key).is_ok() {
                removed_count += 1;
                bytes_reclaimed += entry_size as u64;
            }
        }

        Ok((removed_count, bytes_reclaimed))
    }

    /// Calculate storage efficiency (used space / total space)
    fn calculate_storage_efficiency<
        K: crate::cache::traits::CacheKey,
        V: crate::cache::traits::CacheValue,
    >(
        &self,
        cache: &ColdTierCache<K, V>,
    ) -> Result<f64, CacheOperationError> {
        // DashMap provides lock-free concurrent access - no lock() needed
        let used_space: u64 = cache
            .index
            .iter()
            .map(|entry_ref| entry_ref.value().data_size as u64)
            .sum();
        let total_space = cache.write_offset.load(Ordering::Relaxed);

        if total_space > 0 {
            Ok(used_space as f64 / total_space as f64)
        } else {
            Ok(1.0)
        }
    }

    /// Calculate fragmentation ratio (gaps / total space)
    fn calculate_fragmentation_ratio<
        K: crate::cache::traits::CacheKey,
        V: crate::cache::traits::CacheValue,
    >(
        &self,
        cache: &ColdTierCache<K, V>,
    ) -> Result<f64, CacheOperationError> {
        // Simplified fragmentation calculation using lock-free DashMap access
        let used_space: u64 = cache
            .index
            .iter()
            .map(|entry_ref| entry_ref.value().data_size as u64)
            .sum();
        let total_space = cache.write_offset.load(Ordering::Relaxed);

        if total_space > 0 {
            Ok(1.0 - (used_space as f64 / total_space as f64))
        } else {
            Ok(0.0)
        }
    }

    /// Check if optimization is needed
    pub fn needs_optimization<K: CacheKey, V: CacheValue>(
        &self,
        cache: &ColdTierCache<K, V>,
    ) -> bool {
        // Check storage efficiency
        if let Ok(efficiency) = self.calculate_storage_efficiency(cache)
            && efficiency < self.config.target_efficiency
        {
            return true;
        }

        // Check fragmentation
        if let Ok(fragmentation) = self.calculate_fragmentation_ratio(cache)
            && fragmentation >= self.config.min_fragmentation_ratio
        {
            return true;
        }

        // Check time since last optimization
        let last_optimization = self.stats.last_optimization.load(Ordering::Relaxed);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now - last_optimization > self.config.optimization_interval.as_secs() {
            return true;
        }

        false
    }

    /// Get optimization statistics
    pub fn get_stats(&self) -> OptimizationStatsSnapshot {
        OptimizationStatsSnapshot {
            total_optimizations: self.stats.total_optimizations.load(Ordering::Relaxed),
            bytes_reclaimed: self.stats.bytes_reclaimed.load(Ordering::Relaxed),
            entries_removed: self.stats.entries_removed.load(Ordering::Relaxed),
            avg_optimization_time_ms: self.stats.avg_optimization_time_ms.load(Ordering::Relaxed),
            last_optimization: self.stats.last_optimization.load(Ordering::Relaxed),
        }
    }
}

/// Optimization statistics snapshot
#[derive(Debug, Clone)]
pub struct OptimizationStatsSnapshot {
    pub total_optimizations: u64,
    pub bytes_reclaimed: u64,
    pub entries_removed: u64,
    pub avg_optimization_time_ms: u64,
    pub last_optimization: u64,
}

/// Analyze storage patterns for optimization decisions
pub fn analyze_storage_patterns<K: CacheKey, V: CacheValue>(
    cache: &ColdTierCache<K, V>,
) -> Result<StorageAnalysis, CacheOperationError> {
    // DashMap provides lock-free concurrent access - no lock() needed
    let mut access_counts = Vec::new();
    let mut sizes = Vec::new();

    for entry_ref in cache.index.iter() {
        let entry = entry_ref.value();
        access_counts.push(entry.access_count);
        sizes.push(entry.data_size);
    }

    let total_entries = cache.index.len();
    let total_size: u64 = sizes.iter().map(|&size| size as u64).sum();

    Ok(StorageAnalysis {
        total_entries,
        total_size,
        hot_entries: access_counts.iter().filter(|&&count| count > 5).count(),
        cold_entries: access_counts.iter().filter(|&&count| count <= 2).count(),
    })
}

/// Storage analysis result
#[derive(Debug)]
pub struct StorageAnalysis {
    pub total_entries: usize,
    pub total_size: u64,
    pub hot_entries: usize,
    pub cold_entries: usize,
}
