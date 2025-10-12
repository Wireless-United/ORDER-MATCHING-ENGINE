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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_order_error() {
        let error = AlgorithmError::InvalidOrder("Zero quantity".to_string());
        assert_eq!(error.to_string(), "Invalid order: Zero quantity");
    }

    #[test]
    fn test_book_error() {
        let error = AlgorithmError::BookError("Book locked".to_string());
        assert_eq!(error.to_string(), "Book error: Book locked");
    }

    #[test]
    fn test_internal_error() {
        let error = AlgorithmError::Internal;
        assert_eq!(error.to_string(), "Internal error");
    }

    #[test]
    fn test_error_equality() {
        let error1 = AlgorithmError::InvalidOrder("test".to_string());
        let error2 = AlgorithmError::InvalidOrder("test".to_string());
        let error3 = AlgorithmError::InvalidOrder("other".to_string());
        
        assert_eq!(error1, error2);
        assert_ne!(error1, error3);
    }

    #[test]
    fn test_error_clone() {
        let error = AlgorithmError::InvalidOrder("test".to_string());
        let cloned = error.clone();
        assert_eq!(error, cloned);
    }

    #[test]
    fn test_error_debug() {
        let error = AlgorithmError::Internal;
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("Internal"));
    }
}