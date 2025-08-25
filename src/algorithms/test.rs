#[cfg(test)]
mod trade_ranking_tests {
    use crate::engine::{Order, Side};
    use crate::algorithms::fifo::FifoMatcher;

    #[test]
    fn test_trade_ranking_sequential() {
        FifoMatcher::reset_trade_rank();
        
        let mut matcher = FifoMatcher::new();
        
        let buy1 = Order::new(1, Side::Buy, 100.0, 10);
        let sell1 = Order::new(2, Side::Sell, 100.0, 5);
        let sell2 = Order::new(3, Side::Sell, 100.0, 8);
        
        matcher.match_order(buy1).unwrap();
        let trades1 = matcher.match_order(sell1).unwrap();
        let trades2 = matcher.match_order(sell2).unwrap();
        
        assert_eq!(trades1.len(), 1);
        assert_eq!(trades2.len(), 1);
        assert_eq!(trades1[0].rank, 1);
        assert_eq!(trades2[0].rank, 2);
    }

    #[test]
    fn test_trade_ranking_uniqueness() {
        FifoMatcher::reset_trade_rank();
        
        let mut matcher1 = FifoMatcher::new();
        let mut matcher2 = FifoMatcher::new();
        
        let buy1 = Order::new(1, Side::Buy, 100.0, 10);
        let sell1 = Order::new(2, Side::Sell, 100.0, 10);
        let buy2 = Order::new(3, Side::Buy, 100.0, 10);
        let sell2 = Order::new(4, Side::Sell, 100.0, 10);
        
        matcher1.match_order(buy1).unwrap();
        matcher2.match_order(buy2).unwrap();
        
        let trades1 = matcher1.match_order(sell1).unwrap();
        let trades2 = matcher2.match_order(sell2).unwrap();
        
        assert_ne!(trades1[0].rank, trades2[0].rank);
        assert!(trades1[0].rank < trades2[0].rank || trades1[0].rank > trades2[0].rank);
    }

    #[test]
    fn test_trade_count_tracking() {
        FifoMatcher::reset_trade_rank();
        
        let mut matcher = FifoMatcher::new();
        
        assert_eq!(FifoMatcher::get_trade_count(), 0);
        
        let buy = Order::new(1, Side::Buy, 100.0, 10);
        let sell = Order::new(2, Side::Sell, 100.0, 5);
        
        matcher.match_order(buy).unwrap();
        matcher.match_order(sell).unwrap();
        
        assert_eq!(FifoMatcher::get_trade_count(), 1);
    }

    #[test]
    fn test_multiple_partial_fills_ranking() {
        FifoMatcher::reset_trade_rank();
        
        let mut matcher = FifoMatcher::new();
        
        let buy = Order::new(1, Side::Buy, 100.0, 20);
        let sell1 = Order::new(2, Side::Sell, 100.0, 5);
        let sell2 = Order::new(3, Side::Sell, 100.0, 8);
        let sell3 = Order::new(4, Side::Sell, 100.0, 7);
        
        matcher.match_order(buy).unwrap();
        
        let trades1 = matcher.match_order(sell1).unwrap();
        let trades2 = matcher.match_order(sell2).unwrap();
        let trades3 = matcher.match_order(sell3).unwrap();
        
        assert_eq!(trades1.len(), 1);
        assert_eq!(trades2.len(), 1);
        assert_eq!(trades3.len(), 1);
        assert_eq!(trades1[0].rank, 1);
        assert_eq!(trades2[0].rank, 2);
        assert_eq!(trades3[0].rank, 3);
        
        assert_eq!(FifoMatcher::get_trade_count(), 3);
    }

    #[test]
    fn test_trade_rank_reset() {
        let mut matcher = FifoMatcher::new();
        
        let buy = Order::new(1, Side::Buy, 100.0, 10);
        let sell = Order::new(2, Side::Sell, 100.0, 10);
        
        matcher.match_order(buy).unwrap();
        matcher.match_order(sell).unwrap();
        
        let count_before_reset = FifoMatcher::get_trade_count();
        assert!(count_before_reset > 0);
        
        FifoMatcher::reset_trade_rank();
        assert_eq!(FifoMatcher::get_trade_count(), 0);
        
        let buy2 = Order::new(3, Side::Buy, 100.0, 10);
        let sell2 = Order::new(4, Side::Sell, 100.0, 10);
        
        matcher.match_order(buy2).unwrap();
        let trades = matcher.match_order(sell2).unwrap();
        
        assert_eq!(trades[0].rank, 1);
    }
}
