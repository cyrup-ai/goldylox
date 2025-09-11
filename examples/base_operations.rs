use goldylox::Goldylox;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache = Goldylox::<String, String>::new()?;
    
    println!("Putting key-value pair...");
    cache.put("key".to_string(), "value".to_string())?;
    println!("Put operation completed");
    
    println!("Getting key...");
    match cache.get(&"key".to_string()) {
        Some(val) => println!("Got: {}", val),
        None => println!("Not found"),
    }
    
    // Let's also check cache statistics
    match cache.stats() {
        Ok(stats_json) => println!("Cache stats: {}", stats_json),
        Err(e) => println!("Error getting stats: {}", e),
    }
    
    Ok(())
}