//! Goldylox prelude - convenient imports for users
//! 
//! This module provides everything users need to work with the Goldylox cache system.

// Re-export the public API
pub use crate::goldylox::{Goldylox, GoldyloxBuilder};

// Re-export essential error types that users might need
pub use crate::cache::traits::types_and_enums::CacheOperationError;

// Re-export serde traits that users' types need to implement
pub use serde::{Deserialize, Serialize};