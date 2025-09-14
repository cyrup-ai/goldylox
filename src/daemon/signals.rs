//! Signal handling for the goldylox daemon
//!
//! Provides lock-free Unix signal handling for graceful shutdown.

use once_cell::sync::Lazy;

// Cheap, polling‑based Unix signal handling (lock‑free).
static RECEIVED_SIGNAL: Lazy<std::sync::atomic::AtomicUsize> =
    Lazy::new(|| std::sync::atomic::AtomicUsize::new(0));

pub fn install_signal_handlers() -> Result<(), Box<dyn std::error::Error>> {
    use nix::sys::signal::{self, Signal};
    extern "C" fn handler(sig: i32) {
        RECEIVED_SIGNAL.store(sig as usize, std::sync::atomic::Ordering::SeqCst);
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

/// Non‑blocking check – returns Some(signal) once.
pub fn check_signals() -> Option<nix::sys::signal::Signal> {
    use std::sync::atomic::Ordering::*;

    use nix::sys::signal::Signal;
    let val = RECEIVED_SIGNAL.swap(0, AcqRel);
    if val == 0 {
        None
    } else {
        match Signal::try_from(val as i32) {
            Ok(signal) => Some(signal),
            Err(_) => None, // Invalid signal value - ignore
        }
    }
}