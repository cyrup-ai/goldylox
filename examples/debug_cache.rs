use goldylox::Goldylox;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== CACHE DEBUG TEST ===");
    
    let cache = Goldylox::<String, String>::new()?;
    println!("Cache created successfully");
    
    let key = "test_key".to_string();
    let value = "test_value".to_string();
    
    println!("Putting key='{}', value='{}'", key, value);
    match cache.put(key.clone(), value.clone()) {
        Ok(()) => println!("PUT operation successful"),
        Err(e) => {
            println!("PUT operation failed: {}", e);
            return Err(e.into());
        }
    }
    
    println!("Getting key='{}'", key);
    match cache.get(&key) {
        Some(retrieved_value) => {
            println!("GET operation successful: found '{}'", retrieved_value);
            if retrieved_value == value {
                println!("✅ Value matches!");
            } else {
                println!("❌ Value mismatch! Expected '{}', got '{}'", value, retrieved_value);
            }
        }
        None => {
            println!("❌ GET operation failed: key not found");
        }
    }
    
    // Print detailed stats
    match cache.stats() {
        Ok(stats) => println!("Cache stats: {}", stats),
        Err(e) => println!("Failed to get stats: {}", e),
    }
    
    Ok(())
}