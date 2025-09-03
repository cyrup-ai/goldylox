# Task 5: Remove Eviction Factory Function Bloat

## Description
Remove unnecessary factory wrapper functions that provide no value over direct constructor calls, cleaning up the API surface and reducing code duplication.

## Objective
Remove bloated factory functions while ensuring all functionality remains available through direct constructor calls.

## Dependencies
None - This is an independent broken code fix

## Success Criteria
1. All unnecessary factory functions removed from `src/cache/eviction/mod.rs:43-70`
2. Functionality preserved through direct constructor calls
3. No callsites broken (functions are marked as unused, so no callsites should exist)
4. Cleaner API surface without unnecessary indirection
5. Documentation updated to reflect direct constructor usage

## Implementation Steps
1. Locate factory functions in `src/cache/eviction/mod.rs:43-70`:
   - `create_policy_engine`
   - `create_ml_policy`
   - `create_write_policy_manager`
   - `create_prefetch_predictor`
   - `create_eviction_system`
2. Verify these are truly unused (should be marked as dead code)
3. Confirm that direct constructors are already used in the codebase:
   - `CachePolicyEngine::new(&config, PolicyType::default())` at `unified_manager.rs:94`
   - Similar patterns for other components
4. Remove all factory wrapper functions
5. Update any module documentation that may reference the factory functions
6. Validate that direct constructor calls provide equivalent functionality

## Deliverables
- Removal of 5 factory wrapper functions from eviction module
- Preserved functionality through existing direct constructor usage
- Updated documentation reflecting direct constructor patterns
- Cleaner module API surface

## Notes
- These functions just add unnecessary indirection without providing value
- Direct constructor calls are already the pattern used in the codebase
- This cleanup should improve code clarity and reduce maintenance burden
- Factory functions were likely created early in development but became obsolete