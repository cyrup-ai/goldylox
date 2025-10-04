# CLEANUP_2: Remove Dead Legacy Code Modules

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

Remove or document unused legacy code modules that are marked with `#[allow(dead_code)]` and have no instantiation in the codebase. These modules contribute to code bloat and maintenance burden without providing functionality.

**CRITICAL:** Do NOT write unit tests or benchmarks. Another team handles testing.

---

## CONTEXT

**Dead Module 1 - TierManager:**
- File: `src/cache/tier/manager.rs`
- Entire module marked `#![allow(dead_code)]`
- TierManager::new() has zero matches in codebase
- All methods return stub values (None, Ok(()), false)

**Dead Module 2 - check_tier_transitions:**
- File: `src/cache/worker/tier_transitions.rs:15-29`
- Function marked `#[allow(dead_code)]`
- Body is just comment "Legacy code - not implemented"

**Impact:** None if truly unused; code bloat and confusion if retained without purpose.

---

## SUBTASK 1: Verify TierManager is Truly Unused

**Location:** `src/cache/tier/manager.rs`

**Action:** Confirm no references to TierManager exist

**Searches to perform:**
```bash
# Search for TierManager references
grep -r "TierManager" src/

# Search for module imports
grep -r "tier::manager" src/

# Search for use statements
grep -r "use.*TierManager" src/
```

**Expected result:** Only self-references within manager.rs file.

**If found external references:** Do NOT remove. Document why it's retained (future use, API stability, etc.).

---

## SUBTASK 2: Distinguish TierManager from TierPromotionManager

**Action:** Verify TierPromotionManager is NOT affected

**Key distinction:**
- **TierManager:** In manager.rs, marked dead_code, NOT used
- **TierPromotionManager:** In same file, actively used for promotion logic

**Verify TierPromotionManager usage:**
```bash
grep -r "TierPromotionManager" src/
```

**Expected:** Multiple references showing active use.

**Important:** Only remove TierManager struct and its methods, NOT TierPromotionManager.

---

## SUBTASK 3: Remove TierManager Struct and Methods

**Location:** `src/cache/tier/manager.rs`

**Action:** Delete TierManager struct and all associated methods

**Code to remove (approximate lines 14-298):**
```rust
// DELETE entire TierManager struct:
pub struct TierManager<K: CacheKey> {
    // ... fields
}

// DELETE all impl blocks for TierManager:
impl<K: CacheKey> TierManager<K> {
    // ... methods including:
    // - read_from_tier()
    // - write_to_tier()
    // - remove_from_tier()
}
```

**Keep:**
- Module-level documentation
- TierPromotionManager struct and implementations
- All other code in the file

---

## SUBTASK 4: Update Module Documentation

**Location:** `src/cache/tier/manager.rs` - top of file

**Action:** Update documentation to remove TierManager references

**Current (likely):**
```rust
//! Main tier promotion manager with SIMD-optimized decision algorithms
//!
//! This module provides the core TierPromotionManager implementation...
```

**Ensure it doesn't mention TierManager if that struct is removed.**

**Add note if needed:**
```rust
//! Main tier promotion manager with SIMD-optimized decision algorithms
//!
//! This module provides the core TierPromotionManager implementation with
//! intelligent promotion/demotion decisions and adaptive learning capabilities.
//!
//! Note: Legacy TierManager removed in favor of unified TierOperations architecture.
```

---

## SUBTASK 5: Remove check_tier_transitions Function

**Location:** `src/cache/worker/tier_transitions.rs:15-29`

**Action:** Delete the dead function

**Code to remove:**
```rust
// DELETE:
#[allow(dead_code)] // Legacy worker system - not actively used
pub fn check_tier_transitions<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    _stat_sender: &Sender<StatUpdate>,
) {
    // Legacy code - not implemented
}
```

**If this is the ONLY content in tier_transitions.rs:**
- Delete entire file: `rm src/cache/worker/tier_transitions.rs`
- Remove module declaration in `src/cache/worker/mod.rs`

---

## SUBTASK 6: Update Worker Module Declarations

**Location:** `src/cache/worker/mod.rs`

**Action:** Remove tier_transitions module if file was deleted

**If tier_transitions.rs deleted, remove:**
```rust
// DELETE if file removed:
pub mod tier_transitions;
```

**Or if other code remains in tier_transitions.rs:**
- Keep module declaration
- Just the dead function was removed

---

## SUBTASK 7: Verify Compilation After Removal

**Action:** Ensure no broken imports or references

**Commands:**
```bash
cargo check
cargo build
```

**If compilation fails:**
- Review error messages
- There may be references we missed
- Either fix references or restore deleted code and document retention reason

---

## SUBTASK 8: Document Removal Decision

**Location:** Create or update ARCHITECTURE.md or CHANGELOG.md

**Action:** Document what was removed and why

**Add entry:**
```markdown
## Removed Dead Code (2025-10-03)

### TierManager Struct
- **Removed from:** `src/cache/tier/manager.rs`
- **Reason:** Entire struct marked #[allow(dead_code)], no instantiation found
- **Methods removed:** read_from_tier(), write_to_tier(), remove_from_tier()
- **Note:** All methods were stubs returning None/Ok(())/false
- **Replacement:** Unified TierOperations architecture

### check_tier_transitions Function
- **Removed from:** `src/cache/worker/tier_transitions.rs`
- **Reason:** Marked dead_code, body was empty stub
- **Note:** Legacy worker system, not actively used
```

---

## ALTERNATIVE: DOCUMENT RETENTION IF KEEPING

**If decision is to KEEP legacy code for any reason:**

**Action:** Add clear documentation explaining retention

**Example retention documentation:**
```rust
//! # Legacy TierManager
//!
//! **Status:** DEPRECATED - Retained for API compatibility
//!
//! This struct is no longer used internally. The modern cache uses
//! TierOperations for all tier coordination. TierManager is kept to
//! avoid breaking external consumers who may depend on this API.
//!
//! **Migration guide:** Use `TierOperations` instead.
//!
//! **Removal planned:** Version 2.0
```

---

## DEFINITION OF DONE

1. ✅ TierManager usage verified (should be zero)
2. ✅ TierPromotionManager confirmed NOT affected
3. ✅ One of the following:
   - TierManager struct and methods removed, OR
   - Retention documented with clear reasoning
4. ✅ check_tier_transitions function removed
5. ✅ tier_transitions.rs deleted if empty (or module removed from mod.rs)
6. ✅ Module documentation updated
7. ✅ Compilation succeeds with `cargo check`
8. ✅ Removal documented in CHANGELOG or ARCHITECTURE.md

---

## RESEARCH NOTES

### Legacy Code Indicators
- `#[allow(dead_code)]` - Compiler knows it's unused
- "Legacy code - not implemented" comments
- Methods returning stub values (None, Ok(()), false)
- Zero instantiation (Type::new() has no matches)

### TierPromotionManager vs TierManager
Both are in `src/cache/tier/manager.rs`:
- **TierPromotionManager:** Active, used for intelligent tier transitions
- **TierManager:** Legacy, dead code with stub methods

### Worker System
Located: `src/cache/worker/`
- Modern system uses BackgroundCoordinator, TaskCoordinator
- Legacy tier_transitions worker not needed

### Removal Safety
Safe to remove when:
- No external references (verified by grep)
- No public API usage (internal crate only)
- Marked as dead_code by developers
- All methods are stubs (no actual functionality lost)

### Key Files
- `src/cache/tier/manager.rs` - TierManager removal
- `src/cache/worker/tier_transitions.rs` - Function removal
- `src/cache/worker/mod.rs` - Module declaration cleanup
- `ARCHITECTURE.md` or `CHANGELOG.md` - Documentation

---

**DO NOT write tests or benchmarks - another team handles that.**
