use std::collections::VecDeque;
use crate::types::{EngineOrder as Order, EngineSide as Side, Request, OrderBookRef};
use crate::algorithms::fifo::Trade;

#[allow(dead_code)]
pub struct ProRataMatcher {
    pub bids: VecDeque<Order>,
    pub asks: VecDeque<Order>,
}

impl ProRataMatcher {
    pub fn new() -> Self {
        Self {
            bids: VecDeque::new(),
            asks: VecDeque::new(),
        }
    }

    pub fn match_order(&mut self, incoming: Order) -> Vec<Trade> {
        if incoming.quantity == 0 || incoming.price <= 0.0 {
            return Vec::new();
        }

        match incoming.side {
            Side::Buy => self.match_buy_order(incoming),
            Side::Sell => self.match_sell_order(incoming),
        }
    }

    fn match_buy_order(&mut self, mut incoming_buy: Order) -> Vec<Trade> {
        let mut trades = Vec::new();

        if self.asks.is_empty() {
            self.bids.push_back(incoming_buy);
            return trades;
        }

        let best_ask_price = self.asks.front().unwrap().price;
        if incoming_buy.price < best_ask_price {
            self.bids.push_back(incoming_buy);
            return trades;
        }

        // Find all orders at the best price level
        let mut matching_orders: Vec<(usize, u64)> = Vec::new();
        let mut total_resting_quantity = 0u64;

        for (index, ask) in self.asks.iter().enumerate() {
            if ask.price == best_ask_price {
                matching_orders.push((index, ask.quantity));
                total_resting_quantity += ask.quantity;
            } else {
                break;
            }
        }

        if total_resting_quantity == 0 {
            self.bids.push_back(incoming_buy);
            return trades;
        }

        // Calculate proportional allocations
        let available_quantity = std::cmp::min(incoming_buy.quantity, total_resting_quantity);
        let mut allocations: Vec<(usize, u64)> = Vec::new();
        let mut total_allocated = 0u64;

        for (index, resting_qty) in &matching_orders {
            let proportion = (*resting_qty as f64) / (total_resting_quantity as f64);
            let allocated = (proportion * available_quantity as f64).floor() as u64;
            allocations.push((*index, allocated));
            total_allocated += allocated;
        }

        // Handle remainder
        let mut remainder = available_quantity - total_allocated;
        let mut allocation_idx = 0;
        while remainder > 0 && allocation_idx < allocations.len() {
            allocations[allocation_idx].1 += 1;
            remainder -= 1;
            allocation_idx += 1;
        }

        // Execute trades
        allocations.sort_by(|a, b| b.0.cmp(&a.0));
        for (index, allocated_qty) in allocations {
            if allocated_qty > 0 {
                let mut ask_order = self.asks.remove(index).unwrap();
                let trade = Trade::new(
                    incoming_buy.id,
                    ask_order.id,
                    ask_order.price,
                    allocated_qty,
                );
                trades.push(trade);
                incoming_buy.quantity -= allocated_qty;
                ask_order.quantity -= allocated_qty;
                if ask_order.quantity > 0 {
                    self.asks.insert(index, ask_order);
                }
            }
        }

        if incoming_buy.quantity > 0 {
            self.bids.push_back(incoming_buy);
        }

        trades
    }

    fn match_sell_order(&mut self, mut incoming_sell: Order) -> Vec<Trade> {
        let mut trades = Vec::new();

        if self.bids.is_empty() {
            self.asks.push_back(incoming_sell);
            return trades;
        }

        let best_bid_price = self.bids.front().unwrap().price;
        if incoming_sell.price > best_bid_price {
            self.asks.push_back(incoming_sell);
            return trades;
        }

        // Find all orders at the best price level
        let mut matching_orders: Vec<(usize, u64)> = Vec::new();
        let mut total_resting_quantity = 0u64;

        for (index, bid) in self.bids.iter().enumerate() {
            if bid.price == best_bid_price {
                matching_orders.push((index, bid.quantity));
                total_resting_quantity += bid.quantity;
            } else {
                break;
            }
        }

        if total_resting_quantity == 0 {
            self.asks.push_back(incoming_sell);
            return trades;
        }

        // Calculate proportional allocations
        let available_quantity = std::cmp::min(incoming_sell.quantity, total_resting_quantity);
        let mut allocations: Vec<(usize, u64)> = Vec::new();
        let mut total_allocated = 0u64;

        for (index, resting_qty) in &matching_orders {
            let proportion = (*resting_qty as f64) / (total_resting_quantity as f64);
            let allocated = (proportion * available_quantity as f64).floor() as u64;
            allocations.push((*index, allocated));
            total_allocated += allocated;
        }

        // Handle remainder
        let mut remainder = available_quantity - total_allocated;
        let mut allocation_idx = 0;
        while remainder > 0 && allocation_idx < allocations.len() {
            allocations[allocation_idx].1 += 1;
            remainder -= 1;
            allocation_idx += 1;
        }

        // Execute trades
        allocations.sort_by(|a, b| b.0.cmp(&a.0));
        for (index, allocated_qty) in allocations {
            if allocated_qty > 0 {
                let mut bid_order = self.bids.remove(index).unwrap();
                let trade = Trade::new(
                    bid_order.id,
                    incoming_sell.id,
                    bid_order.price,
                    allocated_qty,
                );
                trades.push(trade);
                incoming_sell.quantity -= allocated_qty;
                bid_order.quantity -= allocated_qty;
                if bid_order.quantity > 0 {
                    self.bids.insert(index, bid_order);
                }
            }
        }

        if incoming_sell.quantity > 0 {
            self.asks.push_back(incoming_sell);
        }

        trades
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
}

#[allow(dead_code)]
impl Default for ProRataMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Process a request using Pro-Rata matching algorithm.
///
/// This is the concurrent-safe entry point for Pro-Rata matching.
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
    let mut matcher = ProRataMatcher::new();
    
    // Validate using order book reference (read-only)
    if !_order_book.validate_order(request.order.id, request.order.price, request.order.quantity) {
        eprintln!("Order validation failed for request {}", request.id);
        return Vec::new();
    }
    
    // Process the order
    matcher.match_order(request.order)
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
    fn test_pro_rata_matcher_creation() {
        let matcher = ProRataMatcher::new();
        assert!(matcher.is_empty());
        assert_eq!(matcher.bid_depth(), 0);
        assert_eq!(matcher.ask_depth(), 0);
    }

    #[test]
    fn test_add_single_bid() {
        let mut matcher = ProRataMatcher::new();
        let order = create_buy_order(1, 100.0, 50);
        
        let trades = matcher.match_order(order);
        assert_eq!(trades.len(), 0);
        assert_eq!(matcher.bid_depth(), 1);
    }

    #[test]
    fn test_add_single_ask() {
        let mut matcher = ProRataMatcher::new();
        let order = create_sell_order(1, 100.0, 50);
        
        let trades = matcher.match_order(order);
        assert_eq!(trades.len(), 0);
        assert_eq!(matcher.ask_depth(), 1);
    }

    // ========================================================================
    // Validation Tests
    // ========================================================================

    #[test]
    fn test_reject_zero_quantity() {
        let mut matcher = ProRataMatcher::new();
        let order = create_buy_order(1, 100.0, 0);
        
        let trades = matcher.match_order(order);
        assert_eq!(trades.len(), 0);
    }

    #[test]
    fn test_reject_zero_price() {
        let mut matcher = ProRataMatcher::new();
        let order = create_buy_order(1, 0.0, 50);
        
        let trades = matcher.match_order(order);
        assert_eq!(trades.len(), 0);
    }

    // ========================================================================
    // Pro-Rata Allocation Tests
    // ========================================================================

    #[test]
    fn test_pro_rata_equal_distribution() {
        let mut matcher = ProRataMatcher::new();
        
        // Add three equal sell orders at same price
        matcher.match_order(create_sell_order(1, 100.0, 30));
        matcher.match_order(create_sell_order(2, 100.0, 30));
        matcher.match_order(create_sell_order(3, 100.0, 30));
        
        // Buy order that matches all three
        let buy = create_buy_order(4, 100.0, 90);
        let trades = matcher.match_order(buy);
        
        // Should distribute equally: 30 each
        assert_eq!(trades.len(), 3);
        assert_eq!(trades[0].quantity, 30);
        assert_eq!(trades[1].quantity, 30);
        assert_eq!(trades[2].quantity, 30);
        assert!(matcher.is_empty());
    }

    #[test]
    fn test_pro_rata_proportional_distribution() {
        let mut matcher = ProRataMatcher::new();
        
        // Add sell orders with different sizes at same price
        matcher.match_order(create_sell_order(1, 100.0, 50)); // 50%
        matcher.match_order(create_sell_order(2, 100.0, 30)); // 30%
        matcher.match_order(create_sell_order(3, 100.0, 20)); // 20%
        // Total: 100
        
        let buy = create_buy_order(4, 100.0, 100);
        let trades = matcher.match_order(buy);
        
        assert_eq!(trades.len(), 3);
        // Check proportional allocation (50%, 30%, 20%)
        assert_eq!(trades[0].quantity, 50);
        assert_eq!(trades[1].quantity, 30);
        assert_eq!(trades[2].quantity, 20);
    }

    #[test]
    fn test_pro_rata_with_remainder() {
        let mut matcher = ProRataMatcher::new();
        
        // Add three equal orders
        matcher.match_order(create_sell_order(1, 100.0, 10));
        matcher.match_order(create_sell_order(2, 100.0, 10));
        matcher.match_order(create_sell_order(3, 100.0, 10));
        
        // Buy order that doesn't divide evenly (25 / 3 = 8.33...)
        let buy = create_buy_order(4, 100.0, 25);
        let trades = matcher.match_order(buy);
        
        assert_eq!(trades.len(), 3);
        // Floor allocation: 8 each = 24, remainder 1 goes to first
        let total: u64 = trades.iter().map(|t| t.quantity).sum();
        assert_eq!(total, 25);
    }

    #[test]
    fn test_pro_rata_partial_fill() {
        let mut matcher = ProRataMatcher::new();
        
        // Large resting orders
        matcher.match_order(create_sell_order(1, 100.0, 100));
        matcher.match_order(create_sell_order(2, 100.0, 100));
        
        // Small incoming order
        let buy = create_buy_order(3, 100.0, 50);
        let trades = matcher.match_order(buy);
        
        assert_eq!(trades.len(), 2);
        assert_eq!(trades[0].quantity, 25);
        assert_eq!(trades[1].quantity, 25);
        
        // Remaining orders should still be in book
        assert_eq!(matcher.ask_depth(), 2);
    }

    // ========================================================================
    // Buy Side Tests
    // ========================================================================

    #[test]
    fn test_pro_rata_buy_side() {
        let mut matcher = ProRataMatcher::new();
        
        // Add buy orders
        matcher.match_order(create_buy_order(1, 100.0, 50));
        matcher.match_order(create_buy_order(2, 100.0, 30));
        matcher.match_order(create_buy_order(3, 100.0, 20));
        
        let sell = create_sell_order(4, 100.0, 100);
        let trades = matcher.match_order(sell);
        
        assert_eq!(trades.len(), 3);
        assert_eq!(trades[0].quantity, 50);
        assert_eq!(trades[1].quantity, 30);
        assert_eq!(trades[2].quantity, 20);
    }

    // ========================================================================
    // Price Priority Tests
    // ========================================================================

    #[test]
    fn test_no_match_price_too_low() {
        let mut matcher = ProRataMatcher::new();
        
        let sell = create_sell_order(1, 105.0, 50);
        matcher.match_order(sell);
        
        let buy = create_buy_order(2, 100.0, 50);
        let trades = matcher.match_order(buy);
        
        assert_eq!(trades.len(), 0);
        assert_eq!(matcher.bid_depth(), 1);
        assert_eq!(matcher.ask_depth(), 1);
    }

    #[test]
    fn test_match_only_at_best_price() {
        let mut matcher = ProRataMatcher::new();
        
        // Add orders at different prices
        matcher.match_order(create_sell_order(1, 100.0, 50));
        matcher.match_order(create_sell_order(2, 101.0, 50));
        matcher.match_order(create_sell_order(3, 102.0, 50));
        
        let buy = create_buy_order(4, 105.0, 100);
        let trades = matcher.match_order(buy);
        
        // Should only match at best price (100.0)
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].price, 100.0);
        assert_eq!(trades[0].quantity, 50);
    }

    // ========================================================================
    // Edge Cases
    // ========================================================================

    #[test]
    fn test_single_resting_order_pro_rata() {
        let mut matcher = ProRataMatcher::new();
        
        matcher.match_order(create_sell_order(1, 100.0, 50));
        
        let buy = create_buy_order(2, 100.0, 30);
        let trades = matcher.match_order(buy);
        
        // With single order, behaves like FIFO
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, 30);
        assert_eq!(matcher.ask_depth(), 1);
    }

    #[test]
    fn test_exact_match_multiple_orders() {
        let mut matcher = ProRataMatcher::new();
        
        matcher.match_order(create_sell_order(1, 100.0, 40));
        matcher.match_order(create_sell_order(2, 100.0, 60));
        
        let buy = create_buy_order(3, 100.0, 100);
        let trades = matcher.match_order(buy);
        
        assert_eq!(trades.len(), 2);
        assert_eq!(trades[0].quantity, 40);
        assert_eq!(trades[1].quantity, 60);
        assert!(matcher.is_empty());
    }

    #[test]
    fn test_empty_book() {
        let mut matcher = ProRataMatcher::new();
        
        let buy = create_buy_order(1, 100.0, 50);
        let trades = matcher.match_order(buy);
        
        assert_eq!(trades.len(), 0);
        assert_eq!(matcher.bid_depth(), 1);
    }

    // ========================================================================
    // Best Bid/Ask Tests
    // ========================================================================

    #[test]
    fn test_best_bid() {
        let mut matcher = ProRataMatcher::new();
        
        assert!(matcher.best_bid().is_none());
        
        matcher.match_order(create_buy_order(1, 100.0, 50));
        assert_eq!(matcher.best_bid().unwrap().id, 1);
    }

    #[test]
    fn test_best_ask() {
        let mut matcher = ProRataMatcher::new();
        
        assert!(matcher.best_ask().is_none());
        
        matcher.match_order(create_sell_order(1, 100.0, 50));
        assert_eq!(matcher.best_ask().unwrap().id, 1);
    }

    // ========================================================================
    // Iterator Tests
    // ========================================================================

    #[test]
    fn test_bids_iterator() {
        let mut matcher = ProRataMatcher::new();
        
        matcher.match_order(create_buy_order(1, 100.0, 10));
        matcher.match_order(create_buy_order(2, 101.0, 20));
        
        let bids: Vec<_> = matcher.bids_iter().collect();
        assert_eq!(bids.len(), 2);
    }

    #[test]
    fn test_asks_iterator() {
        let mut matcher = ProRataMatcher::new();
        
        matcher.match_order(create_sell_order(1, 100.0, 10));
        matcher.match_order(create_sell_order(2, 101.0, 20));
        
        let asks: Vec<_> = matcher.asks_iter().collect();
        assert_eq!(asks.len(), 2);
    }

    // ========================================================================
    // Clear Tests
    // ========================================================================

    #[test]
    fn test_clear() {
        let mut matcher = ProRataMatcher::new();
        
        matcher.match_order(create_buy_order(1, 100.0, 50));
        matcher.match_order(create_sell_order(2, 105.0, 50));
        
        matcher.clear();
        
        assert!(matcher.is_empty());
        assert_eq!(matcher.bid_depth(), 0);
        assert_eq!(matcher.ask_depth(), 0);
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
        assert_eq!(trades.len(), 0);
    }

    // ========================================================================
    // Complex Allocation Tests
    // ========================================================================

    #[test]
    fn test_pro_rata_small_incoming_large_resting() {
        let mut matcher = ProRataMatcher::new();
        
        // Large resting orders at same price
        matcher.match_order(create_sell_order(1, 100.0, 1000));
        matcher.match_order(create_sell_order(2, 100.0, 2000));
        matcher.match_order(create_sell_order(3, 100.0, 3000));
        
        // Small incoming order
        let buy = create_buy_order(4, 100.0, 60);
        let trades = matcher.match_order(buy);
        
        assert_eq!(trades.len(), 3);
        let total: u64 = trades.iter().map(|t| t.quantity).sum();
        assert_eq!(total, 60);
        
        // Check approximate proportions (1:2:3 ratio)
        // With rounding, should be roughly 10, 20, 30
        assert!(trades[0].quantity <= 11);
        assert!(trades[1].quantity <= 21);
        assert!(trades[2].quantity >= 28);
    }

    #[test]
    fn test_pro_rata_all_remainder_distribution() {
        let mut matcher = ProRataMatcher::new();
        
        // Orders that will cause maximum remainder
        matcher.match_order(create_sell_order(1, 100.0, 1));
        matcher.match_order(create_sell_order(2, 100.0, 1));
        matcher.match_order(create_sell_order(3, 100.0, 1));
        
        let buy = create_buy_order(4, 100.0, 3);
        let trades = matcher.match_order(buy);
        
        assert_eq!(trades.len(), 3);
        assert_eq!(trades[0].quantity, 1);
        assert_eq!(trades[1].quantity, 1);
        assert_eq!(trades[2].quantity, 1);
    }
}