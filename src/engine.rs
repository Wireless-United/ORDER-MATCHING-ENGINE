//! Engine module providing core types for the matching algorithms

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct Order {
    pub id: u64,
    pub side: Side,
    pub price: f64,
    pub quantity: u64,
    pub timestamp: DateTime<Utc>,
}

impl Order {
    pub fn new(id: u64, side: Side, price: f64, quantity: u64) -> Self {
        Self {
            id,
            side,
            price,
            quantity,
            timestamp: Utc::now(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.quantity == 0
    }
}

// Request and OrderBookRef are placeholders for future functionality
#[derive(Debug, Clone)]
pub struct Request {
    pub id: u64,
    pub order: Order,
}

#[derive(Debug, Clone)]
pub struct OrderBookRef {
    pub symbol: String,
}

impl OrderBookRef {
    pub fn validate_order(&self, _id: u64, _price: f64, _quantity: u64) -> bool {
        // Placeholder validation - always returns true
        true
    }
}
