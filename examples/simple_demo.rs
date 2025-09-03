//! Simple Goldylox Cache Demo - Basic Functionality
//! 
//! Shows what currently works in the public API

use goldylox::Goldylox;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, bincode::Encode, bincode::Decode)]
pub struct Product {
    pub id: u64,
    pub name: String,
    pub price: f64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Goldylox Simple Demo");
    println!("========================\n");

    // Test if basic cache creation works
    match Goldylox::<String, Product>::new() {
        Ok(cache) => {
            println!("✅ Cache created successfully");
            
            // Test basic operations
            let product = Product {
                id: 1,
                name: "Test Product".to_string(),
                price: 99.99,
            };
            
            cache.put("product:1".to_string(), product.clone())?;
            println!("✅ Product stored");
            
            if let Some(retrieved) = cache.get(&"product:1".to_string()) {
                println!("✅ Product retrieved: {}", retrieved.name);
            }
            
            if let Ok(stats) = cache.stats() {
                println!("📊 Cache stats: {}", stats);
            }
            
            println!("\n🎉 Basic cache operations working!");
        },
        Err(e) => {
            println!("❌ Cache creation failed: {:?}", e);
            return Err(Box::new(e));
        }
    }

    Ok(())
}