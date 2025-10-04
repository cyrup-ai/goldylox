# POOLFIX_3: Fix Placeholder Tier Coordinators in AllocationManager Thread

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

Fix AllocationManager's spawned thread that creates placeholder hot/warm/cold tier coordinators with empty DashMaps. These coordinators have no backing tier instances, causing all coordination requests to fail.

**CRITICAL:** Do NOT write unit tests or benchmarks. Another team handles testing.

---

## CONTEXT

**File:** `src/cache/memory/allocation_manager.rs:329-344`

**Current Implementation:**
```rust
let hot_coord = std::sync::Arc::new(crate::cache::tier::hot::thread_local::HotTierCoordinator {
    hot_tiers: dashmap::DashMap::new(),  // EMPTY - no tier instances!
    instance_selector: std::sync::atomic::AtomicUsize::new(0),
});
// Similar for warm_coord and cold_coord
```

**Problem:**
- Empty DashMaps contain zero tier instances
- Any tier coordination operation will find nothing
- MaintenanceScheduler cannot access actual cache tiers

**Impact:** HIGH - Maintenance operations cannot coordinate with tiers.

---

## SUBTASK 1: Add Tier Coordinators to AllocationManager::new() Signature

**Location:** `src/cache/memory/allocation_manager.rs` - `new()` function signature

**Current signature:**
```rust
pub fn new(
    config: &CacheConfig,
) -> Result<(Self, std::sync::Arc<PoolCoordinator>), CacheOperationError>
```

**New signature:**
```rust
pub fn new(
    config: &CacheConfig,
    hot_coordinator: std::sync::Arc<crate::cache::tier::hot::thread_local::HotTierCoordinator>,
    warm_coordinator: std::sync::Arc<crate::cache::tier::warm::global_api::WarmTierCoordinator>,
    cold_coordinator: std::sync::Arc<crate::cache::tier::cold::global_api::ColdTierCoordinator>,
) -> Result<(Self, std::sync::Arc<PoolCoordinator>), CacheOperationError>
```

**Why:** AllocationManager needs working coordinators from UnifiedCacheManager, not empty placeholders.

---

## SUBTASK 2: Clone Coordinators Before Thread Move

**Location:** `src/cache/memory/allocation_manager.rs` - before thread spawn

**Action:** Clone tier coordinators before moving into thread closure

**Implementation:**
```rust
// Clone coordinators for thread (before spawn around line 324)
let hot_coord_for_thread = hot_coordinator.clone();
let warm_coord_for_thread = warm_coordinator.clone();
let cold_coord_for_thread = cold_coordinator.clone();

std::thread::Builder::new()
    .name("allocation-pool-defrag".to_string())
    .spawn(move || {
        // Use cloned coordinators (moved into closure)
        // ...
    })?;
```

**Why:** Thread move requires ownership; clone before move to retain coordinator in AllocationManager.

---

## SUBTASK 3: Remove Placeholder Coordinator Creation

**Location:** `src/cache/memory/allocation_manager.rs:329-344`

**Action:** Delete all placeholder coordinator creation code

**Delete these lines:**
```rust
// DELETE ENTIRE BLOCK:
let hot_coord = std::sync::Arc::new(crate::cache::tier::hot::thread_local::HotTierCoordinator {
    hot_tiers: dashmap::DashMap::new(),
    instance_selector: std::sync::atomic::AtomicUsize::new(0),
});

let warm_coord = std::sync::Arc::new(crate::cache::tier::warm::global_api::WarmTierCoordinator {
    warm_tiers: dashmap::DashMap::new(),
    instance_selector: std::sync::atomic::AtomicUsize::new(0),
});

let cold_coord = match crate::cache::tier::cold::global_api::ColdTierCoordinator::new() {
    Ok(coord) => std::sync::Arc::new(coord),
    Err(e) => {
        log::error!("Failed to create cold tier coordinator: {:?}", e);
        return;
    }
};
```

**Replace with:** Use cloned coordinators from parameters (from SUBTASK 2).

---

## SUBTASK 4: Update MaintenanceScheduler::new() Call

**Location:** `src/cache/memory/allocation_manager.rs:349` (approximate)

**Action:** Pass cloned coordinators to MaintenanceScheduler

**Implementation:**
```rust
let scheduler = match MaintenanceScheduler::<K, V>::new(
    MaintenanceConfig::default(),
    unified_stats_arc,
    coherence_stats_arc,
    hot_coord_for_thread,   // From cloned parameter
    warm_coord_for_thread,  // From cloned parameter
    cold_coord_for_thread,  // From cloned parameter
    pool_coord,             // From POOLFIX_1
) {
    Ok(s) => s,
    Err(e) => {
        log::error!("Failed to create maintenance scheduler: {:?}", e);
        return;
    }
};
```

**Why:** MaintenanceScheduler needs working coordinators to access actual tiers.

---

## SUBTASK 5: Update UnifiedCacheManager::new() Call Site

**Location:** `src/cache/coordinator/unified_manager.rs` - AllocationManager::new() call

**Current call (approximate line 388):**
```rust
let (allocation_manager, pool_coordinator) = AllocationManager::new(&config)?;
```

**Updated call:**
```rust
let (allocation_manager, pool_coordinator) = AllocationManager::new(
    &config,
    hot_tier_coordinator.clone(),
    warm_tier_coordinator.clone(),
    cold_tier_coordinator.clone(),
)?;
```

**Why:** Pass working coordinators from UnifiedCacheManager to AllocationManager.

---

## SUBTASK 6: Verify UnifiedCacheStatistics Creation

**Location:** Check if UnifiedCacheStatistics also has placeholder coordinator issue

**Action:** Search for UnifiedCacheStatistics::new() in allocation_manager.rs (line ~327)

**Current (likely):**
```rust
let unified_stats_arc = std::sync::Arc::new(UnifiedCacheStatistics::new());
```

**If UnifiedCacheStatistics has placeholder coordinator bug (from TURD.md Issue #8):**
This will be fixed in separate task STATFIX_1. For now, leave as-is.

**Note in comments:**
```rust
// TODO: UnifiedCacheStatistics needs warm_coordinator parameter (see STATFIX_1)
let unified_stats_arc = std::sync::Arc::new(UnifiedCacheStatistics::new());
```

---

## DEFINITION OF DONE

1. ✅ AllocationManager::new() accepts tier coordinators as parameters
2. ✅ Coordinators are cloned before thread spawn
3. ✅ Placeholder coordinator creation removed (lines 329-344)
4. ✅ MaintenanceScheduler receives working tier coordinators
5. ✅ UnifiedCacheManager passes real coordinators to AllocationManager
6. ✅ Compilation succeeds with `cargo check`
7. ✅ No "unused variable" warnings for coordinators

---

## RESEARCH NOTES

### Coordinator Types
- **HotTierCoordinator:** `src/cache/tier/hot/thread_local.rs`
- **WarmTierCoordinator:** `src/cache/tier/warm/global_api.rs`
- **ColdTierCoordinator:** `src/cache/tier/cold/global_api.rs`

### Coordinator Pattern
- Coordinators contain `DashMap<TypeId, Arc<ActualTierInstance>>`
- Empty DashMap = no tier instances = coordination fails
- Must receive populated coordinators from cache manager

### MaintenanceScheduler
Located: Likely `src/cache/tier/warm/maintenance/scheduler.rs`

Signature (expected):
```rust
pub fn new<K, V>(
    config: MaintenanceConfig,
    unified_stats: Arc<UnifiedCacheStatistics>,
    coherence_stats: Arc<CoherenceStatistics>,
    hot_coordinator: Arc<HotTierCoordinator>,
    warm_coordinator: Arc<WarmTierCoordinator>,
    cold_coordinator: Arc<ColdTierCoordinator>,
    pool_coordinator: Arc<PoolCoordinator>,
) -> Result<Self, CacheOperationError>
```

### Related Issues
- POOLFIX_1: Fix dummy pool_coordinator (same pattern)
- STATFIX_1: UnifiedCacheStatistics placeholder coordinator (separate task)

### Key Files
- `src/cache/memory/allocation_manager.rs` - Primary fix location
- `src/cache/coordinator/unified_manager.rs` - Call site update
- `src/cache/tier/*/global_api.rs` - Coordinator definitions

---

**DO NOT write tests or benchmarks - another team handles that.**
