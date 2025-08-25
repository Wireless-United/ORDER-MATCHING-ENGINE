use thiserror::Error;

/// Errors that can occur during order matching operations.
/// 
/// This enum provides comprehensive error handling for various failure
/// scenarios in the matching engine, with descriptive error messages
/// to aid in debugging and monitoring.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AlgorithmError {
    /// Invalid order parameters were provided.
    /// 
    /// This error occurs when an order fails basic validation,
    /// such as having zero quantity or negative price.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use order_matching_engine::algorithms::AlgorithmError;
    /// 
    /// let error = AlgorithmError::InvalidOrder("Price must be positive".to_string());
    /// assert!(error.to_string().contains("Invalid order input"));
    /// ```
    #[error("Invalid order input: {0}")]
    InvalidOrder(String),
    
    /// Order book is in an inconsistent state.
    /// 
    /// This error indicates internal data structure corruption
    /// or logical inconsistencies in the order book state.
    #[error("Order book inconsistency: {0}")]
    BookError(String),
    
    /// Internal processing error occurred.
    /// 
    /// This is a catch-all error for unexpected internal failures
    /// that shouldn't normally occur during operation.
    #[error("Internal matching error")]
    Internal,
}
