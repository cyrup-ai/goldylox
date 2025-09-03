# Task 2: Fix AlertThresholds Type Confusion

## Description
Fix type confusion where code uses `telemetry::types::AlertThresholds` instead of the correct `cache::manager::performance::types::AlertThresholds` type.

## Objective
Resolve type mismatch compilation errors by using the correct AlertThresholds type consistently throughout the codebase.

## Dependencies
None - This is an independent broken code fix

## Success Criteria
1. All incorrect `telemetry::types::AlertThresholds` usage replaced with correct type
2. Consistent use of `cache::manager::performance::types::AlertThresholds` throughout
3. Type mismatch compilation errors resolved
4. Alert threshold functionality preserved with proper atomic field access
5. No remaining type confusion or incorrect imports

## Implementation Steps
1. Locate broken usage at `src/cache/manager/performance/alert_system.rs:92`
2. Identify all instances of incorrect `telemetry::types::AlertThresholds` usage
3. Review canonical type implementation at `src/cache/manager/performance/types.rs:104`
4. Replace incorrect type references with proper imports:
   - Update import statements
   - Fix type annotations and method signatures
   - Ensure proper field access (AtomicU64 fields)
5. Search for any other locations with the same type confusion
6. Validate that alert threshold functionality works with correct type
7. Remove any incorrect type definitions or duplicates

## Deliverables
- Fixed import statements using correct AlertThresholds type
- Updated type annotations and method signatures
- Removal of incorrect type references
- Verification that alert system functionality is preserved

## Notes
- The correct implementation uses AtomicU64 fields for thread-safe threshold management
- This fix ensures proper atomic operations for performance monitoring
- Look for any helper functions or utilities that may also need type updates
- Verify that no functionality is lost in the type correction process