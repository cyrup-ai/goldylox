//! Goldylox E-commerce Cache Example
//!
//! This implementation uses Goldylox's multi-tier cache system with
//! realistic e-commerce data patterns with products, user sessions, and analytics.

mod ecommerce;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ecommerce::main().await
}
