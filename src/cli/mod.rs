//! CLI module for Goldylox cache system
//!
//! This module provides a comprehensive command-line interface that mirrors
//! all public API functionality of the Goldylox cache system.

pub mod commands;
pub mod config;
pub mod daemon_detection;
pub mod errors;
pub mod http_client;
pub mod output;
pub mod repl;

// Re-export commonly used types
pub use commands::*;
pub use config::CliConfig;
pub use errors::{CliError, CliResult};
pub use output::OutputFormat;
