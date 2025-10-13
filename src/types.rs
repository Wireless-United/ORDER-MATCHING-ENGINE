use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::upper_case_acronyms)]
pub enum Side {
    BUY,
    SELL,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlgorithmType {
    FIFO,
    ProRata,
    Hybrid,
    #[serde(other)]
    Hierarchical,
}

impl Default for AlgorithmType {
    fn default() -> Self {
        AlgorithmType::Hierarchical
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrderIn {
    pub symbol: String,
    pub price: u64,
    pub qty: u64,
    #[serde(default)]
    pub algorithm: AlgorithmType,
}

#[derive(Debug, Clone)]
pub struct Event {
    pub side: Side,
    pub price: u64,
    pub qty: u64,
    pub symbol: String,
    #[allow(dead_code)]
    pub algorithm: AlgorithmType,
    #[allow(dead_code)]
    pub order_id: u64,
    #[allow(dead_code)]
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Event {
    pub fn new_order(side: Side, price: u64, qty: u64, symbol: String) -> Self {
        Self {
            side,
            price,
            qty,
            symbol,
            algorithm: AlgorithmType::default(),
            order_id: generate_order_id(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn new_order_with_algorithm(
        side: Side,
        price: u64,
        qty: u64,
        symbol: String,
        algorithm: AlgorithmType,
    ) -> Self {
        Self {
            side,
            price,
            qty,
            symbol,
            algorithm,
            order_id: generate_order_id(),
            timestamp: chrono::Utc::now(),
        }
    }
}

use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
static ORDER_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

fn generate_order_id() -> u64 {
    ORDER_ID_COUNTER.fetch_add(1, AtomicOrdering::Relaxed)
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

// Trade struct for API compatibility
#[derive(Debug, Clone)]
pub struct Trade {
    pub trade_id: u64,
    pub buy_id: u64,
    pub sell_id: u64,
    pub buy_order_id: u64,
    pub sell_order_id: u64,
    pub price: u64,
    pub qty: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
