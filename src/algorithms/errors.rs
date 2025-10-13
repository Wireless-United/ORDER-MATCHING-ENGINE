use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AlgorithmError {
    #[error("Invalid order: {0}")]
    InvalidOrder(String),
    #[error("Book error: {0}")]
    #[allow(dead_code)]
    BookError(String),
    #[error("Internal error")]
    #[allow(dead_code)]
    Internal,
}
