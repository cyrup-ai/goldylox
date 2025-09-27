//! CLI-specific error handling
//!
//! This module provides error types and handling specifically for the CLI interface,
//! mapping cache operation errors to user-friendly CLI messages.

use std::fmt;

use crate::cache::traits::types_and_enums::CacheOperationError;

/// CLI-specific error type
#[derive(Debug)]
pub enum CliError {
    /// Cache operation error
    CacheError(CacheOperationError),
    /// Configuration error
    ConfigError(String),
    /// Input/output error
    IoError(std::io::Error),
    /// JSON parsing error
    JsonError(serde_json::Error),
    /// TOML parsing error
    TomlError(toml::de::Error),
    /// Invalid argument error
    ArgumentError(String),
    /// REPL error
    ReplError(String),
    /// System error
    SystemError(String),
    /// Daemon not installed
    DaemonNotInstalled(String),
    /// Daemon not running
    DaemonNotRunning(String),
    /// Daemon startup failed
    DaemonStartupFailed(String),
    /// Daemon connection failed
    DaemonConnectionFailed(String),
    /// HTTP request failed
    HttpRequestFailed(String),
    /// Network error (HTTP client issues)
    NetworkError(String),
    /// Daemon management error
    DaemonError(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::CacheError(e) => write!(f, "Cache error: {}", e),
            CliError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            CliError::IoError(e) => write!(f, "I/O error: {}", e),
            CliError::JsonError(e) => write!(f, "JSON error: {}", e),
            CliError::TomlError(e) => write!(f, "TOML error: {}", e),
            CliError::ArgumentError(msg) => write!(f, "Argument error: {}", msg),
            CliError::ReplError(msg) => write!(f, "REPL error: {}", msg),
            CliError::SystemError(msg) => write!(f, "System error: {}", msg),
            CliError::DaemonNotInstalled(msg) => write!(f, "Daemon not installed: {}", msg),
            CliError::DaemonNotRunning(msg) => write!(f, "Daemon not running: {}", msg),
            CliError::DaemonStartupFailed(msg) => write!(f, "Daemon startup failed: {}", msg),
            CliError::DaemonConnectionFailed(msg) => write!(f, "Daemon connection failed: {}", msg),
            CliError::HttpRequestFailed(msg) => write!(f, "HTTP request failed: {}", msg),
            CliError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            CliError::DaemonError(msg) => write!(f, "Daemon error: {}", msg),
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CliError::CacheError(e) => Some(e),
            CliError::IoError(e) => Some(e),
            CliError::JsonError(e) => Some(e),
            CliError::TomlError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<CacheOperationError> for CliError {
    fn from(error: CacheOperationError) -> Self {
        CliError::CacheError(error)
    }
}

impl From<std::io::Error> for CliError {
    fn from(error: std::io::Error) -> Self {
        CliError::IoError(error)
    }
}

impl From<serde_json::Error> for CliError {
    fn from(error: serde_json::Error) -> Self {
        CliError::JsonError(error)
    }
}

impl From<toml::de::Error> for CliError {
    fn from(error: toml::de::Error) -> Self {
        CliError::TomlError(error)
    }
}

/// Result type for CLI operations
pub type CliResult<T> = Result<T, CliError>;
