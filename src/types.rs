use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use chrono::{DateTime, Utc};

static GLOBAL_ORDER_ID: AtomicU64 = AtomicU64::new(1);

// Core types from engine.rs
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EngineSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct EngineOrder {
    pub id: u64,
    pub side: EngineSide,
    pub price: f64,
    pub quantity: u64,
    pub timestamp: DateTime<Utc>,
}

impl EngineOrder {
    pub fn new(id: u64, side: EngineSide, price: f64, quantity: u64) -> Self {
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

#[derive(Debug, Clone)]
pub struct Request {
    pub id: u64,
    pub order: EngineOrder,
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

// End of core types from engine.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    BUY,
    SELL,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchingAlgorithm {
    FIFO,
    ProRata,
    Hybrid,
    Hierarchical,
}

impl Default for MatchingAlgorithm {
    fn default() -> Self {
        MatchingAlgorithm::Hierarchical
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrderIn {
    pub symbol: String,
    pub price: u64,
    pub qty: u64,
    #[serde(default)]
    pub algorithm: MatchingAlgorithm,
}

#[derive(Debug, Clone)]
pub struct Event {
    pub order_id: u64,
    pub side: Side,
    pub price: u64,
    pub qty: u64,
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub algorithm: MatchingAlgorithm,
}

impl Event {
    pub fn new_order(side: Side, price: u64, qty: u64, symbol: String) -> Self {
        Self::new_order_with_algorithm(side, price, qty, symbol, MatchingAlgorithm::default())
    }

    pub fn new_order_with_algorithm(
        side: Side,
        price: u64,
        qty: u64,
        symbol: String,
        algorithm: MatchingAlgorithm,
    ) -> Self {
        let order_id = GLOBAL_ORDER_ID.fetch_add(1, AtomicOrdering::SeqCst);
        Self {
            order_id,
            side,
            price,
            qty,
            symbol,
            timestamp: Utc::now(),
            algorithm,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Order {
    pub order_id: u64,
    pub price: u64,
    pub qty: u64,
    pub side: Side,
    pub timestamp: DateTime<Utc>,
}

impl Order {
    pub fn new(order_id: u64, price: u64, qty: u64, side: Side, timestamp: DateTime<Utc>) -> Self {
        Self {
            order_id,
            price,
            qty,
            side,
            timestamp,
        }
    }

    pub fn from_event(event: &Event) -> Self {
        Self {
            order_id: event.order_id,
            price: event.price,
            qty: event.qty,
            side: event.side,
            timestamp: event.timestamp,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.qty == 0
    }
}

// For BinaryHeap - Buy orders (higher price has higher priority)
// If prices are equal, earlier timestamp (FIFO) has priority
impl PartialEq for Order {
    fn eq(&self, other: &Self) -> bool {
        self.price == other.price && self.timestamp == other.timestamp
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
            Side::BUY => {
                // Higher price = higher priority for buy
                // If prices equal, earlier timestamp (smaller) = higher priority
                match self.price.cmp(&other.price) {
                    Ordering::Equal => other.timestamp.cmp(&self.timestamp),
                    other_ordering => other_ordering,
                }
            }
            Side::SELL => {
                // Lower price = higher priority for sell
                // If prices equal, earlier timestamp (smaller) = higher priority
                match other.price.cmp(&self.price) {
                    Ordering::Equal => other.timestamp.cmp(&self.timestamp),
                    other_ordering => other_ordering,
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Trade {
    pub trade_id: u64,
    pub buy_order_id: u64,
    pub sell_order_id: u64,
    pub price: u64,
    pub qty: u64,
    pub timestamp: DateTime<Utc>,
}

static GLOBAL_TRADE_ID: AtomicU64 = AtomicU64::new(1);

impl Trade {
    pub fn new(buy_order_id: u64, sell_order_id: u64, price: u64, qty: u64) -> Self {
        let trade_id = GLOBAL_TRADE_ID.fetch_add(1, AtomicOrdering::SeqCst);
        Self {
            trade_id,
            buy_order_id,
            sell_order_id,
            price,
            qty,
            timestamp: Utc::now(),
        }
    }
}
