use std::collections::VecDeque;
use crate::engine::{Order, Side, Request, OrderBookRef};
use crate::algorithms::fifo::{Trade, FifoMatcher};
use crate::algorithms::pro_rata::ProRataMatcher;
use crate::algorithms::hybrid::{HybridMatcher, HybridConfig};

/// Hierarchical Matching Configuration
/// Defines the sequence and time steps for each algorithm phase
#[allow(dead_code)]
pub struct HierarchicalConfig {
    /// Time step 1: FIFO matching percentage (0.0 to 1.0)
    pub fifo_step_percentage: f64,
    /// Time step 2: Pro-Rata matching percentage (0.0 to 1.0)
    pub pro_rata_step_percentage: f64,
    /// Time step 3: Hybrid matching percentage (0.0 to 1.0)
    pub hybrid_step_percentage: f64,
    /// Hybrid sub-config: FIFO percentage within hybrid phase
    pub hybrid_fifo_percentage: f64,
}

impl Default for HierarchicalConfig {
    fn default() -> Self {
        Self {
            fifo_step_percentage: 0.4,        // 40% of order goes to FIFO
            pro_rata_step_percentage: 0.3,    // 30% of order goes to Pro-Rata
            hybrid_step_percentage: 0.3,      // 30% of order goes to Hybrid
            hybrid_fifo_percentage: 0.5,      // Within hybrid: 50/50 FIFO/ProRata
        }
    }
}

/// Hierarchical Matcher
/// Executes matching in multiple phases with different algorithms
/// 
/// **Execution Flow:**
/// 1. **Time Step 1 (FIFO)**: Process X% of order with price-time priority
/// 2. **Time Step 2 (Pro-Rata)**: Process Y% of order with proportional allocation
/// 3. **Time Step 3 (Hybrid)**: Process Z% of order with mixed strategy
/// 
/// This provides:
/// - Initial fairness through FIFO
/// - Liquidity distribution through Pro-Rata
/// - Balanced completion through Hybrid
#[allow(dead_code)]
pub struct HierarchicalMatcher {
    pub fifo_matcher: FifoMatcher,
    pub pro_rata_matcher: ProRataMatcher,
    pub hybrid_matcher: HybridMatcher,
    pub config: HierarchicalConfig,
}

impl HierarchicalMatcher {
    pub fn new() -> Self {
        Self {
            fifo_matcher: FifoMatcher::new(),
            pro_rata_matcher: ProRataMatcher::new(),
            hybrid_matcher: HybridMatcher::new(),
            config: HierarchicalConfig::default(),
        }
    }

    #[allow(dead_code)]
    pub fn new_with_config(config: HierarchicalConfig) -> Self {
        let hybrid_config = HybridConfig {
            fifo_percentage: config.hybrid_fifo_percentage,
        };
        
        Self {
            fifo_matcher: FifoMatcher::new(),
            pro_rata_matcher: ProRataMatcher::new(),
            hybrid_matcher: HybridMatcher::new_with_config(hybrid_config),
            config,
        }
    }

    /// Main matching function using hierarchical strategy
    /// 
    /// # Algorithm Steps:
    /// 
    /// ## Time Step 1: FIFO Phase
    /// - Allocate `fifo_step_percentage` of incoming order quantity
    /// - Match using price-time priority
    /// - Ensures fairness for early orders
    /// 
    /// ## Time Step 2: Pro-Rata Phase
    /// - Allocate `pro_rata_step_percentage` of incoming order quantity
    /// - Match proportionally across resting orders at best price
    /// - Prevents gaming, distributes liquidity
    /// 
    /// ## Time Step 3: Hybrid Phase
    /// - Allocate `hybrid_step_percentage` of incoming order quantity
    /// - Match using configurable FIFO/Pro-Rata mix
    /// - Balances the benefits of both strategies
    /// 
    /// # Returns
    /// All trades from all three phases combined
    pub fn match_order(&mut self, mut incoming: Order) -> Vec<Trade> {
        if incoming.quantity == 0 || incoming.price <= 0.0 {
            return Vec::new();
        }

        // Validate config percentages sum to ~1.0
        let total_percentage = self.config.fifo_step_percentage 
            + self.config.pro_rata_step_percentage 
            + self.config.hybrid_step_percentage;
        
        if (total_percentage - 1.0).abs() > 0.01 {
            eprintln!(
                "Warning: Hierarchical config percentages sum to {:.2}, expected 1.0",
                total_percentage
            );
        }

        let mut all_trades = Vec::new();
        let original_quantity = incoming.quantity;

        // ===== TIME STEP 1: FIFO PHASE =====
        let fifo_quantity = (original_quantity as f64 * self.config.fifo_step_percentage).floor() as u64;
        if fifo_quantity > 0 {
            let mut fifo_order = incoming.clone();
            fifo_order.quantity = fifo_quantity;
            
            match self.fifo_matcher.match_order(fifo_order) {
                Ok(mut trades) => {
                    all_trades.append(&mut trades);
                }
                Err(e) => {
                    eprintln!("FIFO phase error in hierarchical matching: {:?}", e);
                }
            }
        }

        // ===== TIME STEP 2: PRO-RATA PHASE =====
        let pro_rata_quantity = (original_quantity as f64 * self.config.pro_rata_step_percentage).floor() as u64;
        if pro_rata_quantity > 0 {
            let mut pro_rata_order = incoming.clone();
            pro_rata_order.quantity = pro_rata_quantity;
            
            let mut trades = self.pro_rata_matcher.match_order(pro_rata_order);
            all_trades.append(&mut trades);
        }

        // ===== TIME STEP 3: HYBRID PHASE =====
        let hybrid_quantity = original_quantity - fifo_quantity - pro_rata_quantity;
        if hybrid_quantity > 0 {
            let mut hybrid_order = incoming.clone();
            hybrid_order.quantity = hybrid_quantity;
            
            let mut trades = self.hybrid_matcher.match_order(hybrid_order);
            all_trades.append(&mut trades);
        }

        all_trades
    }

    #[allow(dead_code)]
    pub fn best_bid(&self) -> Option<&Order> {
        // Return best bid from FIFO matcher (they share orderbook conceptually)
        self.fifo_matcher.best_bid()
    }

    #[allow(dead_code)]
    pub fn best_ask(&self) -> Option<&Order> {
        self.fifo_matcher.best_ask()
    }

    #[allow(dead_code)]
    pub fn bid_depth(&self) -> usize {
        self.fifo_matcher.bid_depth()
            + self.pro_rata_matcher.bid_depth()
            + self.hybrid_matcher.bid_depth()
    }

    #[allow(dead_code)]
    pub fn ask_depth(&self) -> usize {
        self.fifo_matcher.ask_depth()
            + self.pro_rata_matcher.ask_depth()
            + self.hybrid_matcher.ask_depth()
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.fifo_matcher.clear();
        self.pro_rata_matcher.clear();
        self.hybrid_matcher.clear();
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.fifo_matcher.is_empty()
            && self.pro_rata_matcher.is_empty()
            && self.hybrid_matcher.is_empty()
    }

    /// Get detailed statistics about each phase
    #[allow(dead_code)]
    pub fn get_phase_stats(&self) -> HierarchicalStats {
        HierarchicalStats {
            fifo_bids: self.fifo_matcher.bid_depth(),
            fifo_asks: self.fifo_matcher.ask_depth(),
            pro_rata_bids: self.pro_rata_matcher.bid_depth(),
            pro_rata_asks: self.pro_rata_matcher.ask_depth(),
            hybrid_bids: self.hybrid_matcher.bid_depth(),
            hybrid_asks: self.hybrid_matcher.ask_depth(),
        }
    }
}

/// Statistics for hierarchical matching phases
#[allow(dead_code)]
pub struct HierarchicalStats {
    pub fifo_bids: usize,
    pub fifo_asks: usize,
    pub pro_rata_bids: usize,
    pub pro_rata_asks: usize,
    pub hybrid_bids: usize,
    pub hybrid_asks: usize,
}

impl Default for HierarchicalMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Process a request using Hierarchical matching algorithm.
///
/// This is the concurrent-safe entry point for Hierarchical matching.
/// It uses the order book reference for validation only (read-only access).
///
/// # Arguments
///
/// * `request` - The request containing the order to process
/// * `order_book` - Read-only reference to the order book for validation
///
/// # Returns
///
/// A vector of executed trades from all three phases
pub fn process(request: Request, _order_book: &OrderBookRef) -> Vec<Trade> {
    // Create a new matcher instance for this request
    let mut matcher = HierarchicalMatcher::new();
    
    // Validate using order book reference (read-only)
    if !_order_book.validate_order(request.order.id, request.order.price, request.order.quantity) {
        eprintln!("Order validation failed for request {}", request.id);
        return Vec::new();
    }
    
    // Process the order through all three phases
    matcher.match_order(request.order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Side;
    use chrono::Utc;

    #[test]
    fn test_hierarchical_config_default() {
        let config = HierarchicalConfig::default();
        let total = config.fifo_step_percentage 
            + config.pro_rata_step_percentage 
            + config.hybrid_step_percentage;
        assert!((total - 1.0).abs() < 0.01, "Percentages should sum to 1.0");
    }

    #[test]
    fn test_hierarchical_matcher_creation() {
        let matcher = HierarchicalMatcher::new();
        assert!(matcher.is_empty());
    }

    #[test]
    fn test_hierarchical_custom_config() {
        let config = HierarchicalConfig {
            fifo_step_percentage: 0.5,
            pro_rata_step_percentage: 0.3,
            hybrid_step_percentage: 0.2,
            hybrid_fifo_percentage: 0.6,
        };
        
        let matcher = HierarchicalMatcher::new_with_config(config);
        assert_eq!(matcher.config.fifo_step_percentage, 0.5);
        assert_eq!(matcher.config.hybrid_fifo_percentage, 0.6);
    }

    #[test]
    fn test_hierarchical_matching_empty_book() {
        let mut matcher = HierarchicalMatcher::new();
        
        let order = Order::new(1, Side::Buy, 100.0, 100);
        let trades = matcher.match_order(order);
        
        assert_eq!(trades.len(), 0, "No trades with empty orderbook");
        assert!(!matcher.is_empty(), "Order should be added to book");
    }

    #[test]
    fn test_hierarchical_matching_with_resting_orders() {
        let mut matcher = HierarchicalMatcher::new();
        
        // Add resting sell orders to FIFO matcher
        let sell1 = Order::new(1, Side::Sell, 100.0, 50);
        let _ = matcher.fifo_matcher.match_order(sell1);
        
        // Add resting sell orders to Pro-Rata matcher
        let sell2 = Order::new(2, Side::Sell, 100.0, 30);
        let _ = matcher.pro_rata_matcher.match_order(sell2);
        
        // Add resting sell orders to Hybrid matcher
        let sell3 = Order::new(3, Side::Sell, 100.0, 20);
        let _ = matcher.hybrid_matcher.match_order(sell3);
        
        // Now submit a buy order - should match across all phases
        let buy = Order::new(4, Side::Buy, 100.0, 100);
        let trades = matcher.match_order(buy);
        
        assert!(!trades.is_empty(), "Should have some trades");
    }

    #[test]
    fn test_phase_stats() {
        let mut matcher = HierarchicalMatcher::new();
        
        // Add orders to different matchers
        let _ = matcher.fifo_matcher.match_order(Order::new(1, Side::Buy, 100.0, 50));
        let _ = matcher.pro_rata_matcher.match_order(Order::new(2, Side::Sell, 101.0, 30));
        
        let stats = matcher.get_phase_stats();
        assert_eq!(stats.fifo_bids, 1);
        assert_eq!(stats.pro_rata_asks, 1);
    }
}
