//! Order matching algorithms and related types.
//! 
//! This module provides various order matching algorithms for building
//! trading systems and exchanges. Currently includes:
//! 
//! - **FIFO (First-In-First-Out)**: Time-priority based matching
//! 
//! # Usage
//! 
//! ```rust
//! use order_matching_engine::algorithms::FifoMatcher;
//! use order_matching_engine::engine::{Order, Side};
//! 
//! let mut matcher = FifoMatcher::new();
//! let order = Order::new(1, Side::Buy, 100.0, 50);
//! let trades = matcher.match_order(order)?;
//! # Ok::<(), order_matching_engine::algorithms::AlgorithmError>(())
//! ```

pub mod errors;
pub mod fifo;
pub mod test;

pub use errors::AlgorithmError;
pub use fifo::FifoMatcher;
