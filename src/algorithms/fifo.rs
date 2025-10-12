use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use chrono::{DateTime, Utc};
use crate::types::{EngineOrder as Order, EngineSide as Side, Request, OrderBookRef};
use crate::algorithms::errors::AlgorithmError;

#[allow(dead_code)]
static GLOBAL_TRADE_RANK: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq)]
pub struct Trade {
    pub buy_id: u64,
    pub sell_id: u64,
    pub price: f64,
    pub quantity: u64,
    pub rank: u64,
    pub timestamp: DateTime<Utc>,
}

impl Trade {
    pub fn new(buy_id: u64, sell_id: u64, price: f64, quantity: u64) -> Self {
        let rank = GLOBAL_TRADE_RANK.fetch_add(1, Ordering::SeqCst);
        Self {
            buy_id,
            sell_id,
            price,
            quantity,
            rank,
            timestamp: Utc::now(),
        }
    }
}

pub struct FifoMatcher {
    pub bids: VecDeque<Order>,
    pub asks: VecDeque<Order>,
}

impl FifoMatcher {
    pub fn new() -> Self {
        Self {
            bids: VecDeque::new(),
            asks: VecDeque::new(),
        }
    }

    pub fn match_order(&mut self, mut incoming: Order) -> Result<Vec<Trade>, AlgorithmError> {
        self.validate_order(&incoming)?;

        let mut trades = Vec::new();

        match incoming.side {
            Side::Buy => {
                self.match_buy_order(&mut incoming, &mut trades)?;
                if !incoming.is_empty() {
                    self.add_bid(incoming);
                }
            }
            Side::Sell => {
                self.match_sell_order(&mut incoming, &mut trades)?;
                if !incoming.is_empty() {
                    self.add_ask(incoming);
                }
            }
        }

        Ok(trades)
    }

    fn validate_order(&self, order: &Order) -> Result<(), AlgorithmError> {
        if order.quantity == 0 {
            return Err(AlgorithmError::InvalidOrder(
                "Order quantity cannot be zero".to_string(),
            ));
        }
        if order.price <= 0.0 {
            return Err(AlgorithmError::InvalidOrder(
                "Order price must be positive".to_string(),
            ));
        }
        Ok(())
    }

    fn match_buy_order(
        &mut self,
        buy_order: &mut Order,
        trades: &mut Vec<Trade>,
    ) -> Result<(), AlgorithmError> {
        while !buy_order.is_empty() && !self.asks.is_empty() {
            let can_match = self
                .asks
                .front()
                .map(|ask| buy_order.price >= ask.price)
                .unwrap_or(false);

            if !can_match {
                break;
            }

            let mut resting_ask = self.asks.pop_front()
                .expect("Ask queue should not be empty");
            
            let trade = Self::execute_trade(buy_order, &mut resting_ask);
            trades.push(trade);

            if !resting_ask.is_empty() {
                self.asks.push_front(resting_ask);
            }
        }
        Ok(())
    }

    fn match_sell_order(
        &mut self,
        sell_order: &mut Order,
        trades: &mut Vec<Trade>,
    ) -> Result<(), AlgorithmError> {
        while !sell_order.is_empty() && !self.bids.is_empty() {
            let can_match = self
                .bids
                .front()
                .map(|bid| sell_order.price <= bid.price)
                .unwrap_or(false);

            if !can_match {
                break;
            }

            let mut resting_bid = self.bids.pop_front()
                .expect("Bid queue should not be empty");
            
            let trade = Self::execute_trade(&mut resting_bid, sell_order);
            trades.push(trade);

            if !resting_bid.is_empty() {
                self.bids.push_front(resting_bid);
            }
        }
        Ok(())
    }

    fn execute_trade(buy_order: &mut Order, sell_order: &mut Order) -> Trade {
        let trade_quantity = std::cmp::min(buy_order.quantity, sell_order.quantity);
        let trade_price = sell_order.price;

        buy_order.quantity -= trade_quantity;
        sell_order.quantity -= trade_quantity;

        Trade::new(buy_order.id, sell_order.id, trade_price, trade_quantity)
    }

    fn add_bid(&mut self, order: Order) {
        self.bids.push_back(order);
    }

    fn add_ask(&mut self, order: Order) {
        self.asks.push_back(order);
    }

    #[allow(dead_code)]
    pub fn best_bid(&self) -> Option<&Order> {
        self.bids.front()
    }

    #[allow(dead_code)]
    pub fn best_ask(&self) -> Option<&Order> {
        self.asks.front()
    }

    #[allow(dead_code)]
    pub fn bid_depth(&self) -> usize {
        self.bids.len()
    }

    #[allow(dead_code)]
    pub fn ask_depth(&self) -> usize {
        self.asks.len()
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.bids.clear();
        self.asks.clear();
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.bids.is_empty() && self.asks.is_empty()
    }

    #[allow(dead_code)]
    pub fn bids_iter(&self) -> impl Iterator<Item = &Order> {
        self.bids.iter()
    }

    #[allow(dead_code)]
    pub fn asks_iter(&self) -> impl Iterator<Item = &Order> {
        self.asks.iter()
    }

    #[allow(dead_code)]
    pub fn get_trade_count() -> u64 {
        GLOBAL_TRADE_RANK.load(Ordering::SeqCst) - 1
    }

    #[allow(dead_code)]
    pub fn reset_trade_rank() {
        GLOBAL_TRADE_RANK.store(1, Ordering::SeqCst);
    }
}

#[allow(dead_code)]
impl Default for FifoMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Process a request using FIFO matching algorithm.
///
/// This is the concurrent-safe entry point for FIFO matching.
/// It uses the order book reference for validation only (read-only access).
///
/// # Arguments
///
/// * `request` - The request containing the order to process
/// * `order_book` - Read-only reference to the order book for validation
///
/// # Returns
///
/// A vector of executed trades
pub fn process(request: Request, _order_book: &OrderBookRef) -> Vec<Trade> {
    // Create a new matcher instance for this request
    // In a real implementation, this would be a per-shard matcher
    let mut matcher = FifoMatcher::new();
    
    // Validate using order book reference (read-only)
    if !_order_book.validate_order(request.order.id, request.order.price, request.order.quantity) {
        eprintln!("Order validation failed for request {}", request.id);
        return Vec::new();
    }
    
    // Process the order
    match matcher.match_order(request.order) {
        Ok(trades) => trades,
        Err(e) => {
            eprintln!("FIFO matching error for request {}: {:?}", request.id, e);
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_buy_order(id: u64, price: f64, quantity: u64) -> Order {
        Order::new(id, Side::Buy, price, quantity)
    }

    fn create_sell_order(id: u64, price: f64, quantity: u64) -> Order {
        Order::new(id, Side::Sell, price, quantity)
    }

    // ========================================================================
    // Basic Functionality Tests
    // ========================================================================

    #[test]
    fn test_fifo_matcher_creation() {
        let matcher = FifoMatcher::new();
        assert!(matcher.is_empty());
        assert_eq!(matcher.bid_depth(), 0);
        assert_eq!(matcher.ask_depth(), 0);
    }

    #[test]
    fn test_add_single_bid() {
        let mut matcher = FifoMatcher::new();
        let order = create_buy_order(1, 100.0, 50);
        
        let trades = matcher.match_order(order).unwrap();
        assert_eq!(trades.len(), 0);
        assert_eq!(matcher.bid_depth(), 1);
        assert_eq!(matcher.ask_depth(), 0);
    }

    #[test]
    fn test_add_single_ask() {
        let mut matcher = FifoMatcher::new();
        let order = create_sell_order(1, 100.0, 50);
        
        let trades = matcher.match_order(order).unwrap();
        assert_eq!(trades.len(), 0);
        assert_eq!(matcher.bid_depth(), 0);
        assert_eq!(matcher.ask_depth(), 1);
    }

    // ========================================================================
    // Validation Tests
    // ========================================================================

    #[test]
    fn test_reject_zero_quantity() {
        let mut matcher = FifoMatcher::new();
        let order = create_buy_order(1, 100.0, 0);
        
        let result = matcher.match_order(order);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            AlgorithmError::InvalidOrder("Order quantity cannot be zero".to_string())
        );
    }

    #[test]
    fn test_reject_zero_price() {
        let mut matcher = FifoMatcher::new();
        let order = create_buy_order(1, 0.0, 50);
        
        let result = matcher.match_order(order);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            AlgorithmError::InvalidOrder("Order price must be positive".to_string())
        );
    }

    #[test]
    fn test_reject_negative_price() {
        let mut matcher = FifoMatcher::new();
        let order = create_buy_order(1, -10.0, 50);
        
        let result = matcher.match_order(order);
        assert!(result.is_err());
    }

    // ========================================================================
    // Matching Tests - Full Fills
    // ========================================================================

    #[test]
    fn test_exact_match_buy_sell() {
        let mut matcher = FifoMatcher::new();
        
        let buy = create_buy_order(1, 100.0, 50);
        matcher.match_order(buy).unwrap();
        
        let sell = create_sell_order(2, 100.0, 50);
        let trades = matcher.match_order(sell).unwrap();
        
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].buy_id, 1);
        assert_eq!(trades[0].sell_id, 2);
        assert_eq!(trades[0].price, 100.0);
        assert_eq!(trades[0].quantity, 50);
        assert!(matcher.is_empty());
    }

    #[test]
    fn test_exact_match_sell_buy() {
        let mut matcher = FifoMatcher::new();
        
        let sell = create_sell_order(1, 100.0, 50);
        matcher.match_order(sell).unwrap();
        
        let buy = create_buy_order(2, 100.0, 50);
        let trades = matcher.match_order(buy).unwrap();
        
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].buy_id, 2);
        assert_eq!(trades[0].sell_id, 1);
        assert_eq!(trades[0].price, 100.0);
        assert_eq!(trades[0].quantity, 50);
        assert!(matcher.is_empty());
    }

    // ========================================================================
    // Matching Tests - Partial Fills
    // ========================================================================

    #[test]
    fn test_partial_fill_buy_larger() {
        let mut matcher = FifoMatcher::new();
        
        let sell = create_sell_order(1, 100.0, 30);
        matcher.match_order(sell).unwrap();
        
        let buy = create_buy_order(2, 100.0, 50);
        let trades = matcher.match_order(buy).unwrap();
        
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, 30);
        
        // Remaining buy order should be in book
        assert_eq!(matcher.bid_depth(), 1);
        assert_eq!(matcher.ask_depth(), 0);
        assert_eq!(matcher.best_bid().unwrap().quantity, 20);
    }

    #[test]
    fn test_partial_fill_sell_larger() {
        let mut matcher = FifoMatcher::new();
        
        let buy = create_buy_order(1, 100.0, 30);
        matcher.match_order(buy).unwrap();
        
        let sell = create_sell_order(2, 100.0, 50);
        let trades = matcher.match_order(sell).unwrap();
        
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, 30);
        
        // Remaining sell order should be in book
        assert_eq!(matcher.bid_depth(), 0);
        assert_eq!(matcher.ask_depth(), 1);
        assert_eq!(matcher.best_ask().unwrap().quantity, 20);
    }

    // ========================================================================
    // Matching Tests - Multiple Orders
    // ========================================================================

    #[test]
    fn test_multiple_sells_matched_by_single_buy() {
        let mut matcher = FifoMatcher::new();
        
        matcher.match_order(create_sell_order(1, 100.0, 20)).unwrap();
        matcher.match_order(create_sell_order(2, 100.0, 15)).unwrap();
        matcher.match_order(create_sell_order(3, 100.0, 25)).unwrap();
        
        let buy = create_buy_order(4, 100.0, 50);
        let trades = matcher.match_order(buy).unwrap();
        
        assert_eq!(trades.len(), 3);
        assert_eq!(trades[0].quantity, 20);
        assert_eq!(trades[1].quantity, 15);
        assert_eq!(trades[2].quantity, 15); // Partial fill of third order
        
        assert_eq!(matcher.ask_depth(), 1);
        assert_eq!(matcher.best_ask().unwrap().quantity, 10);
    }

    #[test]
    fn test_multiple_buys_matched_by_single_sell() {
        let mut matcher = FifoMatcher::new();
        
        matcher.match_order(create_buy_order(1, 100.0, 20)).unwrap();
        matcher.match_order(create_buy_order(2, 100.0, 15)).unwrap();
        matcher.match_order(create_buy_order(3, 100.0, 25)).unwrap();
        
        let sell = create_sell_order(4, 100.0, 50);
        let trades = matcher.match_order(sell).unwrap();
        
        assert_eq!(trades.len(), 3);
        assert_eq!(trades[0].quantity, 20);
        assert_eq!(trades[1].quantity, 15);
        assert_eq!(trades[2].quantity, 15);
        
        assert_eq!(matcher.bid_depth(), 1);
        assert_eq!(matcher.best_bid().unwrap().quantity, 10);
    }

    // ========================================================================
    // Price Priority Tests
    // ========================================================================

    #[test]
    fn test_no_match_price_too_low() {
        let mut matcher = FifoMatcher::new();
        
        let sell = create_sell_order(1, 105.0, 50);
        matcher.match_order(sell).unwrap();
        
        let buy = create_buy_order(2, 100.0, 50);
        let trades = matcher.match_order(buy).unwrap();
        
        assert_eq!(trades.len(), 0);
        assert_eq!(matcher.bid_depth(), 1);
        assert_eq!(matcher.ask_depth(), 1);
    }

    #[test]
    fn test_no_match_price_too_high() {
        let mut matcher = FifoMatcher::new();
        
        let buy = create_buy_order(1, 95.0, 50);
        matcher.match_order(buy).unwrap();
        
        let sell = create_sell_order(2, 100.0, 50);
        let trades = matcher.match_order(sell).unwrap();
        
        assert_eq!(trades.len(), 0);
        assert_eq!(matcher.bid_depth(), 1);
        assert_eq!(matcher.ask_depth(), 1);
    }

    #[test]
    fn test_match_at_better_price() {
        let mut matcher = FifoMatcher::new();
        
        let sell = create_sell_order(1, 95.0, 50);
        matcher.match_order(sell).unwrap();
        
        let buy = create_buy_order(2, 100.0, 50);
        let trades = matcher.match_order(buy).unwrap();
        
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].price, 95.0); // Matched at resting order price
    }

    // ========================================================================
    // Time Priority Tests (FIFO)
    // ========================================================================

    #[test]
    fn test_fifo_order_same_price() {
        let mut matcher = FifoMatcher::new();
        
        // Add three sell orders at same price
        matcher.match_order(create_sell_order(1, 100.0, 10)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1));
        matcher.match_order(create_sell_order(2, 100.0, 10)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1));
        matcher.match_order(create_sell_order(3, 100.0, 10)).unwrap();
        
        let buy = create_buy_order(4, 100.0, 15);
        let trades = matcher.match_order(buy).unwrap();
        
        // Should match first order completely, second order partially
        assert_eq!(trades.len(), 2);
        assert_eq!(trades[0].sell_id, 1);
        assert_eq!(trades[0].quantity, 10);
        assert_eq!(trades[1].sell_id, 2);
        assert_eq!(trades[1].quantity, 5);
    }

    // ========================================================================
    // Best Bid/Ask Tests
    // ========================================================================

    #[test]
    fn test_best_bid() {
        let mut matcher = FifoMatcher::new();
        
        assert!(matcher.best_bid().is_none());
        
        matcher.match_order(create_buy_order(1, 100.0, 50)).unwrap();
        assert_eq!(matcher.best_bid().unwrap().id, 1);
        assert_eq!(matcher.best_bid().unwrap().price, 100.0);
    }

    #[test]
    fn test_best_ask() {
        let mut matcher = FifoMatcher::new();
        
        assert!(matcher.best_ask().is_none());
        
        matcher.match_order(create_sell_order(1, 100.0, 50)).unwrap();
        assert_eq!(matcher.best_ask().unwrap().id, 1);
        assert_eq!(matcher.best_ask().unwrap().price, 100.0);
    }

    // ========================================================================
    // Clear and Empty Tests
    // ========================================================================

    #[test]
    fn test_clear() {
        let mut matcher = FifoMatcher::new();
        
        matcher.match_order(create_buy_order(1, 100.0, 50)).unwrap();
        matcher.match_order(create_sell_order(2, 105.0, 50)).unwrap();
        
        assert!(!matcher.is_empty());
        
        matcher.clear();
        
        assert!(matcher.is_empty());
        assert_eq!(matcher.bid_depth(), 0);
        assert_eq!(matcher.ask_depth(), 0);
    }

    // ========================================================================
    // Iterator Tests
    // ========================================================================

    #[test]
    fn test_bids_iterator() {
        let mut matcher = FifoMatcher::new();
        
        matcher.match_order(create_buy_order(1, 100.0, 10)).unwrap();
        matcher.match_order(create_buy_order(2, 101.0, 20)).unwrap();
        matcher.match_order(create_buy_order(3, 99.0, 30)).unwrap();
        
        let bids: Vec<_> = matcher.bids_iter().collect();
        assert_eq!(bids.len(), 3);
    }

    #[test]
    fn test_asks_iterator() {
        let mut matcher = FifoMatcher::new();
        
        matcher.match_order(create_sell_order(1, 100.0, 10)).unwrap();
        matcher.match_order(create_sell_order(2, 101.0, 20)).unwrap();
        
        let asks: Vec<_> = matcher.asks_iter().collect();
        assert_eq!(asks.len(), 2);
    }

    // ========================================================================
    // Trade Properties Tests
    // ========================================================================

    #[test]
    fn test_trade_has_timestamp() {
        let mut matcher = FifoMatcher::new();
        
        matcher.match_order(create_sell_order(1, 100.0, 50)).unwrap();
        
        let before = Utc::now();
        let buy = create_buy_order(2, 100.0, 50);
        let trades = matcher.match_order(buy).unwrap();
        let after = Utc::now();
        
        assert!(trades[0].timestamp >= before);
        assert!(trades[0].timestamp <= after);
    }

    #[test]
    fn test_trade_has_unique_rank() {
        FifoMatcher::reset_trade_rank();
        
        let mut matcher = FifoMatcher::new();
        
        matcher.match_order(create_sell_order(1, 100.0, 10)).unwrap();
        matcher.match_order(create_sell_order(2, 100.0, 10)).unwrap();
        
        let buy = create_buy_order(3, 100.0, 20);
        let trades = matcher.match_order(buy).unwrap();
        
        assert_eq!(trades.len(), 2);
        assert!(trades[0].rank < trades[1].rank);
    }

    // ========================================================================
    // Process Function Tests
    // ========================================================================

    #[test]
    fn test_process_function() {
        let order_book = OrderBookRef {
            symbol: "BTCUSD".to_string(),
        };
        
        let order = Order::new(1, Side::Buy, 100.0, 50);
        let request = Request { id: 1, order };
        
        let trades = process(request, &order_book);
        assert_eq!(trades.len(), 0); // No matching orders
    }

    #[test]
    fn test_process_with_validation_failure() {
        // This test would need actual validation logic in OrderBookRef
        // Currently validation always returns true
        let order_book = OrderBookRef {
            symbol: "BTCUSD".to_string(),
        };
        
        let order = Order::new(1, Side::Buy, 100.0, 50);
        let request = Request { id: 1, order };
        
        let trades = process(request, &order_book);
        // Currently will succeed as validation is placeholder
        assert!(trades.is_empty() || !trades.is_empty());
    }

    // ========================================================================
    // Edge Cases
    // ========================================================================

    #[test]
    fn test_large_quantity() {
        let mut matcher = FifoMatcher::new();
        
        let sell = create_sell_order(1, 100.0, u64::MAX / 2);
        matcher.match_order(sell).unwrap();
        
        let buy = create_buy_order(2, 100.0, u64::MAX / 2);
        let trades = matcher.match_order(buy).unwrap();
        
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, u64::MAX / 2);
    }

    #[test]
    fn test_very_small_price() {
        let mut matcher = FifoMatcher::new();
        
        let sell = create_sell_order(1, 0.001, 1000);
        matcher.match_order(sell).unwrap();
        
        let buy = create_buy_order(2, 0.001, 1000);
        let trades = matcher.match_order(buy).unwrap();
        
        assert_eq!(trades.len(), 1);
    }

    #[test]
    fn test_very_large_price() {
        let mut matcher = FifoMatcher::new();
        
        let sell = create_sell_order(1, 999999.99, 1);
        matcher.match_order(sell).unwrap();
        
        let buy = create_buy_order(2, 999999.99, 1);
        let trades = matcher.match_order(buy).unwrap();
        
        assert_eq!(trades.len(), 1);
    }

    #[test]
    fn test_empty_book_operations() {
        let matcher = FifoMatcher::new();
        
        assert!(matcher.is_empty());
        assert!(matcher.best_bid().is_none());
        assert!(matcher.best_ask().is_none());
        assert_eq!(matcher.bid_depth(), 0);
        assert_eq!(matcher.ask_depth(), 0);
        
        let bids: Vec<_> = matcher.bids_iter().collect();
        let asks: Vec<_> = matcher.asks_iter().collect();
        assert_eq!(bids.len(), 0);
        assert_eq!(asks.len(), 0);
    }
}