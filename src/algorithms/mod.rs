//! Order matching algorithms and related types.
//! 
//! This module provides various order matching algorithms for building
//! trading systems and exchanges. Currently includes:
//! 
//! - **FIFO (First-In-First-Out)**: Time-priority based matching
//! - **Pro-Rata**: Proportional allocation based matching
//! - **Hybrid**: Combination of FIFO and Pro-Rata matching

pub mod errors;
pub mod fifo;
pub mod pro_rata;
pub mod hybrid;
pub mod test;
pub mod matcher_bridge;

//can add more algorithms here 