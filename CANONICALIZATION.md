# Comprehensive Type Duplication Canonicalization Table

| Type Name             | Best of Best? | Canonical Base Location                                                   | Call Site Count |
|-----------------------|---------------|---------------------------------------------------------------------------|-----------------|
| AccessPattern         | Yes           | /Volumes/samsung_t9/goldylox/src/cache/analyzer/types.rs:53               | 107             |
| AccessTracker         | Yes           | /Volumes/samsung_t9/goldylox/src/cache/traits/policy.rs:45                | 28              |
| AlertPatternState     | No            | /Volumes/samsung_t9/goldylox/src/telemetry/data_structures.rs:453         | 2               |
| AlertThresholds       | No            | /Volumes/samsung_t9/goldylox/src/telemetry/types.rs:43                    | 8               |
| AlertType             | No            | /Volumes/samsung_t9/goldylox/src/telemetry/types.rs:30                    | 3               |
| AlgorithmMetrics      | No            | /Volumes/samsung_t9/goldylox/src/cache/tier/cold/data_structures.rs:302   | 18              |
| AnalyzerConfig        | No            | /Volumes/samsung_t9/goldylox/src/cache/config/types.rs:131                | 18              |
| BatchOperation        | No            | /Volumes/samsung_t9/goldylox/src/cache/types/core_types.rs:61             | 16              |
| CacheEntry            | Yes           | /Volumes/samsung_t9/goldylox/src/cache/traits/cache_entry.rs:669          | 66              |
| CacheEntryMetadata    | No            | /Volumes/samsung_t9/goldylox/src/cache/traits/cache_entry.rs:112          | 9               |
| CacheError            | No            | /Volumes/samsung_t9/goldylox/src/cache/traits/error.rs:11                 | 36              |
| CacheStrategySelector | No            | /Volumes/samsung_t9/goldylox/src/cache/manager/strategy/core.rs:29        | 11              |
| CompressionAlgorithm  | No            | /Volumes/samsung_t9/goldylox/src/cache/traits/supporting_types.rs:100     | 123             |
| ConsistencyLevel      | No            | /Volumes/samsung_t9/goldylox/src/cache/tier/warm/maintenance.rs:97        | 17              |
| ErrorType             | No            | /Volumes/samsung_t9/goldylox/src/cache/manager/error_recovery/types.rs:13 | 82              |
| EvictionCandidate     | Yes           | /Volumes/samsung_t9/goldylox/src/cache/types/eviction/candidate.rs:15     | 43              |
| EvictionPolicy        | Yes           | /Volumes/samsung_t9/goldylox/src/cache/traits/policy.rs:15                | 130             |
| FrequencyTrend        | No            | /Volumes/samsung_t9/goldylox/src/cache/tier/warm/eviction/types.rs:90     | 9               |
| HashContext           | No            | /Volumes/samsung_t9/goldylox/src/cache/traits/supporting_types.rs:12      | 42              |
| MaintenanceTask       | Yes           | /Volumes/samsung_t9/goldylox/src/cache/tier/warm/maintenance.rs:14        | 164             |
| PerformanceAlert      | No            | /Volumes/samsung_t9/goldylox/src/cache/manager/performance/types.rs:67    | 25              |
| PerformanceMetrics    | No            | /Volumes/samsung_t9/goldylox/src/cache/traits/types_and_enums.rs:578      | 59              |
| PerformanceMonitor    | No            | /Volumes/samsung_t9/goldylox/src/cache/manager/performance/core.rs:12     | 14              |
| Priority              | No            | /Volumes/samsung_t9/goldylox/src/cache/traits/supporting_types.rs:24      | 146             |
| RecoveryHint          | No            | /Volumes/samsung_t9/goldylox/src/cache/traits/types_and_enums.rs:226      | 25              |
| SerializationContext  | Yes           | /Volumes/samsung_t9/goldylox/src/cache/traits/cache_entry.rs:609          | 24              |
| SizeEstimator         | No            | /Volumes/samsung_t9/goldylox/src/cache/traits/supporting_types.rs:36      | 32              |
| TaskPriority          | No            | /Volumes/samsung_t9/goldylox/src/cache/tier/warm/maintenance.rs:237       | 22              |

## Summary
- **Total Types to Canonicalize**: 27 duplicate types
- **"Best of Best" Merges Required**: 7 types (AccessPattern, AccessTracker, CacheEntry, EvictionCandidate, EvictionPolicy, MaintenanceTask, SerializationContext)
- **Clear Winner Consolidations**: 20 types
- **Total Call Sites to Update**: 1,577 import/usage locations

## High-Impact Canonicalizations (Priority Order)
1. **Priority** (146 sites) - trait definitions need consolidation
2. **MaintenanceTask** (164 sites) - warm tier enum version is most comprehensive
3. **EvictionPolicy** (130 sites) - GAT-based trait with HRTBs is canonical
4. **CompressionAlgorithm** (123 sites) - enum unification needed
5. **AccessPattern** (107 sites) - analyzer version has most features
6. **ErrorType** (82 sites) - error recovery types version is canonical
7. **CacheEntry** (66 sites) - struct implementation is primary concrete type

## Canonicalization Strategy
- **No Type Erasure**: All generics preserved, zero runtime overhead
- **No Aliases/Shims/Re-exports**: Direct consolidation only
- **Feature Merging**: "Best of Best" types combine all features from multiple implementations
- **Clear Winners**: Single superior implementation becomes canonical

## Implementation Phases
- **Phase 1**: High-impact types (100+ call sites)
- **Phase 2**: Medium-impact types (25-100 call sites)  
- **Phase 3**: Remaining types (2-24 call sites)

**Highlander Rules**: There can be only one. All duplicates will be eliminated with full feature preservation.