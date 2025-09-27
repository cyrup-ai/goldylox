//! Memory pressure monitoring - Integration with advanced monitoring system
//!
//! This module now serves as a bridge to the production-quality memory pressure monitoring
//! system located in the warm tier monitoring infrastructure.

// Re-export the advanced monitoring system components
pub use crate::cache::tier::warm::monitoring::memory_pressure::MemoryPressureMonitor;
pub use crate::cache::tier::warm::monitoring::types::PressureThresholds;

use crate::cache::config::CacheConfig;
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Create advanced pressure monitor with configuration
#[allow(dead_code)] // Memory management - used in pressure monitoring and threshold management
pub fn create_advanced_pressure_monitor(
    config: &CacheConfig,
) -> Result<MemoryPressureMonitor, CacheOperationError> {
    let thresholds = PressureThresholds {
        low_pressure: config.memory_config.low_pressure_threshold,
        medium_pressure: config.memory_config.medium_pressure_threshold,
        high_pressure: config.memory_config.high_pressure_threshold,
        critical_pressure: config.memory_config.critical_pressure_threshold,
    };

    // Use memory limit from config or auto-detect
    let memory_limit = config
        .memory_config
        .max_memory_usage
        .unwrap_or_else(get_system_memory_with_fallback);

    Ok(MemoryPressureMonitor::with_thresholds(
        memory_limit,
        thresholds,
        config.memory_config.alert_cooldown_ms,
        config.memory_config.max_history_samples,
        true,
    ))
}

/// Create advanced pressure monitor with default settings
#[allow(dead_code)] // Memory management - used in pressure monitoring and threshold management
pub fn create_default_pressure_monitor() -> Result<MemoryPressureMonitor, CacheOperationError> {
    let memory_limit = 1024 * 1024 * 1024; // 1GB default
    Ok(MemoryPressureMonitor::new(memory_limit))
}

/// Get system memory with cross-platform support
#[allow(dead_code)] // Memory management - used in pressure monitoring and threshold management
pub fn get_system_memory_with_fallback() -> u64 {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        if let Ok(contents) = fs::read_to_string("/proc/meminfo") {
            for line in contents.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            return kb * 1024; // Convert KB to bytes
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("sysctl").arg("hw.memsize").output()
            && let Ok(output_str) = String::from_utf8(output.stdout)
            && let Some(mem_str) = output_str.split_whitespace().nth(1)
            && let Ok(bytes) = mem_str.parse::<u64>()
        {
            return bytes;
        }
    }

    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::SystemInformation::GetPhysicallyInstalledSystemMemory;
        let mut memory_kb: u64 = 0;
        unsafe {
            if GetPhysicallyInstalledSystemMemory(&mut memory_kb) != 0 {
                return memory_kb * 1024; // Convert KB to bytes
            }
        }
    }

    // Fallback to 8GB default
    8 * 1024 * 1024 * 1024
}
