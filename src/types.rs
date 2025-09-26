use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    BUY,
    SELL,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrderIn {
    pub symbol: String,
    pub price: u64,
    pub qty: u64,
}

#[derive(Debug, Clone)]
pub struct Event {
    pub side: Side,
    pub price: u64,
    pub qty: u64,
    pub symbol: String,
}

impl Event {
    pub fn new_order(side: Side, price: u64, qty: u64, symbol: String) -> Self {
        Self {
            side,
            price,
            qty,
            symbol,
        }
    }
}

pub struct Order {
    pub price: u64,
    #[allow(dead_code)]
    pub qty: u64,
    pub side: Side,
}

impl Order {
    pub fn new(price: u64, qty: u64, side: Side) -> Self {
        Self { price, qty, side }
    }
}

// For BinaryHeap - Buy orders (higher price has higher priority)
impl PartialEq for Order {
    fn eq(&self, other: &Self) -> bool {
        self.price == other.price
    }
}

impl Eq for Order {}

impl PartialOrd for Order {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Order {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.side {
            Side::BUY => self.price.cmp(&other.price), // Higher price = higher priority for buy
            Side::SELL => other.price.cmp(&self.price), // Lower price = higher priority for sell
        }
    }
}
