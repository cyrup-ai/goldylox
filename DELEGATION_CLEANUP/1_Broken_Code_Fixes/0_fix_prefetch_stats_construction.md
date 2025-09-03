# Task 0: Fix PrefetchStats Incomplete Construction

## Description
Fix the broken `CachePolicyEngine::get_prefetch_stats()` method that has incomplete struct construction missing required fields.

## Objective
Remove the broken duplicate method while ensuring all prefetch statistics functionality remains available through the working canonical implementation.

## Dependencies
None - This is an independent broken code fix

## Success Criteria
1. Broken `get_prefetch_stats()` method removed from `CachePolicyEngine`
2. Compilation error for missing fields resolved
3. All prefetch statistics functionality preserved via working canonical implementation
4. No callsites broken (method is marked as dead code, so no callsites should exist)
5. Functionality validation through working `PrefetchPredictor::get_stats()` method

## Implementation Steps
1. Locate broken method in `src/cache/eviction/policy_engine.rs:290`
2. Verify the method is truly dead code (no callsites exist)
3. Identify canonical implementation at `src/cache/tier/hot/prefetch/core.rs:322`
4. Verify canonical implementation has all required fields correctly populated
5. Remove broken duplicate method completely
6. Run `cargo check` to verify compilation error is resolved
7. Test that prefetch statistics are still accessible through canonical method

## Deliverables
- Removal of broken `CachePolicyEngine::get_prefetch_stats()` method
- Verification that `PrefetchPredictor::get_stats()` provides complete functionality
- Documentation of the fix in commit message
- Confirmation that compilation error is resolved

## Notes
- This is a true broken duplicate removal - no functionality should be lost
- The canonical implementation already provides complete prefetch statistics
- Method is marked as dead code, so removal should not affect any existing functionality
- This fix should reduce compilation errors without affecting working code paths