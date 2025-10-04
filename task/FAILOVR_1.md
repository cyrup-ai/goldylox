# FAILOVR_1: Implement Tier Failover Logic in ErrorRecoverySystem

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

Implement actual tier failover functionality in ErrorRecoverySystem. Currently the TierFailover recovery strategy only logs and records failures without redirecting traffic to healthy tiers, causing operation failures instead of graceful degradation.

**CRITICAL:** Do NOT write unit tests or benchmarks. Another team handles testing.

---

## CONTEXT

**File:** `src/cache/manager/error_recovery.rs:184-189`

**Current stub:**
```rust
RecoveryStrategy::TierFailover => {
    log::info!("Initiating tier failover from tier {}", tier);
    // In real implementation, this would coordinate with tier manager
    // For now, just record the failure and let circuit breaker handle it
    self.circuit_breakers[tier_idx].record_failure();
    true
}
```

**Problem:** When a tier fails, the system only records it but doesn't redirect traffic. Operations continue to fail.

**Impact:** HIGH - No graceful degradation; tier failures cascade to operation failures.

---

## SUBTASK 1: Add tier_operations Coordinator to ErrorRecoverySystem

**Location:** `src/cache/manager/error_recovery.rs` - ErrorRecoverySystem struct

**Action:** Add tier_operations field for coordination

**Current structure (approximate):**
```rust
pub struct ErrorRecoverySystem {
    circuit_breakers: Vec<CircuitBreaker>,
    recovery_stats: Arc<RecoveryStatistics>,
    // ...
}
```

**Updated structure:**
```rust
pub struct ErrorRecoverySystem {
    circuit_breakers: Vec<CircuitBreaker>,
    recovery_stats: Arc<RecoveryStatistics>,
    tier_operations: Arc<TierOperations>,  // Add this
    // ...
}
```

**Import needed:**
```rust
use crate::cache::coordinator::tier_operations::TierOperations;
```

---

## SUBTASK 2: Update ErrorRecoverySystem::new() Signature

**Location:** `src/cache/manager/error_recovery.rs` - new() method

**Action:** Accept tier_operations parameter

**Updated signature:**
```rust
pub fn new(
    tier_operations: Arc<TierOperations>,
    /* other existing params */
) -> Self {
    Self {
        tier_operations,
        /* other existing fields */
    }
}
```

---

## SUBTASK 3: Implement Tier Failover Logic

**Location:** `src/cache/manager/error_recovery.rs:184-189`

**Implementation:**
```rust
RecoveryStrategy::TierFailover => {
    log::info!("Initiating tier failover from tier {:?}", tier);

    // Determine failover target tier
    let target_tier = match tier {
        CacheTier::Hot => CacheTier::Warm,
        CacheTier::Warm => CacheTier::Cold,
        CacheTier::Cold => {
            log::error!("Cannot failover from Cold tier - no lower tier available");
            self.circuit_breakers[tier_idx].record_failure();
            return false;
        }
    };

    // Coordinate tier failover through tier operations
    match self.tier_operations.failover_tier(tier, target_tier) {
        Ok(()) => {
            log::info!("Successfully failed over from {:?} to {:?}", tier, target_tier);
            self.circuit_breakers[tier_idx].record_failure();

            // Mark original tier as degraded
            self.mark_tier_degraded(tier);

            // Schedule recovery attempt
            self.schedule_tier_recovery(tier);

            true
        }
        Err(e) => {
            log::error!("Tier failover failed: {:?}", e);
            self.circuit_breakers[tier_idx].record_failure();
            false
        }
    }
}
```

---

## SUBTASK 4: Implement failover_tier() in TierOperations

**Location:** `src/cache/coordinator/tier_operations.rs`

**Action:** Add failover_tier method to TierOperations

**Method signature:**
```rust
pub fn failover_tier(
    &self,
    from_tier: CacheTier,
    to_tier: CacheTier,
) -> Result<(), CacheOperationError> {
    log::info!("Failing over from {:?} to {:?}", from_tier, to_tier);

    // Mark source tier as degraded/offline
    self.mark_tier_offline(from_tier)?;

    // Redirect future operations to target tier
    self.set_failover_route(from_tier, to_tier)?;

    // Optionally: Copy hot data from failed tier to target (if accessible)
    // self.migrate_hot_data(from_tier, to_tier)?;

    log::info!("Tier failover completed: {:?} -> {:?}", from_tier, to_tier);
    Ok(())
}
```

**Supporting methods to implement:**
```rust
fn mark_tier_offline(&self, tier: CacheTier) -> Result<(), CacheOperationError> {
    // Update tier status to offline/degraded
    // Prevent new operations from targeting this tier
}

fn set_failover_route(&self, from: CacheTier, to: CacheTier) -> Result<(), CacheOperationError> {
    // Configure routing: operations targeting 'from' go to 'to'
    // Update internal routing table/map
}
```

**Note:** Exact implementation depends on TierOperations architecture. May need to add failover routing state to TierOperations struct.

---

## SUBTASK 5: Implement mark_tier_degraded() in ErrorRecoverySystem

**Location:** `src/cache/manager/error_recovery.rs`

**Action:** Add helper method to mark tier as degraded

**Implementation:**
```rust
fn mark_tier_degraded(&self, tier: CacheTier) {
    let tier_idx = tier as usize;
    log::warn!("Marking tier {:?} as degraded", tier);

    // Mark in circuit breaker (may already be done)
    self.circuit_breakers[tier_idx].record_failure();

    // Update recovery statistics
    self.recovery_stats.record_tier_degradation(tier);

    // Optionally: Emit metric/event for monitoring
}
```

---

## SUBTASK 6: Implement schedule_tier_recovery() in ErrorRecoverySystem

**Location:** `src/cache/manager/error_recovery.rs`

**Action:** Add helper method to schedule tier recovery

**Implementation:**
```rust
fn schedule_tier_recovery(&self, tier: CacheTier) {
    log::info!("Scheduling recovery attempt for tier {:?}", tier);

    // Schedule recovery using background worker system
    let recovery_task = RecoveryTask {
        tier,
        attempt_time: std::time::Instant::now() + std::time::Duration::from_secs(30),
        max_retries: 3,
    };

    // Submit to task queue or background coordinator
    // Exact mechanism depends on existing background worker architecture

    log::debug!("Recovery task scheduled for tier {:?} in 30s", tier);
}
```

**Note:** May need to integrate with existing background task system. Check `src/cache/manager/background/` for task submission API.

---

## SUBTASK 7: Update ErrorRecoverySystem Creation Sites

**Action:** Find all ErrorRecoverySystem::new() calls and pass tier_operations

**Search for:** `ErrorRecoverySystem::new(`

**Update each site:**
```rust
// Old:
let recovery = ErrorRecoverySystem::new(/* params */);

// New (requires tier_operations available in context):
let recovery = ErrorRecoverySystem::new(
    tier_operations.clone(),
    /* other params */
);
```

**Note:** Creation site is likely in UnifiedCacheManager::new(). Ensure tier_operations is available.

---

## DEFINITION OF DONE

1. ✅ tier_operations field added to ErrorRecoverySystem
2. ✅ ErrorRecoverySystem::new() accepts tier_operations parameter
3. ✅ TierFailover strategy implements actual failover logic
4. ✅ TierOperations.failover_tier() method implemented
5. ✅ mark_tier_degraded() helper implemented
6. ✅ schedule_tier_recovery() helper implemented
7. ✅ Failover routes traffic from failed tier to healthy tier
8. ✅ All creation sites updated to pass tier_operations
9. ✅ Compilation succeeds with `cargo check`
10. ✅ No stub comments remain in implementation

---

## RESEARCH NOTES

### Tier Hierarchy
- **Hot → Warm:** First failover level
- **Warm → Cold:** Second failover level
- **Cold → (none):** Cannot failover; final failure

### TierOperations
Located: `src/cache/coordinator/tier_operations.rs`

Current capabilities:
- Tier placement analysis
- Promotion/demotion coordination
- May need failover routing addition

### Circuit Breaker
Part of ErrorRecoverySystem:
- Tracks failure rates per tier
- Opens circuit when threshold exceeded
- Already integrated with tier_idx

### Background Recovery
May integrate with:
- `src/cache/manager/background/` - Background coordinator
- `src/cache/worker/` - Worker system
- Or implement simple delayed retry mechanism

### Key Files
- `src/cache/manager/error_recovery.rs` - Primary implementation
- `src/cache/coordinator/tier_operations.rs` - Failover coordination
- `src/cache/manager/background/` - Recovery scheduling (optional)
- `src/cache/coordinator/unified_manager.rs` - ErrorRecoverySystem creation site

---

**DO NOT write tests or benchmarks - another team handles that.**
