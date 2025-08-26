//! Order matching algorithms and related types.
//! 
//! This module provides various order matching algorithms for building
//! trading systems and exchanges. Currently includes:
//! 
//! - **FIFO (First-In-First-Out)**: Time-priority based matching
//! - **Pro-Rata**: Proportional allocation based matching
//! - **Hybrid**: Combination of FIFO and Pro-Rata matching
//! 
//! # Usage
//! 
//! ```rust
//! use order_matching_engine::algorithms::{FifoMatcher, ProRataMatcher, HybridMatcher};
//! use order_matching_engine::engine::{Order, Side};
//! 
//! // FIFO matching
//! let mut fifo_matcher = FifoMatcher::new();
//! let order = Order::new(1, Side::Buy, 100.0, 50);
//! let trades = fifo_matcher.match_order(order)?;
//! 
//! // Pro-Rata matching
//! let mut pro_rata_matcher = ProRataMatcher::new();
//! let trades = pro_rata_matcher.match_order(order);
//! 
//! // Hybrid matching
//! let mut hybrid_matcher = HybridMatcher::new();
//! let trades = hybrid_matcher.match_order(order);
//! # Ok::<(), order_matching_engine::algorithms::AlgorithmError>(())
//! ```

pub mod errors;
pub mod fifo;
pub mod pro_rata;
pub mod hybrid;
pub mod test;

pub use errors::AlgorithmError;
pub use fifo::FifoMatcher;
pub use pro_rata::ProRataMatcher;
pub use hybrid::HybridMatcher;
