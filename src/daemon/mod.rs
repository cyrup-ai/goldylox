//! Loxd daemon module
//!
//! Provides system daemon functionality for the Goldylox cache including
//! installation, service management, and HTTP API server.

pub mod cli;
pub mod config;
pub mod install;
pub mod installer;
pub mod ipc;
pub mod manager;
pub mod process;
pub mod signing;

// Re-export key types
pub use cli::{Args, Cmd};
pub use config::ServiceConfig;
pub use install::{InstallerBuilder, InstallerError, install_daemon_async, uninstall_daemon_async};
pub use installer::{install, uninstall_async};
pub use manager::CacheManager;
pub use signing::{SigningConfig, is_signing_available, sign_binary, verify_signature};

/// Signal handling for Unix systems
pub mod signals {
    use nix::sys::signal::{self, Signal};
    use once_cell::sync::Lazy;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static RECEIVED_SIGNAL: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

    pub fn install_signal_handlers() -> Result<(), Box<dyn std::error::Error>> {
        extern "C" fn handler(sig: i32) {
            RECEIVED_SIGNAL.store(sig as usize, Ordering::SeqCst);
        }

        unsafe {
            signal::sigaction(
                Signal::SIGINT,
                &signal::SigAction::new(
                    signal::SigHandler::Handler(handler),
                    signal::SaFlags::empty(),
                    signal::SigSet::empty(),
                ),
            )
            .map_err(|e| format!("Failed to install SIGINT handler: {}", e))?;
            signal::sigaction(
                Signal::SIGTERM,
                &signal::SigAction::new(
                    signal::SigHandler::Handler(handler),
                    signal::SaFlags::empty(),
                    signal::SigSet::empty(),
                ),
            )
            .map_err(|e| format!("Failed to install SIGTERM handler: {}", e))?;
        }
        Ok(())
    }

    pub fn check_signals() -> Option<Signal> {
        let val = RECEIVED_SIGNAL.swap(0, Ordering::AcqRel);
        if val == 0 {
            None
        } else {
            Signal::try_from(val as i32).ok()
        }
    }
}
