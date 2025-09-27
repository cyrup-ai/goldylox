//! Goldylox prelude - convenient imports for users
//!
//! This module provides everything users need to work with the Goldylox cache system.

// Re-export the public API
pub use crate::goldylox::{Goldylox, GoldyloxBuilder};

// Re-export essential error types that users might need
pub use crate::cache::traits::types_and_enums::CacheOperationError;

// Re-export useful functions that examples might need
pub use crate::cache::tier::cold::should_promote_to_warm;

// Re-export serde traits that users' types need to implement
pub use serde::{Deserialize, Serialize};

// Re-export cache traits that users need to implement for custom types
pub use crate::traits::{CacheKey, CacheValue, CacheValueMetadata, CompressionHint, ValueMetadata};

// Re-export supporting types for trait implementations
pub use crate::cache::traits::impls::{
    StandardHashContext, StandardPriority, StandardSizeEstimator,
};
