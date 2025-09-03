# Task 1: Fix RecoveryStrategy Index Delegation

## Description
Fix broken code that passes `usize strategy_idx` where `RecoveryStrategy` enum is expected, causing compilation errors due to type mismatch.

## Objective
Correct all instances of incorrect index-based strategy usage to use proper `RecoveryStrategy` enum values directly.

## Dependencies
None - This is an independent broken code fix

## Success Criteria
1. All `usize strategy_idx` parameters replaced with proper `RecoveryStrategy` enum usage
2. Type mismatch compilation errors resolved
3. Recovery strategy functionality preserved and enhanced through proper enum usage
4. No broken index-to-enum conversion attempts remaining
5. Code uses canonical `RecoveryStrategy` enum variants throughout

## Implementation Steps
1. Locate broken code at `src/cache/manager/error_recovery/core.rs:62,75`
2. Identify all instances where `usize strategy_idx` is passed to methods expecting `RecoveryStrategy`
3. Review canonical enum implementation at `src/cache/manager/error_recovery/types.rs:26`
4. Replace index-based usage with proper enum variants:
   - Identify what each index was supposed to represent
   - Map to appropriate enum variant (Retry, Fallback, GracefulDegradation, etc.)
5. Remove any index-to-enum conversion helper functions
6. Update method signatures to accept `RecoveryStrategy` directly
7. Validate that all recovery strategies work correctly with enum-based approach

## Deliverables
- Fixed method calls using proper `RecoveryStrategy` enum variants
- Removal of incorrect index-based strategy selection
- Updated method signatures for type consistency
- Verification that all recovery strategy types are accessible and functional

## Notes
- The canonical enum implementation already exists and is properly structured
- This fix enhances type safety by using enums instead of magic numbers
- Focus on finding the intent behind each index value to map to correct enum variant
- This should improve code maintainability in addition to fixing compilation errors