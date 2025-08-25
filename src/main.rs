//! High-Performance Order Matching Engine
//! 
//! This is a demonstration of a FIFO (First-In-First-Out) order matching engine
//! implemented in Rust for high-frequency trading applications.
//! 
//! # Features
//! 
//! - **FIFO Matching**: Orders are matched based on time priority
//! - **Partial Fills**: Orders can be partially executed across multiple trades
//! - **Error Handling**: Comprehensive validation and error reporting
//! - **Performance**: Lock-free design optimized for low latency
//! 
//! # Architecture
//! 
//! ```text
//! ┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
//! │   Order Input   │───▶│   FifoMatcher    │───▶│   Trade Output  │
//! │                 │    │                  │    │                 │
//! │ • Buy/Sell      │    │ • Bid Queue      │    │ • Trade ID      │
//! │ • Price         │    │ • Ask Queue      │    │ • Price         │
//! │ • Quantity      │    │ • Matching Logic │    │ • Quantity      │
//! │ • Timestamp     │    │ • Validation     │    │ • Timestamp     │
//! └─────────────────┘    └──────────────────┘    └─────────────────┘
//! ```

mod algorithms;
mod engine;
mod utils;

use algorithms::{FifoMatcher, AlgorithmError};
use engine::{Order, Side};

fn main() -> Result<(), AlgorithmError> {
    println!("🚀 High-Performance Order Matching Engine - FIFO Algorithm Demo");
    println!("================================================================\n");
    
    let mut matcher = FifoMatcher::new();
    
    // Demonstration 1: Basic Order Matching
    println!("📈 Demo 1: Basic Order Matching");
    println!("-------------------------------");
    
    let sell_order = Order::new(1, Side::Sell, 100.0, 50);
    println!("➕ Adding SELL order: ID={}, Price=${:.2}, Qty={}", 
             sell_order.id, sell_order.price, sell_order.quantity);
    let trades = matcher.match_order(sell_order)?;
    println!("✅ Trades executed: {} (no matching buy orders yet)", trades.len());
    
    let buy_order = Order::new(2, Side::Buy, 100.0, 30);
    println!("➕ Adding BUY order:  ID={}, Price=${:.2}, Qty={}", 
             buy_order.id, buy_order.price, buy_order.quantity);
    let trades = matcher.match_order(buy_order)?;
    println!("✅ Trades executed: {}", trades.len());
    
    for (i, trade) in trades.iter().enumerate() {
        println!("   🤝 Trade {}: Buy ID={}, Sell ID={}, Price=${:.2}, Qty={}", 
                 i + 1, trade.buy_id, trade.sell_id, trade.price, trade.quantity);
    }
    
    println!("\n📊 Current Order Book State:");
    println!("   Best Bid: {:?}", 
             matcher.best_bid().map(|o| format!("${:.2} × {}", o.price, o.quantity)));
    println!("   Best Ask: {:?}", 
             matcher.best_ask().map(|o| format!("${:.2} × {}", o.price, o.quantity)));
    
    // Demonstration 2: Partial Fills
    println!("\n📈 Demo 2: Partial Fill Scenario");
    println!("--------------------------------");
    
    matcher.clear(); // Reset for clean demo
    
    // Large sell order
    let large_sell = Order::new(3, Side::Sell, 99.50, 200);
    println!("➕ Adding large SELL order: ID={}, Price=${:.2}, Qty={}", 
             large_sell.id, large_sell.price, large_sell.quantity);
    matcher.match_order(large_sell)?;
    
    // Multiple small buy orders
    let small_buys = vec![
        Order::new(4, Side::Buy, 99.50, 50),
        Order::new(5, Side::Buy, 99.50, 75),
        Order::new(6, Side::Buy, 99.50, 40),
    ];
    
    for buy in small_buys {
        println!("➕ Adding BUY order: ID={}, Price=${:.2}, Qty={}", 
                 buy.id, buy.price, buy.quantity);
        let trades = matcher.match_order(buy)?;
        
        for trade in &trades {
            println!("   🤝 Trade: Buy ID={}, Sell ID={}, Price=${:.2}, Qty={}, Rank={}", 
                     trade.buy_id, trade.sell_id, trade.price, trade.quantity, trade.rank);
        }
    }
    
    // Demonstration 3: FIFO Priority
    println!("\n📈 Demo 3: FIFO Time Priority");
    println!("-----------------------------");
    
    matcher.clear();
    
    // Add multiple sell orders at same price
    let fifo_sells = vec![
        Order::new(7, Side::Sell, 101.0, 30),
        Order::new(8, Side::Sell, 101.0, 40),
        Order::new(9, Side::Sell, 101.0, 25),
    ];
    
    for sell in fifo_sells {
        println!("➕ Adding SELL order: ID={}, Price=${:.2}, Qty={} (timestamp priority)", 
                 sell.id, sell.price, sell.quantity);
        matcher.match_order(sell)?;
    }
    
    // Buy order that matches partially with multiple sells
    let fifo_buy = Order::new(10, Side::Buy, 101.0, 80);
    println!("➕ Adding BUY order: ID={}, Price=${:.2}, Qty={}", 
             fifo_buy.id, fifo_buy.price, fifo_buy.quantity);
    let trades = matcher.match_order(fifo_buy)?;
    
    println!("✅ FIFO matching results:");
    for (i, trade) in trades.iter().enumerate() {
        println!("   🤝 Trade {}: Sell ID={} (first in queue), Qty={}, Rank={}", 
                 i + 1, trade.sell_id, trade.quantity, trade.rank);
    }
    
    // Final Statistics
    println!("\n📊 Final Order Book Statistics");
    println!("==============================");
    println!("📋 Bid Depth: {} orders", matcher.bid_depth());
    println!("📋 Ask Depth: {} orders", matcher.ask_depth());
    println!("📊 Best Bid: {:?}", 
             matcher.best_bid().map(|o| format!("${:.2} × {}", o.price, o.quantity)));
    println!("📊 Best Ask: {:?}", 
             matcher.best_ask().map(|o| format!("${:.2} × {}", o.price, o.quantity)));
    
    if !matcher.is_empty() {
        println!("\n🔍 Remaining Orders:");
        println!("   Bids:");
        for (i, order) in matcher.bids_iter().enumerate() {
            println!("     {}. ID={}, Price=${:.2}, Qty={}", 
                     i + 1, order.id, order.price, order.quantity);
        }
        println!("   Asks:");
        for (i, order) in matcher.asks_iter().enumerate() {
            println!("     {}. ID={}, Price=${:.2}, Qty={}", 
                     i + 1, order.id, order.price, order.quantity);
        }
    }
    
    println!("\n🎉 Demo completed successfully!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integration() {
        let mut matcher = FifoMatcher::new();
        
        // Test basic matching functionality
        let sell_order = Order::new(1, Side::Sell, 100.0, 10);
        let buy_order = Order::new(2, Side::Buy, 100.0, 5);
        
        matcher.match_order(sell_order).unwrap();
        let trades = matcher.match_order(buy_order).unwrap();
        
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, 5);
        assert_eq!(trades[0].price, 100.0);
    }
}
