use ::goldylox::*;

fn main() {
    println!("Starting cache functionality test...");
    
    // Create a simple cache
    let cache = GoldyloxBuilder::<String, String>::new()
        .hot_tier_max_entries(128)
        .build()
        .expect("Failed to create cache");
    
    println!("Cache created successfully");
    
    // Test basic PUT and GET operations
    println!("Testing PUT operations...");
    
    for i in 1..=10 {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        match cache.put(key.clone(), value.clone()) {
            Ok(()) => println!("✓ PUT {} = {}", key, value),
            Err(e) => println!("✗ PUT failed for {}: {:?}", key, e),
        }
    }
    
    println!("\nTesting GET operations...");
    
    for i in 1..=10 {
        let key = format!("key{}", i);
        match cache.get(&key) {
            Some(value) => println!("✓ GET {} = {}", key, value),
            None => println!("✗ GET failed for {}: not found", key),
        }
    }
    
    println!("\nCache functionality test completed successfully!");
}