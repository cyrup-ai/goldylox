# STUB_3: Implement Error Recovery Tier Failover

---

## ⚠️ CRITICAL ARCHITECTURE NOTE

**NO Arc<T> ALLOWED IN THIS CODEBASE**

This system uses:
- **Crossbeam channels** for message passing
- **DashMap** for concurrent data structures
- **Worker-owned data** - workers OWN all state, no sharing

**Any solution using Arc<T> violates core architecture and will be rejected.**

Pattern: Send messages via channels. Workers own receivers and all data.

---


## OBJECTIVE
Replace the critical "for now" stub in the error recovery system's tier failover strategy. Currently, the system claims to support tier failover but only records failures without performing actual failover, leaving the cache vulnerable to tier failures.

---

## CURRENT PROBLEM

**File:** `src/cache/manager/error_recovery.rs`  
**Line:** 187

**Current Code:**
```rust
RecoveryStrategy::TierFailover => {
    log::info!("Initiating tier failover from tier {}", tier);
    // In real implementation, this would coordinate with tier manager
    For now  // ❌ REMOVE THIS STUB
    self.circuit_breakers[tier_idx].record_failure();
    true
}
```

**Impact:** When a tier fails, the system logs "Initiating tier failover" and records the failure in the circuit breaker, but doesn't actually perform failover. Requests continue to be routed to the failed tier, causing cascading failures.

---

## SUBTASK 1: Understand Tier Failover Architecture

**Research Required:**

**File:** `src/cache/manager/error_recovery.rs`
- Review `ErrorRecoverySystem` struct and recovery strategies
- Understand circuit breaker states and transitions
- Identify what "tier failover" means in this context

**File:** `src/cache/tier/manager.rs`
- Review `TierPromotionManager` implementation
- Understand tier coordination and routing
- Identify how requests are routed to tiers

**File:** `src/cache/coordinator/tier_operations.rs`
- Review how tier operations access different tiers
- Understand tier priority and fallback logic
- Identify where failover routing should be implemented

**What to Document:**
- What failover means: Skip failed tier? Route to alternative tier?
- How to mark a tier as "failed" or "unavailable"
- How to route requests around failed tiers
- How to coordinate with TierPromotionManager
- Recovery conditions: When does failed tier come back online?

---

## SUBTASK 2: Implement Tier Failover Coordination

**Location:** `src/cache/manager/error_recovery.rs:184-190`

**What Needs to Change:**
1. Remove the "For now" stub comment
2. Implement actual tier failover logic:
   - Notify tier manager that tier is unavailable
   - Update tier routing to skip failed tier
   - Redirect future requests to healthy tiers
   - Set up recovery monitoring for failed tier
   - Return true if failover successful, false if not

**Current Context:**
```rust
fn attempt_recovery(&mut self, tier: usize) -> bool {
    // ... circuit breaker logic ...
    
    match self.recovery_strategies.get(tier) {
        Some(RecoveryStrategy::TierFailover) => {
            log::info!("Initiating tier failover from tier {}", tier);
            // In real implementation, this would coordinate with tier manager
            For now  // ❌ IMPLEMENT THIS
            self.circuit_breakers[tier_idx].record_failure();
            true
        }
        // ... other strategies ...
    }
}
```

**Implementation Approach:**
The error recovery system needs to communicate tier failure to the tier manager. This requires:
- Access to `TierPromotionManager` or tier coordination system
- Method to mark tier as unavailable
- Mechanism to route requests around failed tier

**Possible Solutions:**

**Option A: Add tier_manager reference to ErrorRecoverySystem**
```rust
pub struct ErrorRecoverySystem<K, V> {
    // ... existing fields ...
    tier_manager: Arc<TierPromotionManager<K>>,  // Add this
}
```

**Option B: Use channel-based coordination**
```rust
// Send failover message to tier manager via channel
self.failover_sender.send(TierFailoverMessage {
    failed_tier: tier,
    timestamp: Instant::now(),
})?;
```

**Option C: Update shared tier availability state**
```rust
// Update shared atomic tier availability flags
self.tier_availability[tier].store(false, Ordering::Release);
```

---

## SUBTASK 3: Coordinate with Tier Manager

**Required Changes:**

**In error_recovery.rs:**
- Add necessary references (tier_manager, channels, or shared state)
- Implement failover notification
- Handle failover errors (what if failover fails?)

**Potential Changes in tier_operations.rs:**
- Check tier availability before routing requests
- Skip unavailable tiers during tier access attempts
- Fall through to next tier if current tier unavailable

**Potential Changes in unified_manager.rs:**
- Pass tier availability state to ErrorRecoverySystem
- Ensure ErrorRecoverySystem can coordinate with tier operations

**Implementation Pattern:**
```rust
RecoveryStrategy::TierFailover => {
    log::info!("Initiating tier failover from tier {}", tier);
    
    // Mark tier as unavailable
    if let Some(tier_manager) = self.tier_manager.as_ref() {
        match tier_manager.mark_tier_unavailable(tier) {
            Ok(_) => {
                log::info!("Tier {} marked unavailable, requests will route to healthy tiers", tier);
                self.circuit_breakers[tier_idx].record_failure();
                
                // Schedule recovery check
                self.schedule_tier_recovery_check(tier);
                true
            }
            Err(e) => {
                log::error!("Failed to failover tier {}: {:?}", tier, e);
                false
            }
        }
    } else {
        log::warn!("Cannot perform tier failover: no tier manager reference");
        self.circuit_breakers[tier_idx].record_failure();
        false
    }
}
```

---

## SUBTASK 4: Implement Recovery Monitoring

**What Happens After Failover:**
Failed tiers should eventually be checked for recovery:
- Periodic health checks on failed tier
- When tier becomes healthy, mark as available again
- Circuit breaker transitions from Open → Half-Open → Closed

**Implementation:**
- Use existing circuit breaker half-open state for recovery attempts
- When circuit breaker allows test request, verify tier health
- If tier responds successfully, mark available again

---

## DEFINITION OF DONE

**Verification Steps:**
1. Remove "For now" stub comment from `error_recovery.rs:187`
2. Implement actual tier failover coordination
3. Failed tiers are marked unavailable
4. Requests route around failed tiers
5. Recovery monitoring re-enables recovered tiers
6. Code compiles: `cargo check`
7. Failover returns true on success, false on failure

**Success Criteria:**
- Tier failover actually prevents requests to failed tier
- Circuit breaker and tier availability coordinated
- Requests successfully routed to healthy tiers during failover
- Failed tiers eventually recovered when healthy
- No cascading failures from single tier failure
- Proper error handling and logging

---

## RESEARCH NOTES

**Error Recovery Architecture:**

**Relevant Files:**
- `src/cache/manager/error_recovery.rs` - Error recovery system with circuit breakers
- `src/cache/tier/manager.rs` - Tier promotion/demotion manager
- `src/cache/coordinator/tier_operations.rs` - Tier access operations
- `src/cache/coordinator/unified_manager.rs` - Main cache manager coordination

**Key Data Structures:**
- `ErrorRecoverySystem<K, V>` - Main error recovery coordinator
- `RecoveryStrategy` - Enum of recovery strategies (TierFailover, Retry, etc.)
- `CircuitBreaker` - Per-tier circuit breaker state
- `TierPromotionManager<K>` - Manages tier transitions

**Recovery Strategies:**
- `Retry` - Retry failed operation
- `TierFailover` - Route to different tier (THIS ONE)
- `Graceful` - Degrade gracefully
- `Emergency` - Emergency handling

**Circuit Breaker States:**
- Closed: Normal operation
- Open: Failures detected, blocking requests
- HalfOpen: Testing if service recovered

---

## IMPLEMENTATION CONSTRAINTS

**DO NOT:**
- ❌ Write unit tests (separate team handles testing)
- ❌ Write integration tests (separate team handles testing)
- ❌ Write benchmarks (separate team handles benchmarks)
- ❌ Change circuit breaker logic unless necessary
- ❌ Modify tier operation APIs unnecessarily

**DO:**
- ✅ Focus solely on removing stub with production code
- ✅ Coordinate with existing tier manager
- ✅ Use circuit breaker states appropriately
- ✅ Handle failover errors gracefully
- ✅ Log failover events for observability
- ✅ Follow existing error recovery patterns
- ✅ Maintain thread safety

---

## NOTES

**Complexity:** Medium-High
- Requires coordinating multiple subsystems (error recovery, tier manager, tier operations)
- Need to understand tier routing logic
- Must maintain system stability during failover
- Circuit breaker integration important

**Session Focus:** Single recovery strategy implementation with coordination across error recovery and tier management systems.

**Key Question to Answer:** How should tier availability be tracked and communicated? This determines the implementation approach (shared state vs messages vs direct calls).

**Recommended Approach:** 
1. Review how TierPromotionManager works
2. Add tier availability tracking (atomic flags or shared state)
3. Update tier operations to check availability
4. Implement failover to set availability flags
5. Use circuit breaker half-open state for recovery
