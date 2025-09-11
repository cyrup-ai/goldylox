use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("Testing cold tier initialization...");
    
    // Try to initialize just the cold tier
    match goldylox::cache::tier::cold::init_cold_tier::<String, String>("/tmp", "test_cache") {
        Ok(_) => {
            println!("✅ Cold tier initialized successfully");
            
            // Try to perform a simple operation
            println!("Testing cold tier PUT...");
            match goldylox::cache::tier::cold::insert_demoted("test_key".to_string(), "test_value".to_string()) {
                Ok(_) => println!("✅ Cold tier PUT successful"),
                Err(e) => println!("❌ Cold tier PUT failed: {:?}", e),
            }
            
            println!("Testing cold tier GET...");
            match goldylox::cache::tier::cold::cold_get::<String, String>(&"test_key".to_string()) {
                Ok(Some(value)) => println!("✅ Cold tier GET successful: {}", value),
                Ok(None) => println!("⚠️  Cold tier GET: key not found"),
                Err(e) => println!("❌ Cold tier GET failed: {:?}", e),
            }
        },
        Err(e) => {
            println!("❌ Cold tier initialization failed: {:?}", e);
            return Err(e.into());
        }
    }
    
    Ok(())
}