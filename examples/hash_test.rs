use goldylox::Goldylox;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache = Goldylox::<String, String>::new()?;
    let key = "test_key".to_string();
    
    // Generate hash multiple times for the same key
    let hash1 = cache.hash_key(&key);
    let hash2 = cache.hash_key(&key);
    let hash3 = cache.hash_key(&key);
    
    println!("Hash consistency test:");
    println!("Key: {:?}", key);
    println!("Hash 1: {}", hash1);
    println!("Hash 2: {}", hash2);  
    println!("Hash 3: {}", hash3);
    println!("Consistent: {}", hash1 == hash2 && hash2 == hash3);
    
    // Now test with PUT and GET operations
    println!("\nPUT/GET hash comparison:");
    
    // Put a value
    cache.put(key.clone(), "test_value".to_string())?;
    println!("PUT completed");
    
    // Generate hash after PUT
    let hash_after_put = cache.hash_key(&key);
    println!("Hash after PUT: {}", hash_after_put);
    
    // Get the value  
    let result = cache.get(&key);
    println!("GET result: {:?}", result);
    
    // Generate hash after GET
    let hash_after_get = cache.hash_key(&key);
    println!("Hash after GET: {}", hash_after_get);
    
    println!("PUT/GET hash consistent: {}", hash_after_put == hash_after_get);
    
    Ok(())
}