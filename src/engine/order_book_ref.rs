//! Order book reference wrapper.
//!
//! Provides a lightweight wrapper around the order book data structures
//! for read-only access in concurrent matching operations.

use std::sync::Arc;

/// A thread-safe, read-only reference to order book state.
///
/// This wrapper provides immutable access to the order book for
/// validation and lookup operations during order matching.
/// Since the order-book module contains only templates, this
/// serves as a placeholder/mock for demonstration purposes.
#[derive(Clone)]
pub struct OrderBookRef {
    // In a real implementation, this would hold Arc references to
    // the actual AskBook and BidBook structures
    _phantom: (),
}

impl OrderBookRef {
    /// Creates a new OrderBookRef.
    pub fn new() -> Self {
        Self { _phantom: () }
    }

    /// Validates if an order can be processed.
    ///
    /// This is a placeholder for actual validation logic that would
    /// check against the order book state.
    pub fn validate_order(&self, _order_id: u64, _price: f64, _quantity: u64) -> bool {
        true
    }

    /// Gets the best bid price if available.
    #[allow(dead_code)]
    pub fn best_bid(&self) -> Option<f64> {
        None
    }

    /// Gets the best ask price if available.
    #[allow(dead_code)]
    pub fn best_ask(&self) -> Option<f64> {
        None
    }
}

impl Default for OrderBookRef {
    fn default() -> Self {
        Self::new()
    }
}

/// Type alias for a shared, thread-safe OrderBookRef.
pub type SharedOrderBook = Arc<OrderBookRef>;

/// Creates a new shared OrderBookRef.
pub fn create_shared_orderbook() -> SharedOrderBook {
    Arc::new(OrderBookRef::new())
}
