# STUB_1: Implement Cold Tier Compaction System

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
Replace 3 critical "for now" stubs in the cold tier compaction system with production-quality implementations. Currently, these functions claim to complete work but do nothing, leading to memory leaks, wasted disk space, and unreliable storage.

---

## CURRENT PROBLEM

**File:** `src/cache/tier/cold/compaction_system.rs`

Three functions are completely stubbed out:
1. **Line 126:** `compact_data_file()` - Sets progress to 100% but reclaims 0 bytes (no actual compaction)
2. **Line 160:** `cleanup_expired_entries()` - Returns immediately without removing expired entries
3. **Line 168:** `optimize_compression_parameters()` - Uses default config instead of optimization

**Impact:** Cold tier storage grows unbounded, expired data never removed, compression never optimized.

---

## SUBTASK 1: Implement compact_data_file()

**Location:** `src/cache/tier/cold/compaction_system.rs:123-129`

**Current Code:**
```rust
fn compact_data_file(state: &CompactionState) {
    state.progress.store(0.1);
    
    for now  // ❌ REMOVE THIS STUB
    state.bytes_reclaimed.store(0, Ordering::Relaxed);
    state.progress.store(1.0);
}
```

**What Needs to Change:**
1. Remove the "for now" stub comment
2. Implement actual data file compaction algorithm:
   - Read all valid (non-expired, non-deleted) entries from cold tier storage
   - Write them to a new temporary file in compacted format
   - Calculate bytes reclaimed (old_size - new_size)
   - Atomically replace old data file with compacted version
   - Update `state.bytes_reclaimed` with actual reclaimed bytes
   - Update progress incrementally during the process

**Why This Matters:**
Without compaction, cold tier data files grow indefinitely with dead/deleted entries, wasting disk space and slowing down reads.

**Implementation Requirements:**
- Use existing cold tier storage APIs for reading/writing entries
- Maintain data integrity during compaction (atomic file replacement)
- Handle errors gracefully (rollback on failure)
- Update progress at meaningful intervals (0.1, 0.3, 0.5, 0.7, 0.9, 1.0)
- Calculate and store actual bytes reclaimed

---

## SUBTASK 2: Implement cleanup_expired_entries()

**Location:** `src/cache/tier/cold/compaction_system.rs:157-162`

**Current Code:**
```rust
pub fn cleanup_expired_entries(state: &CompactionState) {
    state.progress.store(0.2);
    
    for now  // ❌ REMOVE THIS STUB
    state.progress.store(1.0);
}
```

**What Needs to Change:**
1. Remove the "for now" stub comment
2. Implement expired entry cleanup:
   - Iterate through all cold tier entries
   - Check each entry's TTL/expiration timestamp
   - Remove entries that have expired
   - Update metadata index to reflect removals
   - Update progress during iteration
   - Log number of entries removed

**Why This Matters:**
Expired entries consume storage space and pollute the cold tier. Without cleanup, the cache grows unbounded with stale data.

**Implementation Requirements:**
- Use current timestamp to determine expiration
- Remove expired entries from both data files and metadata index
- Maintain consistency between data and metadata
- Update `state.progress` during cleanup (0.2 → 1.0)
- Handle concurrent access safely
- Return or log count of removed entries

---

## SUBTASK 3: Implement optimize_compression_parameters()

**Location:** `src/cache/tier/cold/compaction_system.rs:165-179`

**Current Code:**
```rust
fn optimize_compression_parameters(state: &CompactionState) {
    state.progress.store(0.4);
    
    for now  // ❌ REMOVE THIS STUB
    let config = CacheConfig::default();
    match MemoryEfficiencyAnalyzer::new(&config) {
        Ok(analyzer) => {
            // ... existing analyzer code ...
```

**What Needs to Change:**
1. Remove the "for now" stub comment
2. Instead of using `CacheConfig::default()`, analyze actual cold tier data characteristics:
   - Sample existing cold tier entries to determine data patterns
   - Calculate compression ratios for different compression levels
   - Adjust compression parameters based on:
     - Data compressibility (entropy analysis)
     - CPU vs storage tradeoff
     - Access patterns (read-heavy vs write-heavy)
   - Apply optimized compression settings to future compactions

**Why This Matters:**
Using default compression parameters wastes either CPU (over-compression) or storage (under-compression). Dynamic optimization adapts to actual data characteristics.

**Implementation Requirements:**
- Sample representative cold tier entries (e.g., 100-1000 entries)
- Test multiple compression levels on samples
- Calculate compression ratio, CPU cost, and storage savings
- Select optimal compression level based on metrics
- Apply optimized settings to compaction state
- Update progress (0.4 → continuation of compaction)

---

## DEFINITION OF DONE

**Verification Steps:**
1. Remove all "for now" stub comments from `compaction_system.rs`
2. `compact_data_file()` actually reclaims storage space (bytes_reclaimed > 0)
3. `cleanup_expired_entries()` removes expired entries from cold tier
4. `optimize_compression_parameters()` analyzes data and sets compression levels
5. Code compiles: `cargo check`
6. No panics or errors when compaction tasks are triggered

**Success Criteria:**
- All 3 functions perform actual work
- Progress tracking reflects real work progress
- Bytes reclaimed accurately reported
- Expired entries actually removed
- Compression parameters based on data analysis (not defaults)
- No data loss during compaction
- Atomic operations maintain consistency

---

## RESEARCH NOTES

**Cold Tier Architecture:**
- Storage backend: File-based with metadata index
- Entry format: Bincode serialization with optional compression
- Metadata: Separate index tracking entry locations and metadata
- Compaction state: Shared atomic state for progress tracking

**Relevant Files:**
- `src/cache/tier/cold/core/storage.rs` - Storage operations
- `src/cache/tier/cold/metadata_index.rs` - Metadata management
- `src/cache/tier/cold/compression.rs` - Compression utilities
- `src/cache/tier/cold/serialization/` - Entry serialization

**Key Data Structures:**
- `CompactionState` - Atomic progress and metrics tracking
- `ColdTierStorage` - Storage backend interface
- `MetadataIndex` - Entry metadata management

---

## IMPLEMENTATION CONSTRAINTS

**DO NOT:**
- ❌ Write unit tests (separate team handles testing)
- ❌ Write integration tests (separate team handles testing)
- ❌ Write benchmarks (separate team handles benchmarks)
- ❌ Add new dependencies without justification
- ❌ Change public APIs unnecessarily

**DO:**
- ✅ Focus solely on removing stubs with production code
- ✅ Use existing cold tier APIs and data structures
- ✅ Maintain backward compatibility
- ✅ Handle errors gracefully
- ✅ Document non-obvious implementation choices with inline comments
- ✅ Follow existing code style and patterns in the file

---

## NOTES

**Why These 3 Together:**
All three stubs are in the same file (`compaction_system.rs`) and are tightly related to the compaction workflow. Implementing them together ensures a cohesive, working compaction system.

**Estimated Complexity:** Medium-High
- Requires understanding cold tier storage internals
- Need to handle concurrent access safely
- Must maintain data integrity during compaction
- Progress tracking should reflect actual work

**Session Focus:** Single file, three related functions, unified compaction workflow.
