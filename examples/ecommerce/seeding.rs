//! Data seeding functions for e-commerce example
//!
//! This module handles seeding the cache with realistic e-commerce data
//! including products, user sessions, and helper data generators.

use crate::ecommerce::types::*;
use rand::prelude::IndexedRandom;
use rand::rngs::ThreadRng;
use rand::{Rng, rng};
use std::collections::BTreeMap;

/// Seed the product catalog with realistic e-commerce data
pub fn seed_product_catalog(workload: &WorkloadState) -> Result<(), Box<dyn std::error::Error>> {
    let categories = [
        "Electronics",
        "Clothing",
        "Home & Garden",
        "Sports",
        "Books",
        "Health",
        "Automotive",
        "Toys",
        "Beauty",
        "Groceries",
    ];

    let mut rng = rng();

    // Create 10,000 realistic products
    for product_id in 1..=10_000 {
        let category = categories.choose(&mut rng).unwrap();
        let product = Product {
            product_id,
            name: generate_product_name(category, product_id),
            category: category.to_string(),
            price: rng.random_range(9.99..999.99),
            inventory_count: rng.random_range(0..1000),
            description: format!(
                "High-quality {} product #{}",
                category.to_lowercase(),
                product_id
            ),
            reviews_count: rng.random_range(0..5000),
            average_rating: rng.random_range(1.0..5.0),
            tags: generate_product_tags(category),
            metadata: generate_product_metadata(&mut rng),
            last_updated: current_timestamp(),
        };

        // Distribute products across nodes based on hash for even distribution
        let node_index = (product_id as usize) % workload.nodes.len();
        let cache_key = format!("product:{}", product_id);

        workload.nodes[node_index]
            .product_cache
            .put(cache_key, product)?;
    }

    Ok(())
}
/// Seed user sessions with realistic patterns
pub fn seed_user_sessions(workload: &WorkloadState) -> Result<(), Box<dyn std::error::Error>> {
    let mut rng = rng();

    // Create 1,000 user sessions with realistic shopping behavior
    for user_id in 1..=1_000 {
        let session_id = format!("sess_{:06}", user_id);
        let shopping_cart = generate_realistic_cart(&mut rng);
        let browsing_history = generate_browsing_history(&mut rng);

        let session = UserSession {
            session_id: session_id.clone(),
            user_id,
            created_at: current_timestamp() - rng.random_range(0..3600), // Created within last hour
            last_activity: current_timestamp() - rng.random_range(0..600), // Active within 10 minutes
            expires_at: current_timestamp() + 3600,                        // Expires in 1 hour
            shopping_cart,
            browsing_history,
            preferences: generate_user_preferences(&mut rng),
            location: generate_user_location(&mut rng),
        };

        // ACTUALLY PUT SESSION DATA INTO THE REAL CACHE
        let node_index = (user_id as usize) % workload.nodes.len();
        let cache_key = format!("session:{}", session_id);

        workload.nodes[node_index]
            .session_cache
            .put(cache_key, session)?;
    }

    Ok(())
}

/// Generate realistic product name based on category
fn generate_product_name(category: &str, id: u64) -> String {
    match category {
        "Electronics" => format!("Premium {} Device #{}", category, id),
        "Clothing" => format!("Designer {} Item #{}", category, id),
        _ => format!("{} Product #{}", category, id),
    }
}

/// Generate product tags based on category
fn generate_product_tags(category: &str) -> Vec<String> {
    match category {
        "Electronics" => vec![
            "tech".to_string(),
            "gadget".to_string(),
            "premium".to_string(),
        ],
        "Clothing" => vec![
            "fashion".to_string(),
            "style".to_string(),
            "trendy".to_string(),
        ],
        _ => vec!["quality".to_string(), "popular".to_string()],
    }
}

/// Generate product metadata with realistic attributes
fn generate_product_metadata(rng: &mut ThreadRng) -> BTreeMap<String, String> {
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "weight".to_string(),
        format!("{:.1}", rng.random_range(0.1..50.0)),
    );
    metadata.insert(
        "dimensions".to_string(),
        format!(
            "{}x{}x{}",
            rng.random_range(1..100),
            rng.random_range(1..100),
            rng.random_range(1..100)
        ),
    );
    metadata.insert(
        "color".to_string(),
        ["Red", "Blue", "Green", "Black", "White"]
            .choose(rng)
            .unwrap()
            .to_string(),
    );
    metadata
}

/// Generate realistic shopping cart with random items
fn generate_realistic_cart(rng: &mut ThreadRng) -> Vec<CartItem> {
    let cart_size = rng.random_range(0..8); // 0-7 items in cart
    let mut cart = Vec::new();

    for _ in 0..cart_size {
        cart.push(CartItem {
            product_id: rng.random_range(1..10_001),
            quantity: rng.random_range(1..5),
            added_at: current_timestamp() - rng.random_range(0..3600),
            price_at_add: rng.random_range(9.99..999.99),
        });
    }

    cart
}

/// Generate browsing history with random products
fn generate_browsing_history(rng: &mut ThreadRng) -> Vec<u64> {
    let history_size = rng.random_range(5..50); // 5-49 viewed products
    (0..history_size)
        .map(|_| rng.random_range(1..10_001))
        .collect()
}

/// Generate user preferences
fn generate_user_preferences(rng: &mut ThreadRng) -> BTreeMap<String, String> {
    let mut prefs = BTreeMap::new();
    prefs.insert(
        "currency".to_string(),
        ["USD", "EUR", "GBP"].choose(rng).unwrap().to_string(),
    );
    prefs.insert(
        "language".to_string(),
        ["en", "es", "fr", "de"].choose(rng).unwrap().to_string(),
    );
    prefs.insert(
        "theme".to_string(),
        ["light", "dark"].choose(rng).unwrap().to_string(),
    );
    prefs
}

/// Generate user location
fn generate_user_location(rng: &mut ThreadRng) -> String {
    ["US", "UK", "DE", "FR", "CA", "AU"]
        .choose(rng)
        .unwrap()
        .to_string()
}
