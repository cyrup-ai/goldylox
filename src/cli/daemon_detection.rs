//! Daemon detection and management for CLI
//!
//! This module handles detecting if the loxd system daemon is running
//! and provides functionality to start it if needed.

use std::process::{Command, Stdio};
use std::time::Duration;

use tokio::time::sleep;

use crate::cli::errors::{CliError, CliResult};
use crate::cli::http_client::DaemonClient;

/// Daemon detection and management
pub struct DaemonDetection {
    daemon_endpoint: String,
    timeout_ms: u64,
    auto_start: bool,
}

impl DaemonDetection {
    /// Create new daemon detection instance
    pub fn new(daemon_endpoint: String, timeout_ms: u64, auto_start: bool) -> Self {
        Self {
            daemon_endpoint,
            timeout_ms,
            auto_start,
        }
    }

    /// Check if daemon is running and healthy
    pub async fn is_daemon_running(&self) -> bool {
        match DaemonClient::new(self.daemon_endpoint.clone(), Some(self.timeout_ms)) {
            Ok(client) => client.health_check().await.unwrap_or_default(),
            Err(_) => false,
        }
    }

    /// Ensure daemon is running, starting it if necessary and configured to do so
    pub async fn ensure_daemon_running(&self) -> CliResult<DaemonClient> {
        // First check if daemon is already running
        if self.is_daemon_running().await {
            return DaemonClient::new(self.daemon_endpoint.clone(), Some(self.timeout_ms));
        }

        // If not running and auto-start is disabled, return error
        if !self.auto_start {
            return Err(CliError::DaemonError(format!(
                "Daemon not running on {} and auto-start is disabled. Please start loxd manually or enable auto-start.",
                self.daemon_endpoint
            )));
        }

        // Try to start the daemon
        self.start_daemon().await?;

        // Wait for daemon to become healthy
        self.wait_for_daemon().await?;

        // Return client
        DaemonClient::new(self.daemon_endpoint.clone(), Some(self.timeout_ms))
    }

    /// Start the daemon process
    async fn start_daemon(&self) -> CliResult<()> {
        // Try to find loxd binary in PATH
        let loxd_path = self.find_loxd_binary()?;

        // Start daemon process
        let mut child = Command::new(&loxd_path)
            .arg("--daemon")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CliError::DaemonError(format!("Failed to start loxd daemon: {}", e)))?;

        // Give the process a moment to start
        sleep(Duration::from_millis(500)).await;

        // Check if process is still running (not crashed immediately)
        match child.try_wait() {
            Ok(Some(status)) => {
                return Err(CliError::DaemonError(format!(
                    "Daemon process exited immediately with status: {}",
                    status
                )));
            }
            Ok(None) => {
                // Process is still running, good
            }
            Err(e) => {
                return Err(CliError::DaemonError(format!(
                    "Failed to check daemon process status: {}",
                    e
                )));
            }
        }

        Ok(())
    }

    /// Wait for daemon to become healthy
    async fn wait_for_daemon(&self) -> CliResult<()> {
        let max_attempts = 20; // 10 seconds total with 500ms intervals
        let mut attempts = 0;

        while attempts < max_attempts {
            if self.is_daemon_running().await {
                return Ok(());
            }

            attempts += 1;
            sleep(Duration::from_millis(500)).await;
        }

        Err(CliError::DaemonError(format!(
            "Daemon failed to become healthy within {} seconds",
            max_attempts / 2
        )))
    }

    /// Find loxd binary in system PATH
    fn find_loxd_binary(&self) -> CliResult<String> {
        // First try 'loxd' in PATH
        if let Ok(output) = Command::new("which").arg("loxd").output()
            && output.status.success()
        {
            let path_str = String::from_utf8_lossy(&output.stdout);
            let path = path_str.trim();
            if !path.is_empty() {
                return Ok(path.to_string());
            }
        }

        // Try common installation locations
        let common_paths = [
            "/usr/local/bin/loxd",
            "/usr/bin/loxd",
            "/opt/goldylox/bin/loxd",
            "./target/debug/loxd",
            "./target/release/loxd",
        ];

        for path in &common_paths {
            if std::path::Path::new(path).exists() {
                return Ok(path.to_string());
            }
        }

        // If we're in development mode, try building it
        if std::path::Path::new("Cargo.toml").exists()
            && let Ok(status) = Command::new("cargo")
                .args(["build", "--bin", "loxd", "--features", "daemon"])
                .status()
            && status.success()
        {
            let debug_path = "./target/debug/loxd";
            if std::path::Path::new(debug_path).exists() {
                return Ok(debug_path.to_string());
            }
        }

        Err(CliError::DaemonError(
            "Could not find loxd binary. Please ensure it's installed and in PATH, or run 'cargo build --bin loxd --features daemon' to build it.".to_string()
        ))
    }

    /// Get the daemon endpoint URL
    pub fn endpoint(&self) -> &str {
        &self.daemon_endpoint
    }

    /// Check daemon status and return detailed information
    pub async fn daemon_status(&self) -> CliResult<DaemonStatus> {
        let is_running = self.is_daemon_running().await;

        if is_running {
            let client = DaemonClient::new(self.daemon_endpoint.clone(), Some(self.timeout_ms))?;
            let health = client.health_check().await.unwrap_or(false);

            Ok(DaemonStatus {
                running: true,
                healthy: health,
                endpoint: self.daemon_endpoint.clone(),
                auto_start_enabled: self.auto_start,
            })
        } else {
            Ok(DaemonStatus {
                running: false,
                healthy: false,
                endpoint: self.daemon_endpoint.clone(),
                auto_start_enabled: self.auto_start,
            })
        }
    }
}

/// Daemon status information
#[derive(Debug)]
pub struct DaemonStatus {
    /// Whether daemon process is running
    pub running: bool,
    /// Whether daemon is healthy and responsive
    pub healthy: bool,
    /// Daemon endpoint URL
    pub endpoint: String,
    /// Whether auto-start is enabled
    pub auto_start_enabled: bool,
}

impl DaemonStatus {
    /// Get a human-readable status description
    pub fn description(&self) -> String {
        if self.running && self.healthy {
            format!("Daemon running and healthy on {}", self.endpoint)
        } else if self.running {
            format!("Daemon running but unhealthy on {}", self.endpoint)
        } else {
            format!("Daemon not running (endpoint: {})", self.endpoint)
        }
    }

    /// Check if daemon is ready for use
    pub fn is_ready(&self) -> bool {
        self.running && self.healthy
    }
}
