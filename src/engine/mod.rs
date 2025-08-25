//! Core trading engine components.
//! 
//! This module contains the fundamental data structures and types
//! used throughout the matching engine system.
//! 
//! # Components
//! 
//! - [`Order`]: Represents a trading order with price, quantity, and metadata
//! - [`Side`]: Enumeration for buy/sell order types
//! - [`shard`]: Placeholder for distributed order processing (future enhancement)

pub mod order;
pub mod shard;

pub use order::*;
