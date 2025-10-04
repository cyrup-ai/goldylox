# STATFIX_2: Fix Placeholder Coordinators in Statistics Systems

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

Fix placeholder tier coordinators in UnifiedCacheStatistics and CoherenceProtocolManager. Both create empty coordinators with no backing tier instances, causing statistics collection and coherence operations to fail.

**CRITICAL:** Do NOT write unit tests or benchmarks. Another team handles testing.

---

## CONTEXT

**Issue A - UnifiedCacheStatistics:**
- File: `src/telemetry/unified_stats.rs:499-502`
- Creates placeholder warm_coord with empty DashMap
- Statistics collection from warm tier will fail

**Issue B - CoherenceProtocolManager:**
- File: `src/cache/coherence/protocol/global_api.rs:76-90`
- Creates placeholder hot/warm/cold coordinators
- Coherence worker operations will fail

**Impact:** Medium - Statistics and coherence operations non-functional.

---

## PART A: FIX UNIFIEDCACHESTATISTICS

### SUBTASK A1: Add warm_coordinator to UnifiedCacheStatistics Struct

**Location:** `src/telemetry/unified_stats.rs` - UnifiedCacheStatistics struct

**Action:** Add warm_coordinator field (if not already present)

**Check if field exists:**
If field already exists but unused, skip to A2.

**If missing, add:**
```rust
pub struct UnifiedCacheStatistics {
    // ... existing fields
    warm_coordinator: Arc<WarmTierCoordinator>,  // Add if missing
}
```

---

### SUBTASK A2: Update UnifiedCacheStatistics::new() Signature

**Location:** `src/telemetry/unified_stats.rs` - new() method

**Action:** Accept warm_coordinator parameter

**Current signature (approximate):**
```rust
pub fn new() -> Self
```

**Updated signature:**
```rust
pub fn new(
    warm_coordinator: Arc<WarmTierCoordinator>,
) -> Self {
    Self {
        // ... existing fields
        warm_coordinator,
    }
}
```

---

### SUBTASK A3: Remove Placeholder Coordinator Creation

**Location:** `src/telemetry/unified_stats.rs:499-502`

**Action:** Delete placeholder creation code

**Delete these lines:**
```rust
// DELETE:
// Create placeholder coordinator for stats (legacy code)
let warm_coord = std::sync::Arc::new(crate::cache::tier::warm::global_api::WarmTierCoordinator {
    warm_tiers: dashmap::DashMap::new(),
    instance_selector: std::sync::atomic::AtomicUsize::new(0),
});
```

**Replace with:** Use `self.warm_coordinator` directly where warm_coord was used.

---

### SUBTASK A4: Update UnifiedCacheStatistics Creation Sites

**Action:** Find all UnifiedCacheStatistics::new() calls and pass warm_coordinator

**Search for:** `UnifiedCacheStatistics::new(`

**Likely locations:**
- `src/cache/coordinator/unified_manager.rs`
- `src/cache/memory/allocation_manager.rs`

**Update each site:**
```rust
// Old:
let stats = Arc::new(UnifiedCacheStatistics::new());

// New:
let stats = Arc::new(UnifiedCacheStatistics::new(
    warm_tier_coordinator.clone(),
));
```

**Note:** warm_tier_coordinator must be available in context. In UnifiedCacheManager it's already created. In AllocationManager thread, it will be available from POOLFIX_3 fix.

---

## PART B: FIX COHERENCEPROTOCOLMANAGER

### SUBTASK B1: Add Coordinators to get_or_init() Signature

**Location:** `src/cache/coherence/protocol/global_api.rs` - CoherenceProtocolManager

**Action:** Update get_or_init() to accept tier coordinators

**Current signature (approximate):**
```rust
pub fn get_or_init() -> Arc<Self>
```

**Updated signature:**
```rust
pub fn get_or_init(
    hot_coordinator: Arc<HotTierCoordinator>,
    warm_coordinator: Arc<WarmTierCoordinator>,
    cold_coordinator: Arc<ColdTierCoordinator>,
) -> Arc<Self>
```

---

### SUBTASK B2: Remove Placeholder Coordinator Creation

**Location:** `src/cache/coherence/protocol/global_api.rs:76-90`

**Action:** Delete placeholder creation, use passed coordinators

**Delete these lines:**
```rust
// DELETE:
// Create placeholder coordinators for the worker manager
// NOTE: This coherence worker system is secondary to the main TierOperations coherence
let hot_coordinator = std::sync::Arc::new(crate::cache::tier::hot::thread_local::HotTierCoordinator {
    hot_tiers: dashmap::DashMap::new(),
    instance_selector: std::sync::atomic::AtomicUsize::new(0),
});
// ... similar for warm and cold
```

**Replace with:** Use passed parameters directly in WorkerManager::new() call.

---

### SUBTASK B3: Update WorkerManager::new() Call

**Location:** `src/cache/coherence/protocol/global_api.rs` - inside get_or_init()

**Action:** Pass received coordinators to WorkerManager

**Implementation:**
```rust
let worker_manager = WorkerManager::new(
    4,  // num_workers
    hot_coordinator,      // From parameter
    warm_coordinator,     // From parameter
    cold_coordinator,     // From parameter
);
```

---

### SUBTASK B4: Update CoherenceProtocolManager Call Sites

**Action:** Find all get_or_init() calls and pass coordinators

**Search for:** `CoherenceProtocolManager::get_or_init(`

**Update each site:**
```rust
// Old:
let coherence_mgr = CoherenceProtocolManager::get_or_init();

// New:
let coherence_mgr = CoherenceProtocolManager::get_or_init(
    hot_tier_coordinator.clone(),
    warm_tier_coordinator.clone(),
    cold_tier_coordinator.clone(),
);
```

**Likely location:** `src/cache/coordinator/unified_manager.rs`

---

### SUBTASK B5: Handle Static Instance Pattern

**Note:** get_or_init() suggests a singleton/static pattern (OnceCell/OnceLock).

**If using OnceCell:**
```rust
static COHERENCE_PROTOCOL: OnceCell<Arc<CoherenceProtocolManager>> = OnceCell::new();

pub fn get_or_init(
    hot_coordinator: Arc<HotTierCoordinator>,
    warm_coordinator: Arc<WarmTierCoordinator>,
    cold_coordinator: Arc<ColdTierCoordinator>,
) -> Arc<Self> {
    COHERENCE_PROTOCOL.get_or_init(|| {
        Arc::new(Self::new(hot_coordinator, warm_coordinator, cold_coordinator))
    }).clone()
}
```

**Problem:** OnceCell initialization only happens once. Later calls ignore parameters.

**Solution:** Either:
- **Option A:** Switch to per-instance (remove static) - RECOMMENDED
- **Option B:** Store coordinators in struct, ensure first call has correct coordinators
- **Option C:** Pass coordinators on every method call, not just initialization

**Choose Option A if possible:** Make CoherenceProtocolManager per-instance, not global.

---

## DEFINITION OF DONE

### Part A - UnifiedCacheStatistics:
1. ✅ warm_coordinator parameter added to new()
2. ✅ Placeholder coordinator creation removed (lines 499-502)
3. ✅ All creation sites pass warm_coordinator
4. ✅ Compilation succeeds for telemetry module

### Part B - CoherenceProtocolManager:
5. ✅ Tier coordinators added to get_or_init() parameters
6. ✅ Placeholder coordinator creation removed (lines 76-90)
7. ✅ WorkerManager receives real coordinators
8. ✅ All get_or_init() call sites updated
9. ✅ Static instance pattern resolved (preferably to per-instance)
10. ✅ Compilation succeeds with `cargo check`

---

## RESEARCH NOTES

### UnifiedCacheStatistics
Located: `src/telemetry/unified_stats.rs`

Purpose: Collect and aggregate statistics from all cache tiers.

Uses warm_coordinator to:
- Query warm tier instances
- Collect warm tier statistics
- Aggregate into unified view

### CoherenceProtocolManager
Located: `src/cache/coherence/protocol/global_api.rs`

Purpose: Coordinate coherence protocol across tiers.

Note in source: "This coherence worker system is secondary to the main TierOperations coherence"

May be legacy or supplementary system.

### WorkerManager
Coherence worker manager that needs access to tiers via coordinators.

### Tier Coordinator Types
- **HotTierCoordinator:** `src/cache/tier/hot/thread_local.rs`
- **WarmTierCoordinator:** `src/cache/tier/warm/global_api.rs`
- **ColdTierCoordinator:** `src/cache/tier/cold/global_api.rs`

### Key Files
- `src/telemetry/unified_stats.rs` - Part A
- `src/cache/coherence/protocol/global_api.rs` - Part B
- `src/cache/coordinator/unified_manager.rs` - Call sites

---

**DO NOT write tests or benchmarks - another team handles that.**
