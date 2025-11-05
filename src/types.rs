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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    // ========================================================================
    // EngineSide Tests
    // ========================================================================
    
    #[test]
    fn test_engine_side_equality() {
        assert_eq!(EngineSide::Buy, EngineSide::Buy);
        assert_eq!(EngineSide::Sell, EngineSide::Sell);
        assert_ne!(EngineSide::Buy, EngineSide::Sell);
    }

    // ========================================================================
    // EngineOrder Tests
    // ========================================================================
    
    #[test]
    fn test_engine_order_creation() {
        let order = EngineOrder::new(1, EngineSide::Buy, 100.5, 50);
        assert_eq!(order.id, 1);
        assert_eq!(order.side, EngineSide::Buy);
        assert_eq!(order.price, 100.5);
        assert_eq!(order.quantity, 50);
    }

    #[test]
    fn test_engine_order_is_empty() {
        let mut order = EngineOrder::new(1, EngineSide::Buy, 100.0, 10);
        assert!(!order.is_empty());
        
        order.quantity = 0;
        assert!(order.is_empty());
    }

    #[test]
    fn test_engine_order_timestamp_set() {
        let before = Utc::now();
        let order = EngineOrder::new(1, EngineSide::Buy, 100.0, 10);
        let after = Utc::now();
        
        assert!(order.timestamp >= before);
        assert!(order.timestamp <= after);
    }

    // ========================================================================
    // Request Tests
    // ========================================================================
    
    #[test]
    fn test_request_creation() {
        let order = EngineOrder::new(1, EngineSide::Buy, 100.0, 50);
        let request = Request {
            id: 42,
            order: order.clone(),
        };
        
        assert_eq!(request.id, 42);
        assert_eq!(request.order.id, 1);
    }

    // ========================================================================
    // OrderBookRef Tests
    // ========================================================================
    
    #[test]
    fn test_orderbook_ref_validation() {
        let book_ref = OrderBookRef {
            symbol: "BTCUSD".to_string(),
        };
        
        // Currently always returns true
        assert!(book_ref.validate_order(1, 100.0, 50));
        assert!(book_ref.validate_order(2, 0.0, 0)); // Even invalid inputs pass
    }

    // ========================================================================
    // Side Tests
    // ========================================================================
    
    #[test]
    fn test_side_enum() {
        assert_eq!(Side::BUY, Side::BUY);
        assert_eq!(Side::SELL, Side::SELL);
        assert_ne!(Side::BUY, Side::SELL);
    }

    #[test]
    fn test_side_serialization() {
        let buy = Side::BUY;
        let sell = Side::SELL;
        
        let buy_json = serde_json::to_string(&buy).unwrap();
        let sell_json = serde_json::to_string(&sell).unwrap();
        
        assert!(buy_json.contains("BUY"));
        assert!(sell_json.contains("SELL"));
    }

    // ========================================================================
    // MatchingAlgorithm Tests
    // ========================================================================
    
    #[test]
    fn test_matching_algorithm_variants() {
        let fifo = MatchingAlgorithm::FIFO;
        let pro_rata = MatchingAlgorithm::ProRata;
        let hybrid = MatchingAlgorithm::Hybrid;
        let hierarchical = MatchingAlgorithm::Hierarchical;
        
        assert_eq!(fifo, MatchingAlgorithm::FIFO);
        assert_eq!(pro_rata, MatchingAlgorithm::ProRata);
        assert_eq!(hybrid, MatchingAlgorithm::Hybrid);
        assert_eq!(hierarchical, MatchingAlgorithm::Hierarchical);
    }

    #[test]
    fn test_matching_algorithm_default() {
        let default = MatchingAlgorithm::default();
        assert_eq!(default, MatchingAlgorithm::Hierarchical);
    }

    #[test]
    fn test_matching_algorithm_serialization() {
        let algo = MatchingAlgorithm::FIFO;
        let json = serde_json::to_string(&algo).unwrap();
        assert!(json.contains("FIFO"));
        
        let deserialized: MatchingAlgorithm = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, MatchingAlgorithm::FIFO);
    }

    // ========================================================================
    // OrderIn Tests
    // ========================================================================
    
    #[test]
    fn test_order_in_deserialization() {
        let json = r#"{
            "symbol": "BTCUSD",
            "price": 50000,
            "qty": 10
        }"#;
        
        let order: OrderIn = serde_json::from_str(json).unwrap();
        assert_eq!(order.symbol, "BTCUSD");
        assert_eq!(order.price, 50000);
        assert_eq!(order.qty, 10);
        assert_eq!(order.algorithm, MatchingAlgorithm::Hierarchical); // default
    }

    #[test]
    fn test_order_in_with_algorithm() {
        let json = r#"{
            "symbol": "ETHUSD",
            "price": 3000,
            "qty": 5,
            "algorithm": "FIFO"
        }"#;
        
        let order: OrderIn = serde_json::from_str(json).unwrap();
        assert_eq!(order.algorithm, MatchingAlgorithm::FIFO);
    }

    // ========================================================================
    // Event Tests
    // ========================================================================
    
    #[test]
    fn test_event_new_order() {
        let event = Event::new_order(Side::BUY, 100, 50, "BTCUSD".to_string());
        
        assert!(event.order_id > 0);
        assert_eq!(event.side, Side::BUY);
        assert_eq!(event.price, 100);
        assert_eq!(event.qty, 50);
        assert_eq!(event.symbol, "BTCUSD");
        assert_eq!(event.algorithm, MatchingAlgorithm::Hierarchical);
    }

    #[test]
    fn test_event_new_order_with_algorithm() {
        let event = Event::new_order_with_algorithm(
            Side::SELL,
            200,
            75,
            "ETHUSD".to_string(),
            MatchingAlgorithm::FIFO,
        );
        
        assert_eq!(event.side, Side::SELL);
        assert_eq!(event.price, 200);
        assert_eq!(event.qty, 75);
        assert_eq!(event.algorithm, MatchingAlgorithm::FIFO);
    }

    #[test]
    fn test_event_unique_order_ids() {
        let event1 = Event::new_order(Side::BUY, 100, 50, "BTCUSD".to_string());
        let event2 = Event::new_order(Side::BUY, 100, 50, "BTCUSD".to_string());
        
        assert_ne!(event1.order_id, event2.order_id);
        assert!(event2.order_id > event1.order_id);
    }

    // ========================================================================
    // Order Tests
    // ========================================================================
    
    #[test]
    fn test_order_creation() {
        let timestamp = Utc::now();
        let order = Order::new(1, 100, 50, Side::BUY, timestamp);
        
        assert_eq!(order.order_id, 1);
        assert_eq!(order.price, 100);
        assert_eq!(order.qty, 50);
        assert_eq!(order.side, Side::BUY);
        assert_eq!(order.timestamp, timestamp);
    }

    #[test]
    fn test_order_from_event() {
        let event = Event::new_order(Side::SELL, 200, 75, "ETHUSD".to_string());
        let order = Order::from_event(&event);
        
        assert_eq!(order.order_id, event.order_id);
        assert_eq!(order.price, event.price);
        assert_eq!(order.qty, event.qty);
        assert_eq!(order.side, event.side);
        assert_eq!(order.timestamp, event.timestamp);
    }

    #[test]
    fn test_order_is_empty() {
        let mut order = Order::new(1, 100, 10, Side::BUY, Utc::now());
        assert!(!order.is_empty());
        
        order.qty = 0;
        assert!(order.is_empty());
    }

    // ========================================================================
    // Order Ordering Tests (for BinaryHeap)
    // ========================================================================
    
    #[test]
    fn test_order_buy_price_priority() {
        let timestamp = Utc::now();
        let order1 = Order::new(1, 100, 10, Side::BUY, timestamp);
        let order2 = Order::new(2, 105, 10, Side::BUY, timestamp);
        
        // Higher price should have higher priority (greater)
        assert!(order2 > order1);
    }

    #[test]
    fn test_order_buy_time_priority() {
        let early = Utc::now();
        let late = early + Duration::seconds(1);
        
        let order1 = Order::new(1, 100, 10, Side::BUY, early);
        let order2 = Order::new(2, 100, 10, Side::BUY, late);
        
        // Same price, earlier time should have higher priority
        assert!(order1 > order2);
    }

    #[test]
    fn test_order_sell_price_priority() {
        let timestamp = Utc::now();
        let order1 = Order::new(1, 100, 10, Side::SELL, timestamp);
        let order2 = Order::new(2, 95, 10, Side::SELL, timestamp);
        
        // Lower price should have higher priority (greater)
        assert!(order2 > order1);
    }

    #[test]
    fn test_order_sell_time_priority() {
        let early = Utc::now();
        let late = early + Duration::seconds(1);
        
        let order1 = Order::new(1, 100, 10, Side::SELL, early);
        let order2 = Order::new(2, 100, 10, Side::SELL, late);
        
        // Same price, earlier time should have higher priority
        assert!(order1 > order2);
    }

    #[test]
    fn test_order_equality() {
        let timestamp = Utc::now();
        let order1 = Order::new(1, 100, 10, Side::BUY, timestamp);
        let order2 = Order::new(2, 100, 10, Side::BUY, timestamp);
        
        // Same price and timestamp = equal for ordering purposes
        assert_eq!(order1, order2);
    }

    // ========================================================================
    // Trade Tests
    // ========================================================================
    
    #[test]
    fn test_trade_creation() {
        let trade = Trade::new(1, 2, 100, 50);
        
        assert!(trade.trade_id > 0);
        assert_eq!(trade.buy_order_id, 1);
        assert_eq!(trade.sell_order_id, 2);
        assert_eq!(trade.price, 100);
        assert_eq!(trade.qty, 50);
    }

    #[test]
    fn test_trade_unique_ids() {
        let trade1 = Trade::new(1, 2, 100, 50);
        let trade2 = Trade::new(3, 4, 100, 50);
        
        assert_ne!(trade1.trade_id, trade2.trade_id);
        assert!(trade2.trade_id > trade1.trade_id);
    }

    #[test]
    fn test_trade_timestamp() {
        let before = Utc::now();
        let trade = Trade::new(1, 2, 100, 50);
        let after = Utc::now();
        
        assert!(trade.timestamp >= before);
        assert!(trade.timestamp <= after);
    }

    // ========================================================================
    // Edge Case Tests
    // ========================================================================
    
    #[test]
    fn test_order_zero_quantity() {
        let order = Order::new(1, 100, 0, Side::BUY, Utc::now());
        assert!(order.is_empty());
    }

    #[test]
    fn test_order_large_quantities() {
        let order = Order::new(1, 100, u64::MAX, Side::BUY, Utc::now());
        assert_eq!(order.qty, u64::MAX);
        assert!(!order.is_empty());
    }

    #[test]
    fn test_order_zero_price() {
        let order = Order::new(1, 0, 100, Side::BUY, Utc::now());
        assert_eq!(order.price, 0);
    }

    #[test]
    fn test_event_empty_symbol() {
        let event = Event::new_order(Side::BUY, 100, 50, "".to_string());
        assert_eq!(event.symbol, "");
    }
}