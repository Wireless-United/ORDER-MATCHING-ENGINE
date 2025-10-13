use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use chrono::{DateTime, Utc};
use crate::types::{EngineOrder as Order, EngineSide as Side, Request, OrderBookRef};
use crate::algorithms::errors::AlgorithmError;
use crate::algorithms::logger::{get_logger, OrderInfo};

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
        let logger = get_logger();
        let symbol = "UNKNOWN"; // Symbol should be passed from shard if needed

        match incoming.side {
            Side::Buy => {
                self.match_buy_order(&mut incoming, &mut trades)?;
                if !incoming.is_empty() {
                    // Log unmatched portion
                    logger.log_no_match(
                        "FIFO",
                        symbol,
                        OrderInfo {
                            order_id: incoming.id,
                            side: "BUY".to_string(),
                            price: incoming.price,
                            quantity: incoming.quantity,
                            timestamp: Utc::now(),
                            reason: "Partial fill - no matching sellers at this price".to_string(),
                        },
                    );
                    self.add_bid(incoming);
                }
            }
            Side::Sell => {
                self.match_sell_order(&mut incoming, &mut trades)?;
                if !incoming.is_empty() {
                    // Log unmatched portion
                    logger.log_no_match(
                        "FIFO",
                        symbol,
                        OrderInfo {
                            order_id: incoming.id,
                            side: "SELL".to_string(),
                            price: incoming.price,
                            quantity: incoming.quantity,
                            timestamp: Utc::now(),
                            reason: "Partial fill - no matching buyers at this price".to_string(),
                        },
                    );
                    self.add_ask(incoming);
                }
            }
        }

        // Log all matched trades
        for trade in &trades {
            logger.log_match("FIFO", symbol, trade.clone());
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
