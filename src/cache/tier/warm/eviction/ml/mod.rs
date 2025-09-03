//! Machine Learning-based eviction policy module
//!
//! This module provides ML-based cache eviction using feature extraction
//! and linear regression for optimal performance prediction.

pub mod features;
pub mod policy;

pub use policy::MachineLearningEvictionPolicy;
