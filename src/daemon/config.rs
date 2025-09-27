use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top‑level daemon configuration (mirrors original defaults).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub services_dir: Option<String>,
    pub log_dir: Option<String>,
    pub default_user: Option<String>,
    pub default_group: Option<String>,
    pub auto_restart: Option<bool>,
    pub services: Vec<ServiceDefinition>,
    pub sse: Option<SseServerConfig>,
    /// Cache HTTP transport binding (host:port)
    pub cache_bind: Option<String>,
}

/// Cache server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseServerConfig {
    /// Enable cache server
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Host to bind cache server to
    #[serde(default = "default_cache_host")]
    pub host: String,
    /// Port to bind cache server to
    #[serde(default = "default_cache_port")]
    pub port: u16,
    /// Maximum number of concurrent connections
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
    /// Connection timeout (seconds)
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u64,
    /// CORS allowed origins
    #[serde(default = "default_cors_origins")]
    pub cors_origins: Vec<String>,
}

fn default_true() -> bool {
    true
}
fn default_cache_host() -> String {
    "127.0.0.1".to_string()
}
fn default_cache_port() -> u16 {
    51085
}
fn default_max_connections() -> usize {
    100
}
fn default_connection_timeout() -> u64 {
    30
}
fn default_cors_origins() -> Vec<String> {
    vec!["*".to_string()]
}

impl Default for SseServerConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            host: default_cache_host(),
            port: default_cache_port(),
            max_connections: default_max_connections(),
            connection_timeout: default_connection_timeout(),
            cors_origins: default_cors_origins(),
        }
    }
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            services_dir: Some("/etc/loxd/services".into()),
            log_dir: Some("/var/log/loxd".into()),
            default_user: Some("loxd".into()),
            default_group: Some("daemon".into()),
            auto_restart: Some(true),
            services: vec![],
            sse: Some(SseServerConfig::default()),
            cache_bind: Some("127.0.0.1:51085".into()),
        }
    }
}

/// On‑disk TOML description of a single service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDefinition {
    pub name: String,
    pub description: Option<String>,
    pub command: String,
    pub working_dir: Option<String>,
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
    #[serde(default)]
    pub auto_restart: bool,
    pub user: Option<String>,
    pub group: Option<String>,
    pub restart_delay_s: Option<u64>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub health_check: Option<HealthCheckConfig>,
    #[serde(default)]
    pub log_rotation: Option<LogRotationConfig>,
    #[serde(default)]
    pub watch_dirs: Vec<String>,
    pub ephemeral_dir: Option<String>,
    /// Service type (e.g., "cache" for special handling)
    pub service_type: Option<String>,
    pub memfs: Option<MemoryFsConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFsConfig {
    pub size_mb: u32, // clamped at 2048 elsewhere
    pub mount_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub check_type: String, // http | tcp | script
    pub target: String,
    pub interval_secs: u64,
    pub timeout_secs: u64,
    pub retries: u32,
    pub expected_response: Option<String>,
    #[serde(default)]
    pub on_failure: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRotationConfig {
    pub max_size_mb: u64,
    pub max_files: u32,
    pub interval_days: u32,
    pub compress: bool,
    pub timestamp: bool,
}
