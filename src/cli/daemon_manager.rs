//! Daemon lifecycle management for the CLI
//!
//! This module handles detection of daemon installation, process status,
//! startup management, and health checking for the Goldylox daemon.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use std::thread;

use serde::{Deserialize, Serialize};

use crate::cli::errors::{CliError, CliResult};

/// Health check response from daemon API
#[derive(Debug, Deserialize)]
struct HealthResponse {
    success: bool,
    data: String,
    message: Option<String>,
}

/// Configuration for daemon management
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub endpoint: String,
    pub health_timeout_ms: u64,
    pub startup_timeout_ms: u64,
    pub auto_start: bool,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:51085".to_string(),
            health_timeout_ms: 2000,
            startup_timeout_ms: 30000,
            auto_start: true,
        }
    }
}

/// Manages daemon detection, startup, and health checking
pub struct DaemonManager {
    config: DaemonConfig,
    client: reqwest::Client,
}

impl DaemonManager {
    /// Create a new daemon manager with configuration
    pub fn new(config: DaemonConfig) -> CliResult<Self> {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(4)
            .pool_idle_timeout(Duration::from_secs(30))
            .timeout(Duration::from_millis(config.health_timeout_ms + 1000)) // Buffer for health checks
            .connect_timeout(Duration::from_secs(5))
            .gzip(true)
            .http2_prior_knowledge()
            .build()
            .map_err(|e| CliError::SystemError(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { config, client })
    }

    /// Check if loxd binary is installed and accessible
    pub fn is_daemon_installed(&self) -> bool {
        // First check PATH using which
        if which::which("loxd").is_ok() {
            return true;
        }

        // Fall back to checking standard locations
        let standard_paths = [
            "/usr/local/bin/loxd",
            "/usr/bin/loxd",
            "~/.local/bin/loxd",
            "~/.cargo/bin/loxd",
            "./target/release/loxd",
            "./target/debug/loxd",
        ];

        for path in &standard_paths {
            let expanded_path = if path.starts_with("~/") {
                if let Some(home) = dirs::home_dir() {
                    home.join(&path[2..])
                } else {
                    continue;
                }
            } else {
                std::path::PathBuf::from(path)
            };

            if expanded_path.exists() && expanded_path.is_file() {
                return true;
            }
        }

        false
    }

    /// Check if daemon process is currently running
    pub fn is_daemon_running(&self) -> bool {
        // Try to detect process by checking if we can connect to the daemon control port (51084)
        let addr = match "127.0.0.1:51084".parse() {
            Ok(addr) => addr,
            Err(_) => return false, // Invalid address format - daemon not running
        };
        
        if let Ok(_) = std::net::TcpStream::connect_timeout(
            &addr,
            Duration::from_millis(500)
        ) {
            return true;
        }

        // Platform-specific process detection as fallback
        self.detect_daemon_process()
    }

    /// Platform-specific daemon process detection
    #[cfg(unix)]
    fn detect_daemon_process(&self) -> bool {
        // Use pgrep to find loxd processes
        match Command::new("pgrep")
            .arg("-f")
            .arg("loxd")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Ok(status) => status.success(),
            Err(_) => false,
        }
    }

    /// Platform-specific daemon process detection
    #[cfg(windows)]
    fn detect_daemon_process(&self) -> bool {
        // Use tasklist to find loxd.exe processes
        match Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq loxd.exe"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Ok(status) => status.success(),
            Err(_) => false,
        }
    }

    /// Check if Axum server is responding with health check
    pub async fn is_axum_healthy(&self) -> bool {
        let health_url = format!("{}/health", self.config.endpoint);
        
        match self.client
            .get(&health_url)
            .timeout(Duration::from_millis(self.config.health_timeout_ms))
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    // Try to parse the JSON response
                    if let Ok(health) = response.json::<HealthResponse>().await {
                        return health.success && health.data == "healthy";
                    }
                }
                false
            }
            Err(_) => false,
        }
    }

    /// Start the daemon process if it's installed but not running
    pub fn start_daemon(&self) -> CliResult<()> {
        if !self.is_daemon_installed() {
            return Err(CliError::SystemError(
                "loxd daemon is not installed. Please install it first.".to_string()
            ));
        }

        if self.is_daemon_running() {
            return Ok(()); // Already running
        }

        // Find the daemon binary
        let daemon_path = if let Ok(path) = which::which("loxd") {
            path
        } else {
            // Try standard locations
            let standard_paths = [
                "/usr/local/bin/loxd",
                "~/.local/bin/loxd", 
                "~/.cargo/bin/loxd",
                "./target/release/loxd",
                "./target/debug/loxd",
            ];

            let mut found_path = None;
            for path in &standard_paths {
                let expanded_path = if path.starts_with("~/") {
                    if let Some(home) = dirs::home_dir() {
                        home.join(&path[2..])
                    } else {
                        continue;
                    }
                } else {
                    std::path::PathBuf::from(path)
                };

                if expanded_path.exists() && expanded_path.is_file() {
                    found_path = Some(expanded_path);
                    break;
                }
            }

            found_path.ok_or_else(|| CliError::SystemError(
                "Could not find loxd binary to start".to_string()
            ))?
        };

        // Start the daemon process
        let mut child = Command::new(&daemon_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CliError::SystemError(format!("Failed to start daemon: {}", e)))?;

        // Give it a moment to start
        thread::sleep(Duration::from_millis(500));

        // Check if it's still running (not crashed immediately)
        match child.try_wait() {
            Ok(Some(status)) => {
                return Err(CliError::SystemError(format!(
                    "Daemon process exited immediately with status: {}", status
                )));
            }
            Ok(None) => {
                // Still running, good
            }
            Err(e) => {
                return Err(CliError::SystemError(format!(
                    "Failed to check daemon process status: {}", e
                )));
            }
        }

        Ok(())
    }

    /// Wait for the daemon to become ready with exponential backoff
    pub async fn wait_for_ready(&self) -> CliResult<()> {
        let start_time = Instant::now();
        let timeout = Duration::from_millis(self.config.startup_timeout_ms);
        let mut delay = Duration::from_millis(100);

        while start_time.elapsed() < timeout {
            if self.is_daemon_running() && self.is_axum_healthy().await {
                return Ok(());
            }

            tokio::time::sleep(delay).await;

            // Exponential backoff with cap
            delay = std::cmp::min(delay * 2, Duration::from_secs(2));
        }

        Err(CliError::SystemError(format!(
            "Daemon failed to become ready within {}ms", 
            self.config.startup_timeout_ms
        )))
    }

    /// Offer installation guidance if daemon is not installed
    pub fn offer_installation(&self) -> CliResult<()> {
        if self.is_daemon_installed() {
            return Ok(());
        }

        eprintln!("❌ Goldylox daemon (loxd) is not installed or not found in PATH");
        eprintln!();
        eprintln!("To install the daemon, you can:");
        eprintln!("1. Build from source: cargo build --release --features daemon");
        eprintln!("2. Install via cargo: cargo install goldylox --features daemon");
        eprintln!("3. Download from releases: https://github.com/cyrup-ai/goldylox/releases");
        eprintln!();
        eprintln!("Make sure the 'loxd' binary is in your PATH or in one of these locations:");
        eprintln!("  • /usr/local/bin/loxd");
        eprintln!("  • ~/.local/bin/loxd");
        eprintln!("  • ~/.cargo/bin/loxd");
        eprintln!();

        Err(CliError::SystemError("Daemon not installed".to_string()))
    }

    /// Ensure daemon is running, offering installation or starting as needed
    pub async fn ensure_daemon_ready(&self) -> CliResult<()> {
        // Check installation
        if !self.is_daemon_installed() {
            return self.offer_installation();
        }

        // Check if already running and healthy
        if self.is_daemon_running() && self.is_axum_healthy().await {
            return Ok(());
        }

        if !self.config.auto_start {
            return Err(CliError::SystemError(
                "Daemon is not running. Start it manually or enable auto_start.".to_string()
            ));
        }

        // Start daemon if not running
        if !self.is_daemon_running() {
            println!("🚀 Starting Goldylox daemon...");
            self.start_daemon()?;
        }

        // Wait for readiness
        println!("⏳ Waiting for daemon to become ready...");
        self.wait_for_ready().await?;

        println!("✅ Daemon is ready!");
        Ok(())
    }
}