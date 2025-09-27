//! Installer functions for loxd daemon
//!
//! High-level installer interface for installing and uninstalling the loxd daemon.

use anyhow::Result;
use std::env;

use crate::daemon::install::{InstallerBuilder, install_daemon_async, uninstall_daemon_async};

/// Install the loxd daemon as a system service
pub async fn install(dry_run: bool, _sign: bool, _identity: Option<String>) -> Result<()> {
    let current_exe = env::current_exe()?;

    let builder = InstallerBuilder::new("loxd", &current_exe)
        .description("Goldylox cache daemon")
        .auto_restart(true)
        .network(true)
        .user("root")
        .group("root");

    if !dry_run {
        install_daemon_async(builder).await?;
        println!("✓ loxd daemon installed successfully");
    } else {
        println!("✓ loxd daemon installation (dry run)");
    }

    Ok(())
}

/// Uninstall the loxd daemon
pub async fn uninstall_async(dry_run: bool) -> Result<()> {
    if !dry_run {
        uninstall_daemon_async("loxd").await?;
        println!("✓ loxd daemon uninstalled successfully");
    } else {
        println!("✓ loxd daemon uninstallation (dry run)");
    }

    Ok(())
}
