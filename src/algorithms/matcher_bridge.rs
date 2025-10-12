//! Bridge module to integrate existing algorithm implementations with the shard-based system
//! 
//! This module converts between:
//! - `crate::types::Order` (uses u64 price) - Used in shard.rs
//! - `crate::engine::Order` (uses f64 price) - Used in algorithms
//! 
//! And provides unified interfaces for all matching algorithms.

use crate::types::{Order as ShardOrder, Trade as ShardTrade, Side as ShardSide, Event};
use crate::engine::{Order as EngineOrder, Side as EngineSide};
use crate::algorithms::fifo::{FifoMatcher, Trade as AlgoTrade};
use crate::algorithms::pro_rata::ProRataMatcher;
use crate::algorithms::hybrid::{HybridMatcher, HybridConfig};
use std::collections::VecDeque;

const PRICE_SCALE: f64 = 100.0; // Convert u64 to f64 price (e.g., 100 -> 1.00)

/// Convert ShardOrder (u64 price) to EngineOrder (f64 price)
fn shard_order_to_engine_order(order: &ShardOrder) -> EngineOrder {
    EngineOrder {
        id: order.order_id,
        side: match order.side {
            ShardSide::BUY => EngineSide::Buy,
            ShardSide::SELL => EngineSide::Sell,
        },
        price: order.price as f64 / PRICE_SCALE,
        quantity: order.qty,
        timestamp: order.timestamp,
    }
}

/// Convert EngineOrder (f64 price) back to ShardOrder (u64 price)
fn engine_order_to_shard_order(order: &EngineOrder) -> ShardOrder {
    ShardOrder {
        order_id: order.id,
        side: match order.side {
            EngineSide::Buy => ShardSide::BUY,
            EngineSide::Sell => ShardSide::SELL,
        },
        price: (order.price * PRICE_SCALE) as u64,
        qty: order.quantity,
        timestamp: order.timestamp,
    }
}

/// Convert algorithm Trade to ShardTrade
fn algo_trade_to_shard_trade(trade: &AlgoTrade) -> ShardTrade {
    ShardTrade::new(
        trade.buy_id,
        trade.sell_id,
        (trade.price * PRICE_SCALE) as u64,
        trade.quantity,
    )
}

/// FIFO Matcher Wrapper
pub struct FifoMatcherBridge {
    matcher: FifoMatcher,
}

impl FifoMatcherBridge {
    pub fn new() -> Self {
        Self {
            matcher: FifoMatcher::new(),
        }
    }

    /// Match an incoming order using FIFO algorithm
    pub fn match_order(&mut self, order: ShardOrder) -> Vec<ShardTrade> {
        let engine_order = shard_order_to_engine_order(&order);
        
        match self.matcher.match_order(engine_order) {
            Ok(trades) => trades.iter().map(algo_trade_to_shard_trade).collect(),
            Err(e) => {
                tracing::error!("FIFO matching error: {:?}", e);
                Vec::new()
            }
        }
    }

    /// Get current orderbook state
    pub fn get_orderbook_state(&self) -> (Vec<ShardOrder>, Vec<ShardOrder>) {
        let bids: Vec<ShardOrder> = self.matcher.bids.iter()
            .map(engine_order_to_shard_order)
            .collect();
        let asks: Vec<ShardOrder> = self.matcher.asks.iter()
            .map(engine_order_to_shard_order)
            .collect();
        (bids, asks)
    }

    pub fn bid_depth(&self) -> usize {
        self.matcher.bid_depth()
    }

    pub fn ask_depth(&self) -> usize {
        self.matcher.ask_depth()
    }
}

impl Default for FifoMatcherBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Pro-Rata Matcher Wrapper
pub struct ProRataMatcherBridge {
    matcher: ProRataMatcher,
}

impl ProRataMatcherBridge {
    pub fn new() -> Self {
        Self {
            matcher: ProRataMatcher::new(),
        }
    }

    /// Match an incoming order using Pro-Rata algorithm
    pub fn match_order(&mut self, order: ShardOrder) -> Vec<ShardTrade> {
        let engine_order = shard_order_to_engine_order(&order);
        let trades = self.matcher.match_order(engine_order);
        trades.iter().map(algo_trade_to_shard_trade).collect()
    }

    /// Get current orderbook state
    pub fn get_orderbook_state(&self) -> (Vec<ShardOrder>, Vec<ShardOrder>) {
        let bids: Vec<ShardOrder> = self.matcher.bids.iter()
            .map(engine_order_to_shard_order)
            .collect();
        let asks: Vec<ShardOrder> = self.matcher.asks.iter()
            .map(engine_order_to_shard_order)
            .collect();
        (bids, asks)
    }

    pub fn bid_depth(&self) -> usize {
        self.matcher.bid_depth()
    }

    pub fn ask_depth(&self) -> usize {
        self.matcher.ask_depth()
    }
}

impl Default for ProRataMatcherBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Hybrid Matcher Wrapper
pub struct HybridMatcherBridge {
    matcher: HybridMatcher,
}

impl HybridMatcherBridge {
    pub fn new() -> Self {
        Self {
            matcher: HybridMatcher::new(),
        }
    }

    pub fn new_with_config(fifo_percentage: f64) -> Self {
        Self {
            matcher: HybridMatcher::new_with_config(HybridConfig { fifo_percentage }),
        }
    }

    /// Match an incoming order using Hybrid algorithm
    pub fn match_order(&mut self, order: ShardOrder) -> Vec<ShardTrade> {
        let engine_order = shard_order_to_engine_order(&order);
        let trades = self.matcher.match_order(engine_order);
        trades.iter().map(algo_trade_to_shard_trade).collect()
    }

    /// Get current orderbook state
    pub fn get_orderbook_state(&self) -> (Vec<ShardOrder>, Vec<ShardOrder>) {
        let bids: Vec<ShardOrder> = self.matcher.bids.iter()
            .map(engine_order_to_shard_order)
            .collect();
        let asks: Vec<ShardOrder> = self.matcher.asks.iter()
            .map(engine_order_to_shard_order)
            .collect();
        (bids, asks)
    }

    pub fn bid_depth(&self) -> usize {
        self.matcher.bid_depth()
    }

    pub fn ask_depth(&self) -> usize {
        self.matcher.ask_depth()
    }
}

impl Default for HybridMatcherBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Hierarchical Matcher - Multi-phase matching strategy
/// Executes FIFO → Pro-Rata → Hybrid in sequence
pub struct HierarchicalMatcherBridge {
    matcher: crate::algorithms::hierarchical::HierarchicalMatcher,
}

impl HierarchicalMatcherBridge {
    pub fn new() -> Self {
        Self {
            matcher: crate::algorithms::hierarchical::HierarchicalMatcher::new(),
        }
    }

    /// Create with custom time step configuration
    pub fn new_with_config(fifo_pct: f64, pro_rata_pct: f64, hybrid_pct: f64) -> Self {
        let config = crate::algorithms::hierarchical::HierarchicalConfig {
            fifo_step_percentage: fifo_pct,
            pro_rata_step_percentage: pro_rata_pct,
            hybrid_step_percentage: hybrid_pct,
            hybrid_fifo_percentage: 0.5, // Default 50/50 split in hybrid phase
        };
        Self {
            matcher: crate::algorithms::hierarchical::HierarchicalMatcher::new_with_config(config),
        }
    }

    /// Match an incoming order using Hierarchical algorithm
    /// Executes in three time steps: FIFO → Pro-Rata → Hybrid
    pub fn match_order(&mut self, order: ShardOrder) -> Vec<ShardTrade> {
        let engine_order = shard_order_to_engine_order(&order);
        let trades = self.matcher.match_order(engine_order);
        trades.iter().map(algo_trade_to_shard_trade).collect()
    }

    /// Get current orderbook state (combined from all phases)
    pub fn get_orderbook_state(&self) -> (Vec<ShardOrder>, Vec<ShardOrder>) {
        let stats = self.matcher.get_phase_stats();
        // For simplicity, return combined depths
        // In real implementation, you'd combine actual orders from all matchers
        (vec![], vec![]) // Placeholder - actual implementation would merge orderbooks
    }

    pub fn bid_depth(&self) -> usize {
        self.matcher.bid_depth()
    }

    pub fn ask_depth(&self) -> usize {
        self.matcher.ask_depth()
    }
    
    /// Get detailed statistics about each phase
    pub fn get_phase_stats(&self) -> String {
        let stats = self.matcher.get_phase_stats();
        format!(
            "FIFO: {} bids/{} asks, ProRata: {} bids/{} asks, Hybrid: {} bids/{} asks",
            stats.fifo_bids, stats.fifo_asks,
            stats.pro_rata_bids, stats.pro_rata_asks,
            stats.hybrid_bids, stats.hybrid_asks
        )
    }
}

impl Default for HierarchicalMatcherBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_order_conversion() {
        let shard_order = ShardOrder {
            order_id: 1,
            side: ShardSide::BUY,
            price: 10050, // 100.50 * 100
            qty: 100,
            timestamp: Utc::now(),
        };

        let engine_order = shard_order_to_engine_order(&shard_order);
        assert_eq!(engine_order.id, 1);
        assert_eq!(engine_order.price, 100.5);
        assert_eq!(engine_order.quantity, 100);

        let back_to_shard = engine_order_to_shard_order(&engine_order);
        assert_eq!(back_to_shard.order_id, shard_order.order_id);
        assert_eq!(back_to_shard.price, shard_order.price);
        assert_eq!(back_to_shard.qty, shard_order.qty);
    }

    #[test]
    fn test_fifo_matcher_bridge() {
        let mut matcher = FifoMatcherBridge::new();
        
        // Add a sell order
        let sell_order = ShardOrder {
            order_id: 1,
            side: ShardSide::SELL,
            price: 10000,
            qty: 50,
            timestamp: Utc::now(),
        };
        
        let trades = matcher.match_order(sell_order);
        assert_eq!(trades.len(), 0); // No match, just added to book
        assert_eq!(matcher.ask_depth(), 1);
        
        // Add a matching buy order
        let buy_order = ShardOrder {
            order_id: 2,
            side: ShardSide::BUY,
            price: 10000,
            qty: 30,
            timestamp: Utc::now(),
        };
        
        let trades = matcher.match_order(buy_order);
        assert_eq!(trades.len(), 1); // Should match
        assert_eq!(trades[0].qty, 30);
        assert_eq!(matcher.ask_depth(), 1); // Remaining 20
    }
}
