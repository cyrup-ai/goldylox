# POOLFIX_2: Implement TriggerDefragmentation in PoolCleanupWorker

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

Implement actual defragmentation logic in PoolCleanupWorker's TriggerDefragmentation handler. Currently returns success with 0 bytes without performing any defragmentation, causing memory fragmentation to accumulate unchecked.

**CRITICAL:** Do NOT write unit tests or benchmarks. Another team handles testing.

---

## CONTEXT

**File:** `src/cache/memory/allocation_manager.rs:271-275`

**Current Implementation:**
```rust
PoolCleanupRequest::TriggerDefragmentation { response } => {
    // Defragmentation logic would go here
    // For now, return success with 0 bytes (actual implementation needed)
    let result = Ok(0);
    let _ = response.send(result);
}
```

**Problem:** Defragmentation requests are acknowledged but ignored. Memory fragmentation accumulates, degrading performance.

**Impact:** HIGH - Critical memory management operation is non-functional.

---

## SUBTASK 1: Implement Pool Iteration for Defragmentation

**Location:** `src/cache/memory/allocation_manager.rs:271-275`

**Action:** Replace stub with actual defragmentation across all pools

**Implementation:**
```rust
PoolCleanupRequest::TriggerDefragmentation { response } => {
    let mut total_reclaimed = 0usize;

    // Access pools from pool_manager
    for pool_idx in 0..self.pool_manager.pool_count() {
        if let Ok(pool) = self.pool_manager.get_pool(pool_idx) {
            // Use PoolCleanupManager for sophisticated defragmentation
            let cleanup_mgr = pool.cleanup_manager();
            match cleanup_mgr.try_sophisticated_cleanup(&pool) {
                Ok(true) => {
                    // Track reclaimed bytes from pool statistics
                    let stats = pool.get_stats();
                    total_reclaimed += stats.reclaimed_bytes as usize;
                }
                Ok(false) => {
                    // No cleanup needed for this pool
                }
                Err(e) => {
                    log::warn!("Pool {} defragmentation failed: {:?}", pool_idx, e);
                }
            }
        }
    }

    let result = Ok(total_reclaimed);
    let _ = response.send(result);
}
```

**Why:** Need to iterate all pools (small <1KB, medium <64KB, large >=64KB) and defragment each.

---

## SUBTASK 2: Verify PoolManager API

**Location:** `src/cache/memory/pool_manager/manager.rs`

**Action:** Verify these methods exist on MemoryPoolManager:
- `pool_count() -> usize` - returns number of pools
- `get_pool(idx: usize) -> Result<&MemoryPool, Error>` - returns pool reference

**If missing:** Check actual API. Alternatives:
- Direct pool access via struct fields
- Iterator method like `pools().iter().enumerate()`

**Update implementation in SUBTASK 1 based on actual API.**

---

## SUBTASK 3: Verify PoolCleanupManager API

**Location:** `src/cache/memory/pool_manager/cleanup_manager.rs`

**Action:** Verify PoolCleanupManager methods:
- `try_sophisticated_cleanup(&self, pool: &MemoryPool) -> Result<bool, CacheOperationError>`

**Check:** Does MemoryPool have `cleanup_manager()` method or is it accessed differently?

**Possible alternatives:**
```rust
// Option A: cleanup_manager is a method
let cleanup_mgr = pool.cleanup_manager();

// Option B: cleanup_manager is stored in PoolCleanupWorker
// Use self's cleanup_manager reference

// Option C: Create new cleanup manager instance
let cleanup_mgr = PoolCleanupManager::new(/* params */);
```

**If PoolCleanupWorker already has cleanup_manager field:** Use `self.cleanup_manager` instead of `pool.cleanup_manager()`.

---

## SUBTASK 4: Verify Statistics Tracking

**Location:** After defragmentation completes

**Action:** Ensure reclaimed bytes are properly tracked

**Check pool statistics API:**
```rust
let stats = pool.get_stats();
// Verify stats has reclaimed_bytes field
```

**If reclaimed_bytes unavailable:**
- Check for alternative stat: `freed_bytes`, `compacted_bytes`, `defrag_bytes`
- Calculate from before/after memory usage: `before_usage - after_usage`

**Update total_reclaimed calculation based on actual stats structure.**

---

## SUBTASK 5: Add Error Logging

**Action:** Add appropriate logging for defragmentation operations

**Implementation:**
```rust
PoolCleanupRequest::TriggerDefragmentation { response } => {
    log::debug!("Starting pool defragmentation across {} pools", self.pool_manager.pool_count());
    let mut total_reclaimed = 0usize;
    let mut pools_cleaned = 0usize;
    let mut pools_failed = 0usize;

    for pool_idx in 0..self.pool_manager.pool_count() {
        if let Ok(pool) = self.pool_manager.get_pool(pool_idx) {
            match cleanup_mgr.try_sophisticated_cleanup(&pool) {
                Ok(true) => {
                    let stats = pool.get_stats();
                    total_reclaimed += stats.reclaimed_bytes as usize;
                    pools_cleaned += 1;
                    log::debug!("Pool {} defragmented: {} bytes reclaimed", pool_idx, stats.reclaimed_bytes);
                }
                Ok(false) => {
                    log::debug!("Pool {} skipped: no cleanup needed", pool_idx);
                }
                Err(e) => {
                    log::warn!("Pool {} defragmentation failed: {:?}", pool_idx, e);
                    pools_failed += 1;
                }
            }
        } else {
            log::warn!("Could not access pool {}", pool_idx);
            pools_failed += 1;
        }
    }

    log::info!(
        "Defragmentation complete: {} bytes reclaimed, {} pools cleaned, {} pools failed",
        total_reclaimed, pools_cleaned, pools_failed
    );

    let result = Ok(total_reclaimed);
    let _ = response.send(result);
}
```

**Why:** Visibility into defragmentation operations for debugging and monitoring.

---

## DEFINITION OF DONE

1. ✅ TriggerDefragmentation implements actual pool defragmentation
2. ✅ All pools are iterated and defragmented
3. ✅ Reclaimed bytes are accurately tracked and returned
4. ✅ Error handling for individual pool failures
5. ✅ Appropriate logging at debug/info/warn levels
6. ✅ Compilation succeeds with `cargo check`
7. ✅ No stub comments remain in implementation

---

## RESEARCH NOTES

### Pool Architecture
- **Small pool:** <1KB allocations
- **Medium pool:** <64KB allocations
- **Large pool:** >=64KB allocations
- Each pool can independently defragment

### PoolCleanupManager
Located: `src/cache/memory/pool_manager/cleanup_manager.rs`

Methods:
- `try_sophisticated_cleanup()` - Uses FragmentationAnalyzer, MaintenanceScheduler
- Integrates with existing allocation statistics
- Returns bool indicating if cleanup was performed

### PoolCleanupWorker Structure
Located: `src/cache/memory/allocation_manager.rs:260-276`

```rust
struct PoolCleanupWorker {
    receiver: Receiver<PoolCleanupRequest>,
    pool_manager: Arc<MemoryPoolManager>,
}
```

Worker has direct access to pool_manager.

### Key Files
- `src/cache/memory/allocation_manager.rs` - Primary implementation location
- `src/cache/memory/pool_manager/manager.rs` - MemoryPoolManager API
- `src/cache/memory/pool_manager/cleanup_manager.rs` - PoolCleanupManager
- `src/cache/memory/pool_manager/individual_pool.rs` - MemoryPool structure

---

**DO NOT write tests or benchmarks - another team handles that.**
