use std::collections::VecDeque;
use crate::types::{EngineOrder as Order, EngineSide as Side, Request, OrderBookRef};
use crate::algorithms::fifo::Trade;

#[allow(dead_code)]
pub struct HybridConfig {
    pub fifo_percentage: f64,
}

impl Default for HybridConfig {
    fn default() -> Self {
        Self {
            fifo_percentage: 0.5,
        }
    }
}

#[allow(dead_code)]
pub struct HybridMatcher {
    pub bids: VecDeque<Order>,
    pub asks: VecDeque<Order>,
    pub config: HybridConfig,
}

impl HybridMatcher {
    pub fn new() -> Self {
        Self {
            bids: VecDeque::new(),
            asks: VecDeque::new(),
            config: HybridConfig::default(),
        }
    }

    #[allow(dead_code)]
    pub fn new_with_config(config: HybridConfig) -> Self {
        Self {
            bids: VecDeque::new(),
            asks: VecDeque::new(),
            config,
        }
    }

    pub fn match_order(&mut self, incoming: Order) -> Vec<Trade> {
        if incoming.quantity == 0 || incoming.price <= 0.0 {
            return Vec::new();
        }

        if self.config.fifo_percentage < 0.0 || self.config.fifo_percentage > 1.0 {
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

        let total_quantity = incoming_buy.quantity;
        let fifo_quantity = (total_quantity as f64 * self.config.fifo_percentage).floor() as u64;
        let pro_rata_quantity = total_quantity - fifo_quantity;

        // FIFO portion
        if fifo_quantity > 0 {
            let mut fifo_order = incoming_buy.clone();
            fifo_order.quantity = fifo_quantity;
            self.apply_fifo_matching_buy(&mut fifo_order, &mut trades, best_ask_price);
            incoming_buy.quantity -= fifo_quantity - fifo_order.quantity;
        }

        // Pro-rata portion
        if pro_rata_quantity > 0 && !self.asks.is_empty() {
            let mut pro_rata_order = incoming_buy.clone();
            pro_rata_order.quantity = pro_rata_quantity;
            self.apply_pro_rata_matching_buy(&mut pro_rata_order, &mut trades, best_ask_price);
            incoming_buy.quantity -= pro_rata_quantity - pro_rata_order.quantity;
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

        let total_quantity = incoming_sell.quantity;
        let fifo_quantity = (total_quantity as f64 * self.config.fifo_percentage).floor() as u64;
        let pro_rata_quantity = total_quantity - fifo_quantity;

        // FIFO portion
        if fifo_quantity > 0 {
            let mut fifo_order = incoming_sell.clone();
            fifo_order.quantity = fifo_quantity;
            self.apply_fifo_matching_sell(&mut fifo_order, &mut trades, best_bid_price);
            incoming_sell.quantity -= fifo_quantity - fifo_order.quantity;
        }

        // Pro-rata portion
        if pro_rata_quantity > 0 && !self.bids.is_empty() {
            let mut pro_rata_order = incoming_sell.clone();
            pro_rata_order.quantity = pro_rata_quantity;
            self.apply_pro_rata_matching_sell(&mut pro_rata_order, &mut trades, best_bid_price);
            incoming_sell.quantity -= pro_rata_quantity - pro_rata_order.quantity;
        }

        if incoming_sell.quantity > 0 {
            self.asks.push_back(incoming_sell);
        }

        trades
    }

    fn apply_fifo_matching_buy(&mut self, incoming_buy: &mut Order, trades: &mut Vec<Trade>, target_price: f64) {
        while !incoming_buy.is_empty() && !self.asks.is_empty() {
            let front_ask = self.asks.front().unwrap();
            if front_ask.price != target_price {
                break;
            }

            let mut ask_order = self.asks.pop_front().unwrap();
            let trade_quantity = std::cmp::min(incoming_buy.quantity, ask_order.quantity);

            let trade = Trade::new(
                incoming_buy.id,
                ask_order.id,
                ask_order.price,
                trade_quantity,
            );
            trades.push(trade);

            incoming_buy.quantity -= trade_quantity;
            ask_order.quantity -= trade_quantity;

            if ask_order.quantity > 0 {
                self.asks.push_front(ask_order);
            }
        }
    }

    fn apply_fifo_matching_sell(&mut self, incoming_sell: &mut Order, trades: &mut Vec<Trade>, target_price: f64) {
        while !incoming_sell.is_empty() && !self.bids.is_empty() {
            let front_bid = self.bids.front().unwrap();
            if front_bid.price != target_price {
                break;
            }

            let mut bid_order = self.bids.pop_front().unwrap();
            let trade_quantity = std::cmp::min(incoming_sell.quantity, bid_order.quantity);

            let trade = Trade::new(
                bid_order.id,
                incoming_sell.id,
                bid_order.price,
                trade_quantity,
            );
            trades.push(trade);

            incoming_sell.quantity -= trade_quantity;
            bid_order.quantity -= trade_quantity;

            if bid_order.quantity > 0 {
                self.bids.push_front(bid_order);
            }
        }
    }

    fn apply_pro_rata_matching_buy(&mut self, incoming_buy: &mut Order, trades: &mut Vec<Trade>, target_price: f64) {
        let mut matching_orders: Vec<(usize, u64)> = Vec::new();
        let mut total_resting_quantity = 0u64;

        for (index, ask) in self.asks.iter().enumerate() {
            if ask.price == target_price {
                matching_orders.push((index, ask.quantity));
                total_resting_quantity += ask.quantity;
            } else {
                break;
            }
        }

        if total_resting_quantity == 0 {
            return;
        }

        let available_quantity = std::cmp::min(incoming_buy.quantity, total_resting_quantity);
        let mut allocations: Vec<(usize, u64)> = Vec::new();
        let mut total_allocated = 0u64;

        for (index, resting_qty) in &matching_orders {
            let proportion = (*resting_qty as f64) / (total_resting_quantity as f64);
            let allocated = (proportion * available_quantity as f64).floor() as u64;
            allocations.push((*index, allocated));
            total_allocated += allocated;
        }

        let mut remainder = available_quantity - total_allocated;
        let mut allocation_idx = 0;
        while remainder > 0 && allocation_idx < allocations.len() {
            allocations[allocation_idx].1 += 1;
            remainder -= 1;
            allocation_idx += 1;
        }

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
    }

    fn apply_pro_rata_matching_sell(&mut self, incoming_sell: &mut Order, trades: &mut Vec<Trade>, target_price: f64) {
        let mut matching_orders: Vec<(usize, u64)> = Vec::new();
        let mut total_resting_quantity = 0u64;

        for (index, bid) in self.bids.iter().enumerate() {
            if bid.price == target_price {
                matching_orders.push((index, bid.quantity));
                total_resting_quantity += bid.quantity;
            } else {
                break;
            }
        }

        if total_resting_quantity == 0 {
            return;
        }

        let available_quantity = std::cmp::min(incoming_sell.quantity, total_resting_quantity);
        let mut allocations: Vec<(usize, u64)> = Vec::new();
        let mut total_allocated = 0u64;

        for (index, resting_qty) in &matching_orders {
            let proportion = (*resting_qty as f64) / (total_resting_quantity as f64);
            let allocated = (proportion * available_quantity as f64).floor() as u64;
            allocations.push((*index, allocated));
            total_allocated += allocated;
        }

        let mut remainder = available_quantity - total_allocated;
        let mut allocation_idx = 0;
        while remainder > 0 && allocation_idx < allocations.len() {
            allocations[allocation_idx].1 += 1;
            remainder -= 1;
            allocation_idx += 1;
        }

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
impl Default for HybridMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Process a request using Hybrid matching algorithm.
///
/// This is the concurrent-safe entry point for Hybrid matching.
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
    let mut matcher = HybridMatcher::new();
    
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
    // Configuration Tests
    // ========================================================================

    #[test]
    fn test_default_config() {
        let config = HybridConfig::default();
        assert_eq!(config.fifo_percentage, 0.5);
    }

    #[test]
    fn test_custom_config() {
        let config = HybridConfig {
            fifo_percentage: 0.7,
        };
        assert_eq!(config.fifo_percentage, 0.7);
    }

    // ========================================================================
    // Basic Functionality Tests
    // ========================================================================

    #[test]
    fn test_hybrid_matcher_creation() {
        let matcher = HybridMatcher::new();
        assert!(matcher.is_empty());
        assert_eq!(matcher.config.fifo_percentage, 0.5);
    }

    #[test]
    fn test_hybrid_matcher_with_config() {
        let config = HybridConfig {
            fifo_percentage: 0.6,
        };
        let matcher = HybridMatcher::new_with_config(config);
        assert_eq!(matcher.config.fifo_percentage, 0.6);
    }

    // ========================================================================
    // Validation Tests
    // ========================================================================

    #[test]
    fn test_reject_zero_quantity() {
        let mut matcher = HybridMatcher::new();
        let order = create_buy_order(1, 100.0, 0);
        
        let trades = matcher.match_order(order);
        assert_eq!(trades.len(), 0);
    }

    #[test]
    fn test_reject_invalid_config() {
        let config = HybridConfig {
            fifo_percentage: 1.5, // Invalid > 1.0
        };
        let mut matcher = HybridMatcher::new_with_config(config);
        
        let order = create_buy_order(1, 100.0, 100);
        let trades = matcher.match_order(order);
        assert_eq!(trades.len(), 0);
    }

    #[test]
    fn test_reject_negative_config() {
        let config = HybridConfig {
            fifo_percentage: -0.1,
        };
        let mut matcher = HybridMatcher::new_with_config(config);
        
        let order = create_buy_order(1, 100.0, 100);
        let trades = matcher.match_order(order);
        assert_eq!(trades.len(), 0);
    }

    // ========================================================================
    // Hybrid Matching Tests - 50/50 Split
    // ========================================================================

    #[test]
    fn test_hybrid_fifty_fifty_split() {
        let mut matcher = HybridMatcher::new(); // Default 50/50
        
        // Add resting sell orders
        matcher.match_order(create_sell_order(1, 100.0, 25));
        matcher.match_order(create_sell_order(2, 100.0, 25));
        matcher.match_order(create_sell_order(3, 100.0, 25));
        matcher.match_order(create_sell_order(4, 100.0, 25));
        
        // Buy 100: 50 FIFO, 50 Pro-Rata
        let buy = create_buy_order(5, 100.0, 100);
        let trades = matcher.match_order(buy);
        
        // Should execute trades
        assert!(!trades.is_empty());
        let total: u64 = trades.iter().map(|t| t.quantity).sum();
        assert_eq!(total, 100);
    }

    #[test]
    fn test_hybrid_mostly_fifo() {
        let config = HybridConfig {
            fifo_percentage: 0.8, // 80% FIFO, 20% Pro-Rata
        };
        let mut matcher = HybridMatcher::new_with_config(config);
        
        matcher.match_order(create_sell_order(1, 100.0, 50));
        matcher.match_order(create_sell_order(2, 100.0, 50));
        
        let buy = create_buy_order(3, 100.0, 100);
        let trades = matcher.match_order(buy);
        
        assert!(!trades.is_empty());
        let total: u64 = trades.iter().map(|t| t.quantity).sum();
        assert_eq!(total, 100);
    }

    #[test]
    fn test_hybrid_mostly_pro_rata() {
        let config = HybridConfig {
            fifo_percentage: 0.2, // 20% FIFO, 80% Pro-Rata
        };
        let mut matcher = HybridMatcher::new_with_config(config);
        
        matcher.match_order(create_sell_order(1, 100.0, 50));
        matcher.match_order(create_sell_order(2, 100.0, 50));
        
        let buy = create_buy_order(3, 100.0, 100);
        let trades = matcher.match_order(buy);
        
        assert!(!trades.is_empty());
        let total: u64 = trades.iter().map(|t| t.quantity).sum();
        assert_eq!(total, 100);
    }

    // ========================================================================
    // FIFO Phase Tests
    // ========================================================================

    #[test]
    fn test_fifo_phase_order_priority() {
        let mut matcher = HybridMatcher::new();
        
        // Add orders in specific order
        matcher.match_order(create_sell_order(1, 100.0, 100));
        matcher.match_order(create_sell_order(2, 100.0, 100));
        
        let buy = create_buy_order(3, 100.0, 100);
        let trades = matcher.match_order(buy);
        
        // FIFO phase (50%) should match first order
        assert!(!trades.is_empty());
        assert!(trades.iter().any(|t| t.sell_id == 1));
    }

    // ========================================================================
    // Price Priority Tests
    // ========================================================================

    #[test]
    fn test_no_match_price_spread() {
        let mut matcher = HybridMatcher::new();
        
        let sell = create_sell_order(1, 105.0, 50);
        matcher.match_order(sell);
        
        let buy = create_buy_order(2, 100.0, 50);
        let trades = matcher.match_order(buy);
        
        assert_eq!(trades.len(), 0);
        assert_eq!(matcher.bid_depth(), 1);
        assert_eq!(matcher.ask_depth(), 1);
    }

    #[test]
    fn test_match_at_best_price_only() {
        let mut matcher = HybridMatcher::new();
        
        matcher.match_order(create_sell_order(1, 100.0, 50));
        matcher.match_order(create_sell_order(2, 101.0, 50));
        
        let buy = create_buy_order(3, 105.0, 100);
        let trades = matcher.match_order(buy);
        
        // Should only match at 100.0 price level
        for trade in &trades {
            assert_eq!(trade.price, 100.0);
        }
    }

    // ========================================================================
    // Partial Fill Tests
    // ========================================================================

    #[test]
    fn test_partial_fill_with_remainder() {
        let mut matcher = HybridMatcher::new();
        
        matcher.match_order(create_sell_order(1, 100.0, 30));
        
        let buy = create_buy_order(2, 100.0, 100);
        let trades = matcher.match_order(buy);
        
        assert!(!trades.is_empty());
        let total: u64 = trades.iter().map(|t| t.quantity).sum();
        assert_eq!(total, 30);
        
        // Remainder should be in book
        assert_eq!(matcher.bid_depth(), 1);
    }

    // ========================================================================
    // Empty Book Tests
    // ========================================================================

    #[test]
    fn test_empty_book_add_order() {
        let mut matcher = HybridMatcher::new();
        
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
        let mut matcher = HybridMatcher::new();
        
        assert!(matcher.best_bid().is_none());
        
        matcher.match_order(create_buy_order(1, 100.0, 50));
        assert!(matcher.best_bid().is_some());
    }

    #[test]
    fn test_best_ask() {
        let mut matcher = HybridMatcher::new();
        
        assert!(matcher.best_ask().is_none());
        
        matcher.match_order(create_sell_order(1, 100.0, 50));
        assert!(matcher.best_ask().is_some());
    }

    // ========================================================================
    // Clear Tests
    // ========================================================================

    #[test]
    fn test_clear() {
        let mut matcher = HybridMatcher::new();
        
        matcher.match_order(create_buy_order(1, 100.0, 50));
        matcher.match_order(create_sell_order(2, 105.0, 50));
        
        matcher.clear();
        
        assert!(matcher.is_empty());
    }

    // ========================================================================
    // Iterator Tests
    // ========================================================================

    #[test]
    fn test_bids_iterator() {
        let mut matcher = HybridMatcher::new();
        
        matcher.match_order(create_buy_order(1, 100.0, 10));
        matcher.match_order(create_buy_order(2, 101.0, 20));
        
        let bids: Vec<_> = matcher.bids_iter().collect();
        assert_eq!(bids.len(), 2);
    }

    #[test]
    fn test_asks_iterator() {
        let mut matcher = HybridMatcher::new();
        
        matcher.match_order(create_sell_order(1, 100.0, 10));
        matcher.match_order(create_sell_order(2, 101.0, 20));
        
        let asks: Vec<_> = matcher.asks_iter().collect();
        assert_eq!(asks.len(), 2);
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
    // Edge Cases
    // ========================================================================

    #[test]
    fn test_zero_fifo_percentage() {
        let config = HybridConfig {
            fifo_percentage: 0.0, // 100% Pro-Rata
        };
        let mut matcher = HybridMatcher::new_with_config(config);
        
        matcher.match_order(create_sell_order(1, 100.0, 50));
        matcher.match_order(create_sell_order(2, 100.0, 50));
        
        let buy = create_buy_order(3, 100.0, 100);
        let trades = matcher.match_order(buy);
        
        assert!(!trades.is_empty());
        let total: u64 = trades.iter().map(|t| t.quantity).sum();
        assert_eq!(total, 100);
    }

    #[test]
    fn test_full_fifo_percentage() {
        let config = HybridConfig {
            fifo_percentage: 1.0, // 100% FIFO
        };
        let mut matcher = HybridMatcher::new_with_config(config);
        
        matcher.match_order(create_sell_order(1, 100.0, 50));
        matcher.match_order(create_sell_order(2, 100.0, 50));
        
        let buy = create_buy_order(3, 100.0, 100);
        let trades = matcher.match_order(buy);
        
        assert!(!trades.is_empty());
        let total: u64 = trades.iter().map(|t| t.quantity).sum();
        assert_eq!(total, 100);
    }

    #[test]
    fn test_odd_quantity_split() {
        let mut matcher = HybridMatcher::new(); // 50/50
        
        matcher.match_order(create_sell_order(1, 100.0, 100));
        
        let buy = create_buy_order(2, 100.0, 99);
        let trades = matcher.match_order(buy);
        
        assert!(!trades.is_empty());
        let total: u64 = trades.iter().map(|t| t.quantity).sum();
        assert_eq!(total, 99);
    }

    #[test]
    fn test_sell_side_matching() {
        let mut matcher = HybridMatcher::new();
        
        matcher.match_order(create_buy_order(1, 100.0, 50));
        matcher.match_order(create_buy_order(2, 100.0, 50));
        
        let sell = create_sell_order(3, 100.0, 100);
        let trades = matcher.match_order(sell);
        
        assert!(!trades.is_empty());
        let total: u64 = trades.iter().map(|t| t.quantity).sum();
        assert_eq!(total, 100);
    }
}