//! Core trading engine components.
//! 
//! This module contains the fundamental data structures and types
//! used throughout the matching engine system.
//! 
//! # Components
//! 
//! - [`Order`]: Represents a trading order with price, quantity, and metadata
//! - [`Side`]: Enumeration for buy/sell order types
//! - [`Request`]: Request type for concurrent order processing
//! - [`OrderBookRef`]: Thread-safe read-only reference to order book
//! - [`ingress`]: Async ingress for receiving and routing orders
//! - [`shard`]: Distributed order processing with async workers

pub mod ingress;
pub mod order;
pub mod order_book_ref;
pub mod request;
pub mod shard;

pub use order::*;
pub use order_book_ref::*;
pub use request::*;

