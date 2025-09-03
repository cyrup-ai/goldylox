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

// Public API modules
pub mod goldylox;
pub mod prelude;

// Internal implementation modules (crate private)
pub(crate) mod cache;
pub(crate) mod telemetry;

// Re-export the public API at the crate root for convenience
pub use goldylox::{Goldylox, GoldyloxBuilder};
pub use prelude::*;
