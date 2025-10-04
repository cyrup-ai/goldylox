# Actor Messages in Goldylox

This document catalogs all message types used in the Goldylox actor-based architecture.

## Overview

Goldylox uses message passing for inter-thread communication. Workers own data; coordinators route messages; actors process messages.

---

## Tier Actor Messages

### 1. Hot Tier Messages: `CacheRequest<K, V>`
**File:** `src/cache/tier/hot/thread_local.rs`

**Sender → Hot Tier Actor**

```rust
pub enum CacheRequest<K: CacheKey, V: CacheValue> {
    // Data operations
    Get { key: K, response: Sender<Option<V>> },
    Put { key: K, value: V, response: Sender<Result<(), CacheOperationError>> },
    Remove { key: K, response: Sender<Option<V>> },
    
    // Atomic operations
    PutIfAbsent { key: K, value: V, response: Sender<Option<V>> },
    Replace { key: K, value: V, response: Sender<Option<V>> },
    CompareAndSwap { key: K, expected: V, new_value: V, response: Sender<bool> },
    
    // Maintenance
    CleanupExpired { ttl_ns: u64, response: Sender<usize> },
    ProcessPrefetch { response: Sender<usize> },
    Compact { response: Sender<usize> },
    Clear { response: Sender<()> },
    
    // Statistics
    GetStats { response: Sender<TierStatistics> },
    GetMemoryStats { response: Sender<MemoryPoolStats> },
    GetEvictionStats { response: Sender<EvictionStats> },
    GetPrefetchStats { response: Sender<PrefetchStats> },
    ShouldOptimize { response: Sender<bool> },
    
    // Analytics
    GetFrequentKeys { threshold: u32, window_ns: u64, response: Sender<Vec<K>> },
    GetIdleKeys { threshold_ns: u64, response: Sender<Vec<K>> },
    
    Shutdown,
}
```

**Actor:** Hot tier worker thread (owns `SimdHotTier<K, V>` data)

---

### 2. Warm Tier Messages: `WarmCacheRequest<K, V>`
**File:** `src/cache/tier/warm/global_api.rs`

**Sender → Warm Tier Actor**

```rust
pub enum WarmCacheRequest<K: CacheKey, V: CacheValue> {
    // Data operations
    Get { key: K, response: Sender<Option<V>> },
    Put { key: K, value: V, response: Sender<Result<(), CacheOperationError>> },
    Remove { key: K, response: Sender<Option<V>> },
    
    // Atomic operations
    PutIfAbsent { key: K, value: V, response: Sender<Option<V>> },
    Replace { key: K, value: V, response: Sender<Option<V>> },
    CompareAndSwap { key: K, expected: V, new_value: V, response: Sender<bool> },
    
    // Maintenance
    CleanupExpired { max_age: Duration, response: Sender<Result<usize, CacheOperationError>> },
    ForceEviction { target_count: usize, response: Sender<Result<usize, CacheOperationError>> },
    ProcessMaintenance { response: Sender<Result<usize, CacheOperationError>> },
    
    // Statistics
    GetStats { response: Sender<TierStatsSnapshot> },
    GetMemoryUsage { response: Sender<Option<usize>> },
    GetMemoryPressure { response: Sender<Option<f64>> },
    GetCacheSize { response: Sender<Option<usize>> },
    
    // Analytics
    GetFrequentKeys { limit: usize, response: Sender<Vec<K>> },
    GetIdleKeys { threshold: Duration, response: Sender<Vec<K>> },
    GetAllKeys { response: Sender<Vec<K>> },
    GetAlerts { response: Sender<Vec<CacheAlert>> },
    GetMLPolicies { response: Sender<Vec<PolicyPerformanceMetrics>> },
    
    // ML operations
    UpdateMLModels { response: Sender<Result<usize, CacheOperationError>> },
    
    Shutdown,
}
```

**Actor:** Warm tier worker thread (owns `LockFreeWarmTier<K, V>` data)

---

### 3. Cold Tier Messages: `ColdTierMessage`
**File:** `src/cache/tier/cold/mod.rs`

**Sender → Cold Tier Service Actor**

```rust
pub enum ColdTierMessage {
    Register {
        type_key: (TypeId, TypeId),
        tier: Box<dyn Any + Send + Sync>,
        response: Sender<Result<(), CacheOperationError>>,
    },
    ExecuteOp {
        type_key: (TypeId, TypeId),
        op: MutateOperation,  // Closure that modifies data
        response: Sender<Result<Vec<u8>, CacheOperationError>>,
    },
    ExecuteReadOp {
        type_key: (TypeId, TypeId),
        op: ReadOperation,  // Closure that reads data
        response: Sender<Result<Vec<u8>, CacheOperationError>>,
    },
    Maintenance {
        operation: MaintenanceOperation,
        response: Sender<Result<(), CacheOperationError>>,
    },
}
```

**Actor:** `ColdTierService` (owns all cold tier data for all K,V types)

**Note:** Operations use closures because cold tier is type-erased

---

## Coordination Messages

### 4. Coherence Worker Messages: `CoherenceRequest<K, V>` / `CoherenceResponse<K, V>`
**File:** `src/cache/coherence/worker/message_types.rs`

**Sender → Coherence Actor → Sender**

```rust
pub enum CoherenceRequest<K, V> {
    Read {
        key: K,
        requesting_tier: CacheTier,
        request_id: u64,
    },
    Write {
        key: K,
        data: V,
        requesting_tier: CacheTier,
        request_id: u64,
    },
    GetStatistics { request_id: u64 },
    Serialize {
        key: K,
        value: V,
        target_tier: CacheTier,
        request_id: u64,
    },
    RecordRead { key: K, tier: CacheTier, request_id: u64 },
    RecordWrite { key: K, data: V, tier: CacheTier, request_id: u64 },
    RecordPrefetch { key: K, tier: CacheTier, request_id: u64 },
}

pub enum CoherenceResponse<K, V> {
    ReadSuccess { request_id: u64, response: ReadResponse },
    WriteSuccess { request_id: u64, response: WriteResponse },
    Statistics { request_id: u64, statistics: CoherenceStatisticsSnapshot },
    SerializeSuccess { request_id: u64, envelope: Box<SerializationEnvelope<K, V>> },
    AccessRecorded { request_id: u64 },
    Error { request_id: u64, error: CoherenceError },
}
```

**Actor:** `CoherenceWorker` (owns `CoherenceController<K, V>`)

---

### 5. Prefetch Predictor Messages: `PrefetchRequest<K>`
**File:** `src/cache/coordinator/unified_manager.rs`

**Sender → Prefetch Actor**

```rust
pub enum PrefetchRequest<K: CacheKey> {
    RecordAccess {
        key: K,
        access_time_ns: u64,
        context_hash: u64,
    },
    ProcessPrefetches {
        max_count: usize,
        response: Sender<Vec<PrefetchRequest<K>>>,
    },
    ShouldPrefetch {
        key: K,
        response: Sender<bool>,
    },
    GetQueueStatus {
        response: Sender<(usize, usize)>,
    },
    CleanupExpired {
        current_time_ns: u64,
        response: Sender<usize>,
    },
    UpdateConfig { config: PrefetchConfig },
    ClearState,
    GetStats { response: Sender<PrefetchStats> },
}
```

**Actor:** `PrefetchWorker` (owns `PrefetchPredictor<K>`)

---

## Task Coordination Messages

### 6. Cache Commands: `CacheCommand<K, V>`
**File:** `src/cache/worker/task_coordination.rs`

**Sender → Task Executor**

```rust
pub enum CacheCommand<K: CacheKey, V: CacheValue> {
    Insert { key: K, value: V, tier: CacheTier, timestamp: Instant },
    Remove { key: K, tier: CacheTier, timestamp: Instant },
    Prefetch { key: K, confidence: f64, timestamp: Instant },
    Move { key: K, from_tier: CacheTier, to_tier: CacheTier, timestamp: Instant },
    UpdateMetadata { key: K, tier: CacheTier, access_count: u64, last_access: Instant, timestamp: Instant },
    FlushDirty { keys: Vec<K>, tier: CacheTier, timestamp: Instant },
    Compact { tier: CacheTier, target_size: usize, timestamp: Instant },
}
```

**Actor:** Task execution worker

---

### 7. Maintenance Tasks: `MaintenanceTask`
**File:** `src/cache/tier/warm/maintenance.rs`

**Sender → Maintenance Worker**

```rust
pub enum MaintenanceTask {
    CleanupExpired { ttl: Duration, batch_size: usize },
    PerformEviction { target_pressure: f64, max_evictions: usize },
    Evict { target_count: usize },
    UpdateStatistics,
    OptimizeStructure,
    CompactStorage { target_ratio: f32 },
    AnalyzePatterns { analysis_depth: AnalysisDepth, prediction_horizon_sec: u64 },
    SyncTiers { source: CacheTier, target: CacheTier },
    ValidateIntegrity,
    DefragmentMemory { target_fragmentation: f32 },
    UpdateMLModels { training_data_size: usize, model_complexity: ModelComplexity },
}
```

**Actor:** Maintenance coordinator workers

---

### 8. Background Tasks: `BackgroundTask`
**File:** `src/cache/manager/background/types.rs`

**Sender → Background Worker Pool**

```rust
pub enum BackgroundTask {
    Eviction { tier: u8, count: u32, priority: u16 },
    Compression { algorithm: u8, ratio: f32 },
    Statistics { stats_type: u8, interval_ms: u64 },
    Maintenance(MaintenanceTask),
    Prefetch { count: u32, strategy: u8 },
}
```

**Actor:** Background worker threads (managed by `MaintenanceScheduler`)

---

## Memory Management Messages

### 9. Pool Cleanup Messages: `PoolCleanupRequest`
**File:** `src/cache/memory/pool_manager/cleanup_manager.rs`

**Sender → Pool Cleanup Actor**

```rust
pub enum PoolCleanupRequest {
    EmergencyCleanup { min_bytes_to_free: usize, response: Sender<usize> },
    ScheduledCleanup { target_fragmentation: f32, response: Sender<bool> },
    Defragment { pool_id: usize, response: Sender<Result<(), String>> },
    GetMetrics { response: Sender<PoolMetrics> },
}
```

**Actor:** `PoolCleanupWorker` (owns cleanup state)

---

## Data Flow Pattern

```
User API Call
    ↓
UnifiedCacheManager (owns tier coordinators)
    ↓
TierOperations uses coordinator to lookup tier handle
    ↓
Sends CacheRequest<K,V> to tier actor
    ↓
Tier Actor (owns data) processes message
    ↓
Sends response back via response channel
    ↓
Returns to caller
```

---

## Key Observations

### ✅ Proper Message Passing
- All tier data operations use request/response pattern
- Actors own their data
- Communication via channels only
- Response channels for results

### ❌ The Coordinator Problem
**Currently:** Multiple "coordinators" (Hot, Warm, Cold) being cloned/Arc-wrapped and passed to workers

**Should be:** 
1. ONE `TierCoordinator` actor that owns all tier registries
2. Workers send routing requests to coordinator
3. Coordinator responds with appropriate tier handle
4. Worker uses handle to send actual data request

### The Arc Issue Root Cause
Workers in `WorkerContext` hold cloned coordinators instead of:
- Holding a `Sender<CoordinatorRequest>` to ask coordinator for routing
- OR getting specific tier handles upfront before spawning

**Example fix:**
```rust
// Instead of:
struct WorkerContext {
    hot_tier_coordinator: HotTierCoordinator,  // ❌ Clone/Arc needed
}

// Should be:
struct WorkerContext {
    coordinator_channel: Sender<CoordinatorRequest>,  // ✅ Just a Sender
}

// Or extract handles before spawning:
let hot_handle = coordinator.get_or_create_tier::<K,V>()?;
thread::spawn(move || {
    hot_handle.send_request(...)  // ✅ Just use the handle
});
```

---

## Message Types That Move Data

**CRITICAL:** Looking at all message types, the ones that **move user data (K, V)**:

1. `CacheRequest::Put { value: V }` - User data moves INTO tier actor (owned there)
2. `WarmCacheRequest::Put { value: V }` - User data moves INTO tier actor (owned there)
3. `CoherenceRequest::Write { data: V }` - User data moved for coherence tracking
4. `CacheCommand::Insert { value: V }` - Task carries user data to executor

**None of these are problematic** - they're moving data TO the owner, not cloning it everywhere.

The issue is **coordinators** (routing tables) being cloned, not user data.
