//! Blitz Cache - High-performance generic cache system
//!
//! A sophisticated multi-tier cache system with Generic Associated Types (GATs),
//! SIMD optimizations, coherence protocols, and advanced eviction policies.
//!
//! # Features
//!
//! - **Multi-tier architecture**: Hot, warm, and cold tiers with intelligent placement
//! - **Generic Associated Types**: Zero-cost abstractions with compile-time optimization
//! - **SIMD optimizations**: Vectorized operations for maximum performance
//! - **Coherence protocols**: Distributed cache consistency
//! - **Advanced eviction**: LRU, LFU, ARC, and ML-based policies
//! - **Lock-free operations**: Atomic data structures for high concurrency
//! - **Compression support**: LZ4 and Zstd for cold tier storage
//! - **Async support**: Tokio integration for async workloads

pub mod cache;
pub mod telemetry;

// REMOVED: Compatibility re-exports that hide canonical API paths
// These re-exports enabled broken code to import via the root instead of explicit paths
// Users must now import from canonical module paths:
// - Use blitz_cache::cache::coordinator:: for coordinator functionality
// - Use specific cache::* imports instead of wildcard re-exports
