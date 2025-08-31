//! Circuit breaker implementation for fault tolerance
//!
//! This module provides circuit breaker functionality to prevent cascading failures
//! and manage tier-specific fault tolerance.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use crossbeam_utils::{atomic::AtomicCell, CachePadded};

use super::types::{CircuitBreakerConfig, CircuitState};

/// Circuit breaker for tier failures
#[derive(Debug)]
pub struct CircuitBreaker {
    /// Circuit state per tier
    pub tier_states: CachePadded<[AtomicCell<CircuitState>; 3]>, // Hot, Warm, Cold
    /// Failure counters
    pub failure_counters: CachePadded<[AtomicU32; 3]>,
    /// Success counters
    pub success_counters: CachePadded<[AtomicU32; 3]>,
    /// State transition timestamps
    pub state_transitions: CachePadded<[AtomicCell<Instant>; 3]>,
    /// Circuit breaker configuration
    pub breaker_config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    /// Create new circuit breaker
    pub fn new() -> Self {
        Self {
            tier_states: CachePadded::new([
                AtomicCell::new(CircuitState::Closed),
                AtomicCell::new(CircuitState::Closed),
                AtomicCell::new(CircuitState::Closed),
            ]),
            failure_counters: CachePadded::new([
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
            ]),
            success_counters: CachePadded::new([
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
            ]),
            state_transitions: CachePadded::new([
                AtomicCell::new(Instant::now()),
                AtomicCell::new(Instant::now()),
                AtomicCell::new(Instant::now()),
            ]),
            breaker_config: CircuitBreakerConfig::default(),
        }
    }

    /// Record failure for tier
    #[inline]
    pub fn record_failure(&self, tier: u8) {
        if tier >= 3 {
            return;
        }

        let tier_idx = tier as usize;
        let failures = self.failure_counters[tier_idx].fetch_add(1, Ordering::Relaxed) + 1;

        // Check if we should open the circuit
        if failures >= self.breaker_config.failure_threshold {
            let current_state = self.tier_states[tier_idx].load();
            if current_state == CircuitState::Closed {
                self.tier_states[tier_idx].store(CircuitState::Open);
                self.state_transitions[tier_idx].store(Instant::now());
            }
        }
    }

    /// Record success for tier
    #[inline]
    pub fn record_success(&self, tier: u8) {
        if tier >= 3 {
            return;
        }

        let tier_idx = tier as usize;
        let current_state = self.tier_states[tier_idx].load();

        match current_state {
            CircuitState::HalfOpen => {
                let successes = self.success_counters[tier_idx].fetch_add(1, Ordering::Relaxed) + 1;
                if successes >= self.breaker_config.success_threshold {
                    // Close the circuit
                    self.tier_states[tier_idx].store(CircuitState::Closed);
                    self.failure_counters[tier_idx].store(0, Ordering::Relaxed);
                    self.success_counters[tier_idx].store(0, Ordering::Relaxed);
                    self.state_transitions[tier_idx].store(Instant::now());
                }
            }
            CircuitState::Closed => {
                // Reset failure counter on success
                self.failure_counters[tier_idx].store(0, Ordering::Relaxed);
            }
            CircuitState::Open => {
                // Check if we should transition to half-open
                let last_transition = self.state_transitions[tier_idx].load();
                if Instant::now().duration_since(last_transition)
                    >= self.breaker_config.timeout_duration
                {
                    self.tier_states[tier_idx].store(CircuitState::HalfOpen);
                    self.success_counters[tier_idx].store(1, Ordering::Relaxed);
                    self.state_transitions[tier_idx].store(Instant::now());
                }
            }
        }
    }

    /// Get circuit state for tier
    #[inline(always)]
    pub fn get_state(&self, tier: u8) -> CircuitState {
        if tier >= 3 {
            return CircuitState::Closed;
        }

        let tier_idx = tier as usize;
        let current_state = self.tier_states[tier_idx].load();

        // Check for automatic state transitions
        match current_state {
            CircuitState::Open => {
                let last_transition = self.state_transitions[tier_idx].load();
                if Instant::now().duration_since(last_transition)
                    >= self.breaker_config.timeout_duration
                {
                    self.tier_states[tier_idx].store(CircuitState::HalfOpen);
                    self.state_transitions[tier_idx].store(Instant::now());
                    CircuitState::HalfOpen
                } else {
                    CircuitState::Open
                }
            }
            _ => current_state,
        }
    }

    /// Check if tier is available for requests
    #[inline(always)]
    pub fn is_tier_available(&self, tier: u8) -> bool {
        match self.get_state(tier) {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => {
                // Allow limited requests in half-open state
                let tier_idx = tier as usize;
                let successes = self.success_counters[tier_idx].load(Ordering::Relaxed);
                successes < self.breaker_config.half_open_test_count
            }
            CircuitState::Open => false,
        }
    }

    /// Get failure count for tier
    #[inline(always)]
    pub fn get_failure_count(&self, tier: u8) -> u32 {
        if tier >= 3 {
            return 0;
        }
        self.failure_counters[tier as usize].load(Ordering::Relaxed)
    }

    /// Get success count for tier
    #[inline(always)]
    pub fn get_success_count(&self, tier: u8) -> u32 {
        if tier >= 3 {
            return 0;
        }
        self.success_counters[tier as usize].load(Ordering::Relaxed)
    }

    /// Reset circuit breaker for tier
    pub fn reset_tier(&self, tier: u8) {
        if tier >= 3 {
            return;
        }

        let tier_idx = tier as usize;
        self.tier_states[tier_idx].store(CircuitState::Closed);
        self.failure_counters[tier_idx].store(0, Ordering::Relaxed);
        self.success_counters[tier_idx].store(0, Ordering::Relaxed);
        self.state_transitions[tier_idx].store(Instant::now());
    }

    /// Reset all circuit breakers
    pub fn reset_all(&self) {
        for tier in 0..3 {
            self.reset_tier(tier);
        }
    }

    /// Reset circuit breaker (alias for reset_all for backward compatibility)
    pub fn reset(&self) {
        self.reset_all();
    }

    /// Get circuit breaker configuration
    pub fn get_config(&self) -> &CircuitBreakerConfig {
        &self.breaker_config
    }

    /// Update circuit breaker configuration
    pub fn update_config(&mut self, config: CircuitBreakerConfig) {
        self.breaker_config = config;
    }

    /// Force circuit state for tier (for testing)
    pub fn force_state(&self, tier: u8, state: CircuitState) {
        if tier >= 3 {
            return;
        }

        let tier_idx = tier as usize;
        self.tier_states[tier_idx].store(state);
        self.state_transitions[tier_idx].store(Instant::now());
    }

    /// Get time since last state transition
    pub fn time_since_transition(&self, tier: u8) -> Option<std::time::Duration> {
        if tier >= 3 {
            return None;
        }

        let tier_idx = tier as usize;
        let last_transition = self.state_transitions[tier_idx].load();
        Some(Instant::now().duration_since(last_transition))
    }

    /// Get circuit breaker health summary
    pub fn get_health_summary(&self) -> CircuitBreakerHealth {
        CircuitBreakerHealth {
            hot_tier_state: self.get_state(0),
            warm_tier_state: self.get_state(1),
            cold_tier_state: self.get_state(2),
            total_failures: self
                .failure_counters
                .iter()
                .map(|c| c.load(Ordering::Relaxed))
                .sum(),
            total_successes: self
                .success_counters
                .iter()
                .map(|c| c.load(Ordering::Relaxed))
                .sum(),
        }
    }
}

/// Circuit breaker health summary
#[derive(Debug, Clone)]
pub struct CircuitBreakerHealth {
    pub hot_tier_state: CircuitState,
    pub warm_tier_state: CircuitState,
    pub cold_tier_state: CircuitState,
    pub total_failures: u32,
    pub total_successes: u32,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}
