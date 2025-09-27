//! State transition management for MESI cache coherence protocol
//!
//! This module implements state transition validation and management for the MESI protocol,
//! ensuring correct state transitions and protocol compliance.

// Internal coherence architecture - components may not be used in minimal API

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::cache::coherence::communication::CoherenceError;
use crate::cache::coherence::data_structures::{CacheTier, MesiState};
use crate::cache::coherence::statistics::core_statistics::CoherenceStatistics;

/// State transition validator for protocol correctness
#[allow(dead_code)] // MESI coherence - used in protocol state transition validation and enforcement
#[derive(Debug)]
pub struct StateTransitionValidator {
    /// Valid state transitions matrix
    valid_transitions: [[bool; 4]; 4],
    /// Transition statistics
    transition_stats: TransitionStatistics,
    /// Protocol violation detector
    violation_detector: ViolationDetector,
}

/// Statistics for state transitions
#[allow(dead_code)] // MESI coherence - used in protocol monitoring and telemetry collection
#[derive(Debug)]
pub struct TransitionStatistics {
    /// Total transition count
    #[allow(dead_code)]
    // MESI coherence - used in protocol transition monitoring and statistics collection
    pub transition_count: AtomicU64,
    /// Invalid transitions attempted
    #[allow(dead_code)]
    // MESI coherence - used in protocol violation tracking and error analysis
    pub invalid_transitions: AtomicU64,
    /// Transitions by state (from Invalid, Shared, Exclusive, Modified)
    #[allow(dead_code)] // MESI coherence - used in protocol state transition analytics
    pub transitions_from_invalid: AtomicU64,
    #[allow(dead_code)] // MESI coherence - used in protocol state transition analytics
    pub transitions_from_shared: AtomicU64,
    #[allow(dead_code)] // MESI coherence - used in protocol state transition analytics
    pub transitions_from_exclusive: AtomicU64,
    #[allow(dead_code)] // MESI coherence - used in protocol state transition analytics
    pub transitions_from_modified: AtomicU64,
    /// Transitions to state (to Invalid, Shared, Exclusive, Modified)
    #[allow(dead_code)] // MESI coherence - used in protocol state transition analytics
    pub transitions_to_invalid: AtomicU64,
    #[allow(dead_code)] // MESI coherence - used in protocol state transition analytics
    pub transitions_to_shared: AtomicU64,
    #[allow(dead_code)] // MESI coherence - used in protocol state transition analytics
    pub transitions_to_exclusive: AtomicU64,
    #[allow(dead_code)] // MESI coherence - used in protocol state transition analytics
    pub transitions_to_modified: AtomicU64,
}

/// Protocol violation detection and tracking
#[allow(dead_code)] // MESI coherence - used in protocol violation detection and error recovery
#[derive(Debug)]
pub struct ViolationDetector {
    /// Total violations detected
    #[allow(dead_code)]
    // MESI coherence - used in protocol violation monitoring and error recovery
    pub violation_count: AtomicU32,
    /// Concurrent write violations
    #[allow(dead_code)] // MESI coherence - used in protocol concurrency violation detection
    pub concurrent_write_violations: AtomicU32,
    /// Invalid state access violations
    #[allow(dead_code)] // MESI coherence - used in protocol invalid state access tracking
    pub invalid_state_violations: AtomicU32,
    /// Protocol ordering violations
    #[allow(dead_code)] // MESI coherence - used in protocol ordering violation detection
    pub ordering_violations: AtomicU32,
}

/// State transition request with context
#[allow(dead_code)] // MESI coherence - used in protocol state transition requests and validation
#[derive(Debug, Clone)]
pub struct StateTransitionRequest {
    pub from_state: MesiState,
    pub to_state: MesiState,
    pub tier: CacheTier,
    pub version: u64,
    pub timestamp_ns: u64,
    pub reason: TransitionReason,
}

/// Reasons for state transitions
#[allow(dead_code)] // MESI coherence - enum variants constructed in protocol transition logic
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionReason {
    /// Read operation
    Read,
    /// Write operation
    Write,
    /// Invalidation from another tier
    Invalidation,
    /// Eviction due to capacity
    Eviction,
    /// Explicit flush
    Flush,
    /// Protocol enforcement
    ProtocolEnforcement,
}

/// Result of state transition validation
#[allow(dead_code)] // MESI coherence - enum variants constructed in protocol validation logic
#[derive(Debug, Clone)]
pub enum TransitionResult {
    /// Transition is valid and allowed
    Valid,
    /// Transition is invalid
    Invalid { reason: String },
    /// Transition requires additional steps
    RequiresSteps { steps: Vec<MesiState> },
}

impl Default for StateTransitionValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl StateTransitionValidator {
    pub fn new() -> Self {
        // Initialize valid transition matrix for MESI protocol
        let mut valid_transitions = [[false; 4]; 4];

        // From Invalid (0)
        valid_transitions[0][1] = true; // Invalid -> Shared (read miss)
        valid_transitions[0][2] = true; // Invalid -> Exclusive (read miss, exclusive)
        valid_transitions[0][3] = true; // Invalid -> Modified (write miss)

        // From Shared (1)
        valid_transitions[1][0] = true; // Shared -> Invalid (invalidation)
        valid_transitions[1][2] = true; // Shared -> Exclusive (write hit, no other sharers)
        valid_transitions[1][3] = true; // Shared -> Modified (write hit)

        // From Exclusive (2)
        valid_transitions[2][0] = true; // Exclusive -> Invalid (invalidation)
        valid_transitions[2][1] = true; // Exclusive -> Shared (read by other tier)
        valid_transitions[2][3] = true; // Exclusive -> Modified (write)

        // From Modified (3)
        valid_transitions[3][0] = true; // Modified -> Invalid (invalidation)
        valid_transitions[3][1] = true; // Modified -> Shared (read by other tier, write-back)

        Self {
            valid_transitions,
            transition_stats: TransitionStatistics::new(),
            violation_detector: ViolationDetector::new(),
        }
    }

    /// Validate a state transition
    #[allow(dead_code)] // MESI coherence - used in protocol state transition validation logic
    pub fn validate_transition(&self, request: &StateTransitionRequest) -> TransitionResult {
        let from_idx = request.from_state as usize;
        let to_idx = request.to_state as usize;

        // Record transition attempt
        self.transition_stats
            .transition_count
            .fetch_add(1, Ordering::Relaxed);
        self.record_transition_stats(request.from_state, request.to_state);

        // Check if transition is valid in the matrix
        if !self.valid_transitions[from_idx][to_idx] {
            // Record to global coherence statistics
            CoherenceStatistics::global().record_violation();

            self.transition_stats
                .invalid_transitions
                .fetch_add(1, Ordering::Relaxed);
            self.violation_detector
                .violation_count
                .fetch_add(1, Ordering::Relaxed);

            return TransitionResult::Invalid {
                reason: format!(
                    "Invalid MESI transition from {:?} to {:?}",
                    request.from_state, request.to_state
                ),
            };
        }

        // Additional validation based on context
        match (request.from_state, request.to_state, request.reason) {
            // Write to Invalid state is not allowed
            (_, MesiState::Invalid, TransitionReason::Write) => {
                self.violation_detector
                    .invalid_state_violations
                    .fetch_add(1, Ordering::Relaxed);
                TransitionResult::Invalid {
                    reason: "Cannot write to invalid state".to_string(),
                }
            }

            // Direct transition from Shared to Modified might require invalidating other sharers
            (MesiState::Shared, MesiState::Modified, TransitionReason::Write) => {
                TransitionResult::RequiresSteps {
                    steps: vec![MesiState::Exclusive, MesiState::Modified],
                }
            }

            // All other valid transitions
            _ => TransitionResult::Valid,
        }
    }

    /// Execute a validated state transition
    #[allow(dead_code)] // MESI coherence - used in protocol state transition execution
    pub fn execute_transition(
        &self,
        request: &StateTransitionRequest,
    ) -> Result<(), CoherenceError> {
        match self.validate_transition(request) {
            TransitionResult::Valid => Ok(()),
            TransitionResult::Invalid { reason: _ } => {
                Err(CoherenceError::InvalidStateTransition {
                    from: request.from_state,
                    to: request.to_state,
                })
            }
            TransitionResult::RequiresSteps { steps } => {
                // Execute multi-step transitions sequentially
                let mut current_state = request.from_state;

                for step_state in steps {
                    // Create intermediate transition request
                    let step_request = StateTransitionRequest {
                        from_state: current_state,
                        to_state: step_state,
                        reason: request.reason,
                        tier: request.tier,
                        version: request.version,
                        timestamp_ns: request.timestamp_ns,
                    };

                    // Validate each intermediate step
                    match self.validate_transition(&step_request) {
                        TransitionResult::Valid => {
                            // Record the successful intermediate transition
                            self.record_transition_stats(current_state, step_state);
                            current_state = step_state;
                        }
                        TransitionResult::Invalid { reason: _ } => {
                            return Err(CoherenceError::InvalidStateTransition {
                                from: current_state,
                                to: step_state,
                            });
                        }
                        TransitionResult::RequiresSteps {
                            steps: _nested_steps,
                        } => {
                            // Handle nested multi-step transitions recursively
                            let nested_request = StateTransitionRequest {
                                from_state: current_state,
                                to_state: step_state,
                                reason: request.reason,
                                tier: request.tier,
                                version: request.version,
                                timestamp_ns: request.timestamp_ns,
                            };

                            // Recursive call to handle nested steps
                            self.execute_transition(&nested_request)?;
                            current_state = step_state;
                        }
                    }
                }

                // Verify we reached the target state
                if current_state == request.to_state {
                    Ok(())
                } else {
                    Err(CoherenceError::InvalidStateTransition {
                        from: current_state,
                        to: request.to_state,
                    })
                }
            }
        }
    }

    /// Record transition statistics
    fn record_transition_stats(&self, from_state: MesiState, to_state: MesiState) {
        // Record to global coherence statistics
        CoherenceStatistics::global().record_transition();

        // Record local transition statistics
        // Record from-state statistics
        match from_state {
            MesiState::Invalid => self
                .transition_stats
                .transitions_from_invalid
                .fetch_add(1, Ordering::Relaxed),
            MesiState::Shared => self
                .transition_stats
                .transitions_from_shared
                .fetch_add(1, Ordering::Relaxed),
            MesiState::Exclusive => self
                .transition_stats
                .transitions_from_exclusive
                .fetch_add(1, Ordering::Relaxed),
            MesiState::Modified => self
                .transition_stats
                .transitions_from_modified
                .fetch_add(1, Ordering::Relaxed),
        };

        // Record to-state statistics
        match to_state {
            MesiState::Invalid => self
                .transition_stats
                .transitions_to_invalid
                .fetch_add(1, Ordering::Relaxed),
            MesiState::Shared => self
                .transition_stats
                .transitions_to_shared
                .fetch_add(1, Ordering::Relaxed),
            MesiState::Exclusive => self
                .transition_stats
                .transitions_to_exclusive
                .fetch_add(1, Ordering::Relaxed),
            MesiState::Modified => self
                .transition_stats
                .transitions_to_modified
                .fetch_add(1, Ordering::Relaxed),
        };
    }

    /// Check if a state transition is theoretically valid
    #[allow(dead_code)] // MESI coherence - used in protocol state transition validation checks
    pub fn is_transition_valid(&self, from: MesiState, to: MesiState) -> bool {
        self.valid_transitions[from as usize][to as usize]
    }
}

impl Default for TransitionStatistics {
    fn default() -> Self {
        Self::new()
    }
}

impl TransitionStatistics {
    pub fn new() -> Self {
        Self {
            transition_count: AtomicU64::new(0),
            invalid_transitions: AtomicU64::new(0),
            transitions_from_invalid: AtomicU64::new(0),
            transitions_from_shared: AtomicU64::new(0),
            transitions_from_exclusive: AtomicU64::new(0),
            transitions_from_modified: AtomicU64::new(0),
            transitions_to_invalid: AtomicU64::new(0),
            transitions_to_shared: AtomicU64::new(0),
            transitions_to_exclusive: AtomicU64::new(0),
            transitions_to_modified: AtomicU64::new(0),
        }
    }
}

impl Default for ViolationDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ViolationDetector {
    pub fn new() -> Self {
        Self {
            violation_count: AtomicU32::new(0),
            concurrent_write_violations: AtomicU32::new(0),
            invalid_state_violations: AtomicU32::new(0),
            ordering_violations: AtomicU32::new(0),
        }
    }
}
