# Task: Review Statistics Infrastructure

## Description
Examine the existing statistics infrastructure in `src/telemetry/unified_stats.rs` and related modules to understand the 608 lines of production-ready atomic statistics system before implementing integration.

## Objective
Gain comprehensive understanding of UnifiedCacheStatistics, explicit hit/miss tracking, and tier coordination to enable proper integration in warm tier monitoring.

## Success Criteria
- [ ] Understand UnifiedCacheStatistics::get_global_instance() pattern
- [ ] Understand explicit hit/miss counting (not calculated rates)
- [ ] Understand get_performance_metrics() comprehensive data
- [ ] Understand atomic coordination across tiers
- [ ] Understand TierStatistics to TierStatsSnapshot conversion
- [ ] Document key integration points for warm tier usage

## Files to Review
- `src/telemetry/unified_stats.rs` (all 608 lines)
- `src/cache/types/statistics/tier_stats.rs`
- Current violation in `src/cache/tier/warm/monitoring/types.rs:162-163`

## Dependencies
- None (this is the first task in the milestone)

## Key Integration Points to Identify
1. How UnifiedCacheStatistics provides explicit hit/miss counts
2. How get_performance_metrics() gives comprehensive tier breakdown
3. How atomic coordination maintains consistency across tiers
4. How existing warm/cold tier APIs provide real statistics
5. How to connect TierStatistics with explicit tracking

## Current Violation Analysis
The violation uses hardcoded zeros:
```rust
total_hits: 0,    // WRONG: hardcoded
total_misses: 0,  // WRONG: hardcoded
```

Should use explicit tracking:
```rust
total_hits: stats.hits,      // CORRECT: explicit tracking
total_misses: stats.misses,  // CORRECT: explicit tracking
```

## Next Task
After completing this review, proceed to `1_connect_hit_miss_tracking.md`