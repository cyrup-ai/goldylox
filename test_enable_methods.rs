// Test to see if enable methods exist
use goldylox::Goldylox;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, bincode::Encode, bincode::Decode)]
pub struct TestData {
    pub id: u64,
    pub name: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Try to call the enable methods that examples use
    let cache = Goldylox::<String, TestData>::builder()
        .enable_simd(true)  // This should fail if method doesn't exist
        .build()?;
    
    println!("Cache created successfully");
    Ok(())
}