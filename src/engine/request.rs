//! Request model for order matching system.
//!
//! This module defines the request types that flow through the concurrent
//! order matching pipeline.

use crate::engine::Order;

/// Type of matching algorithm to apply to the request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestType {
    /// First-In-First-Out matching algorithm
    Fifo,
    /// Hybrid matching algorithm (combination of FIFO and Pro-Rata)
    Hybrid,
    /// Pro-Rata matching algorithm
    ProRata,
}

/// A request representing an order to be processed by the matching engine.
#[derive(Clone, Debug)]
pub struct Request {
    /// Unique identifier for this request
    pub id: u64,
    /// Shard ID for routing (determines which shard handles this request)
    pub shard_id: usize,
    /// Type of matching algorithm to use
    pub request_type: RequestType,
    /// The order to be matched
    pub order: Order,
}

impl Request {
    /// Creates a new request.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the request
    /// * `shard_id` - Shard that should process this request
    /// * `request_type` - Matching algorithm to use
    /// * `order` - The order to be matched
    pub fn new(id: u64, shard_id: usize, request_type: RequestType, order: Order) -> Self {
        Self {
            id,
            shard_id,
            request_type,
            order,
        }
    }
}
