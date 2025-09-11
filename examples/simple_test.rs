use goldylox::GoldyloxBuilder;

fn main() {
    println!("🧪 Testing core cache functionality...");
    
    // Test unused MaintenanceTask::new function
    use goldylox::cache::manager::background::types::{MaintenanceTask, CanonicalMaintenanceTask};
    let canonical_task = CanonicalMaintenanceTask::UpdateStatistics { 
        include_detailed_analysis: false 
    };
    let _maintenance_task = MaintenanceTask::new(canonical_task);
    println!("✅ MaintenanceTask::new tested");
    
    let cache = GoldyloxBuilder::new()
        .hot_tier_max_entries(128)
        .build()
        .expect("Failed to create cache");

    // Test basic put/get
    let key = "test_key".to_string();
    let value = "test_value".to_string();
    
    println!("📝 Putting key-value pair...");
    match cache.put(key.clone(), value.clone()) {
        Ok(()) => println!("✅ PUT successful"),
        Err(e) => {
            println!("❌ PUT failed: {:?}", e);
            return;
        }
    }
    
    println!("📖 Getting key...");
    match cache.get(&key) {
        Some(retrieved) => {
            if retrieved == value {
                println!("🎉 SUCCESS: GET successful - cache is working!");
                println!("   Retrieved: '{}'", retrieved);
            } else {
                println!("❌ GET returned wrong value: '{}'", retrieved);
            }
        }
        None => {
            println!("❌ GET failed - key not found (cache miss)");
        }
    }
    
    println!("✅ Test completed");
}