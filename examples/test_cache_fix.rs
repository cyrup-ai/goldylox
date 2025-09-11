use goldylox::prelude::*;
use goldylox::cache::traits::supporting_types::HashContext;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
struct TestData {
    value: u32,
}

impl CacheKey for TestData {
    type HashContext = StandardHashContext;
    type Priority = StandardPriority;
    type SizeEstimator = StandardSizeEstimator;

    fn estimated_size(&self) -> usize {
        std::mem::size_of::<TestData>()
    }

    fn hash_context(&self) -> Self::HashContext {
        StandardHashContext::ahash_default()
    }

    fn priority(&self) -> Self::Priority {
        StandardPriority::new(50)
    }

    fn size_estimator(&self) -> Self::SizeEstimator {
        StandardSizeEstimator::new()
    }

    fn fast_hash(&self, context: &Self::HashContext) -> u64 {
        context.compute_hash(self)
    }
}

impl CacheValue for TestData {
    type Metadata = CacheValueMetadata;

    fn estimated_size(&self) -> usize {
        std::mem::size_of::<TestData>()
    }

    fn is_expensive(&self) -> bool {
        false
    }

    fn compression_hint(&self) -> CompressionHint {
        CompressionHint::Disable
    }

    fn metadata(&self) -> Self::Metadata {
        CacheValueMetadata::from_cache_value(self)
    }
}

fn main() {
    println!("🧪 Testing cache PUT/GET after array bounds fix...");
    
    // Create a simple cache with small capacity to trigger eviction quickly
    let cache = GoldyloxBuilder::new()
        .hot_tier_max_entries(64)  // Small to trigger eviction
        .build()
        .expect("Failed to create cache");

    // Test 1: Simple PUT/GET
    println!("📝 Test 1: Simple PUT/GET");
    let key1 = "test_key_1".to_string();
    let value1 = TestData { value: 42 };
    
    match cache.put(key1.clone(), value1.clone()) {
        Ok(()) => println!("  ✅ PUT successful"),
        Err(e) => println!("  ❌ PUT failed: {:?}", e),
    }
    
    match cache.get(&key1) {
        Some(retrieved) => {
            if retrieved == value1 {
                println!("  ✅ GET successful - values match");
            } else {
                println!("  ❌ GET returned wrong value: {:?}", retrieved);
            }
        }
        None => println!("  ❌ GET failed - key not found"),
    }
    
    // Test 2: Multiple operations to potentially trigger eviction
    println!("\n📝 Test 2: Multiple PUT operations (trigger eviction)");
    let mut success_count = 0;
    let mut get_success_count = 0;
    
    for i in 0..100 {
        let key = format!("key_{}", i);
        let value = TestData { value: i };
        
        match cache.put(key.clone(), value.clone()) {
            Ok(()) => {
                success_count += 1;
                // Try to GET immediately after PUT
                if let Some(retrieved) = cache.get(&key)
                    && retrieved == value {
                        get_success_count += 1;
                    }
            }
            Err(e) => println!("  ❌ PUT {} failed: {:?}", i, e),
        }
    }
    
    println!("  📊 PUT operations successful: {}/100", success_count);
    println!("  📊 GET operations successful: {}/100", get_success_count);
    
    if success_count > 0 && get_success_count > 0 {
        println!("\n🎉 SUCCESS: Cache is working! PUT/GET operations are successful");
        println!("   Hit rate: {}%", (get_success_count * 100) / success_count);
    } else {
        println!("\n💥 FAILURE: Cache is still not working properly");
    }
    
    println!("\n✅ Test completed");
}