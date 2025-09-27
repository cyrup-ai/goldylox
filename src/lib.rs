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

// CLI module (for binary)
#[cfg(feature = "cli")]
pub mod cli;

// Daemon module (for loxd binary)
#[cfg(feature = "daemon")]
pub mod daemon;

// Cache implementation modules - traits are public for user implementations
pub mod cache;
pub(crate) mod telemetry;

// Re-export the public API at the crate root for convenience
pub use cache::manager::strategy::CacheStrategy;
pub use goldylox::{Goldylox, GoldyloxBuilder};
pub use prelude::*;

// Public cache traits and types that users need to implement
pub mod traits {
    pub use crate::cache::traits::types_and_enums::CompressionHint;
    pub use crate::cache::traits::{CacheKey, CacheValue};
    pub use crate::cache::traits::{CacheValueMetadata, ValueMetadata};
}
