//! Output formatting for CLI commands
//!
//! This module provides different output formats for CLI commands including
//! JSON, human-readable, and quiet modes.

use clap::ValueEnum;
use serde_json;

use crate::goldylox::BatchOperationSummary;

/// Output format options
#[derive(Clone, Debug, PartialEq, ValueEnum, Default)]
pub enum OutputFormat {
    /// Human-readable output (default)
    #[default]
    Human,
    /// JSON output
    Json,
    /// Quiet mode (minimal output)
    Quiet,
}

/// Format a simple value for output
pub fn format_value(value: &str, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Human => value.to_string(),
        OutputFormat::Json => serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string()),
        OutputFormat::Quiet => value.to_string(),
    }
}

/// Format a boolean result for output
pub fn format_boolean(value: bool, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Human => {
            if value {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        OutputFormat::Json => serde_json::to_string(&value).unwrap_or_else(|_| "false".to_string()),
        OutputFormat::Quiet => {
            if value {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
    }
}

/// Format an optional value for output
pub fn format_optional<T: std::fmt::Display>(value: &Option<T>, format: &OutputFormat) -> String {
    match value {
        Some(v) => match format {
            OutputFormat::Human => format!("{}", v),
            OutputFormat::Json => {
                serde_json::to_string(&format!("{}", v)).unwrap_or_else(|_| "null".to_string())
            }
            OutputFormat::Quiet => format!("{}", v),
        },
        None => match format {
            OutputFormat::Human => "Not found".to_string(),
            OutputFormat::Json => "null".to_string(),
            OutputFormat::Quiet => "".to_string(),
        },
    }
}

/// Format a hash value for output
pub fn format_hash(hash: u64, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Human => format!("0x{:016x}", hash),
        OutputFormat::Json => serde_json::to_string(&format!("0x{:016x}", hash))
            .unwrap_or_else(|_| "null".to_string()),
        OutputFormat::Quiet => format!("{}", hash),
    }
}

/// Format batch operation results
pub fn format_batch_summary<T: std::fmt::Debug>(
    summary: &BatchOperationSummary<T>,
    operation_name: &str,
    format: &OutputFormat,
) -> String {
    match format {
        OutputFormat::Human => {
            let (avg_ms, min_ms, max_ms) = summary.latency_stats_ms();
            format!(
                "Batch {} Results:\n\
                 ✓ Success: {}/{} ({:.1}%)\n\
                 ⏱ Latency: avg={:.2}ms, min={:.2}ms, max={:.2}ms\n\
                 🚀 Throughput: {:.0} ops/sec",
                operation_name,
                summary.successful_results.len(),
                summary.total_operations,
                summary.success_rate * 100.0,
                avg_ms,
                min_ms,
                max_ms,
                summary.throughput_ops_per_sec()
            )
        }
        OutputFormat::Json => {
            let json_result = serde_json::json!({
                "operation": operation_name,
                "successful_count": summary.successful_results.len(),
                "failed_count": summary.failed_count,
                "total_operations": summary.total_operations,
                "success_rate": summary.success_rate,
                "total_time_ns": summary.total_time_ns,
                "avg_latency_ns": summary.avg_latency_ns,
                "min_latency_ns": summary.min_latency_ns,
                "max_latency_ns": summary.max_latency_ns,
                "throughput_ops_per_sec": summary.throughput_ops_per_sec()
            });
            serde_json::to_string_pretty(&json_result).unwrap_or_else(|_| "{}".to_string())
        }
        OutputFormat::Quiet => {
            format!(
                "{}/{}",
                summary.successful_results.len(),
                summary.total_operations
            )
        }
    }
}

/// Format statistics output
pub fn format_stats(stats_json: &str, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Human => {
            // Parse JSON and format as human-readable
            if let Ok(stats) = serde_json::from_str::<serde_json::Value>(stats_json) {
                format!(
                    "Cache Statistics:\n\
                     📊 Operations: {}\n\
                     🎯 Hit Rate: {:.2}%\n\
                     🚀 Ops/Sec: {:.2}\n\
                     🔥 Hot Hits: {}\n\
                     🌡️ Warm Hits: {}\n\
                     🧊 Cold Hits: {}\n\
                     ❌ Misses: {}\n\
                     ⏱️ Avg Latency: {}ns\n\
                     💾 Memory: {} bytes",
                    stats["total_operations"].as_u64().unwrap_or(0),
                    stats["overall_hit_rate"].as_f64().unwrap_or(0.0) * 100.0,
                    stats["ops_per_second"].as_f64().unwrap_or(0.0),
                    stats["hot_tier_hits"].as_u64().unwrap_or(0),
                    stats["warm_tier_hits"].as_u64().unwrap_or(0),
                    stats["cold_tier_hits"].as_u64().unwrap_or(0),
                    stats["total_misses"].as_u64().unwrap_or(0),
                    stats["avg_access_latency_ns"].as_u64().unwrap_or(0),
                    stats["total_memory_usage"].as_u64().unwrap_or(0)
                )
            } else {
                "Invalid statistics data".to_string()
            }
        }
        OutputFormat::Json => {
            // Pretty-print the JSON
            if let Ok(stats) = serde_json::from_str::<serde_json::Value>(stats_json) {
                serde_json::to_string_pretty(&stats).unwrap_or_else(|_| stats_json.to_string())
            } else {
                stats_json.to_string()
            }
        }
        OutputFormat::Quiet => {
            // Just the hit rate as a percentage
            if let Ok(stats) = serde_json::from_str::<serde_json::Value>(stats_json) {
                format!(
                    "{:.1}",
                    stats["overall_hit_rate"].as_f64().unwrap_or(0.0) * 100.0
                )
            } else {
                "0".to_string()
            }
        }
    }
}

/// Print output based on format and quiet mode
pub fn print_output(content: &str, format: &OutputFormat, quiet: bool) {
    if quiet && !matches!(format, OutputFormat::Quiet) {
        return;
    }
    println!("{}", content);
}

/// Print error message
pub fn print_error(message: &str, quiet: bool) {
    if !quiet {
        eprintln!("Error: {}", message);
    }
}
