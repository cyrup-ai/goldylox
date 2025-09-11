use goldylox::Goldylox;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Creating cache...");
    let cache = match Goldylox::<String, String>::new() {
        Ok(c) => {
            println!("✅ Cache created successfully");
            c
        }
        Err(e) => {
            println!("❌ Cache creation failed: {:?}", e);
            return Err(e.into());
        }
    };
    
    println!("Testing basic PUT...");
    match cache.put("test".to_string(), "value".to_string()) {
        Ok(()) => println!("✅ PUT succeeded"),
        Err(e) => {
            println!("❌ PUT failed: {:?}", e);
            return Err(e.into());
        }
    }
    
    println!("Testing basic GET...");
    match cache.get(&"test".to_string()) {
        Some(value) => {
            println!("✅ GET succeeded: {}", value);
            assert_eq!(value, "value");
        }
        None => {
            println!("❌ GET failed: key not found");
            return Err("GET failed".into());
        }
    }
    
    println!("🎉 All tests passed!");
    Ok(())
}