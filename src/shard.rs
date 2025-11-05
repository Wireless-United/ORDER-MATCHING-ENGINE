use crate::types::{Event, Order, Side};
use crossbeam_channel::Receiver;
use crossbeam_queue::ArrayQueue;
use std::collections::BinaryHeap;
use std::sync::Arc;
use tracing::{debug, info};

pub struct Shard {
    pub symbol: String,
    pub buy_orderbook: BinaryHeap<Order>,
    pub sell_orderbook: BinaryHeap<Order>,
    pub input_queue: Arc<ArrayQueue<Event>>,
    pub wakeup_receiver: Receiver<()>,
}

impl Shard {
    pub fn new(
        symbol: String,
        input_queue: Arc<ArrayQueue<Event>>,
        wakeup_receiver: Receiver<()>,
    ) -> Self {
        Self {
            symbol,
            buy_orderbook: BinaryHeap::new(),
            sell_orderbook: BinaryHeap::new(),
            input_queue,
            wakeup_receiver,
        }
    }

    pub fn run(&mut self) {
        info!(
            "Shard for symbol '{}' started on thread '{}'", 
            self.symbol, 
            std::thread::current().name().unwrap_or("unnamed")
        );

        loop {
            // Wait for wake-up signal
            if let Err(_) = self.wakeup_receiver.recv() {
                debug!("Wakeup channel closed for symbol '{}'", self.symbol);
                break;
            }

            // Process all available events in the queue
            while let Some(event) = self.input_queue.pop() {
                self.process_event(event);
            }
        }

        info!("Shard for symbol '{}' shutting down", self.symbol);
    }

    fn process_event(&mut self, event: Event) {
        debug!(
            "Processing event for symbol '{}': {:?}",
            self.symbol, event
        );

        let order = Order::new(event.price, event.qty, event.side);

        match event.side {
            Side::BUY => {
                self.buy_orderbook.push(order);
                info!(
                    "Added BUY order to '{}' orderbook: price={}, qty={}, total_buy_orders={}",
                    self.symbol,
                    event.price,
                    event.qty,
                    self.buy_orderbook.len()
                );
            }
            Side::SELL => {
                self.sell_orderbook.push(order);
                info!(
                    "Added SELL order to '{}' orderbook: price={}, qty={}, total_sell_orders={}",
                    self.symbol,
                    event.price,
                    event.qty,
                    self.sell_orderbook.len()
                );
            }
        }
    }

    #[allow(dead_code)]
    pub fn get_stats(&self) -> (usize, usize) {
        (self.buy_orderbook.len(), self.sell_orderbook.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;

    #[test]
    fn test_shard_creation() {
        let (_, wakeup_rx) = unbounded();
        let queue = Arc::new(ArrayQueue::new(100));
        
        let shard = Shard::new(
            "BTCUSD".to_string(),
            queue,
            wakeup_rx,
        );
        
        assert_eq!(shard.symbol, "BTCUSD");
        assert_eq!(shard.total_trades, 0);
    }

    #[test]
    fn test_shard_set_egress_sender() {
        let (_, wakeup_rx) = unbounded();
        let queue = Arc::new(ArrayQueue::new(100));
        let (egress_tx, _) = unbounded();
        
        let mut shard = Shard::new(
            "BTCUSD".to_string(),
            queue,
            wakeup_rx,
        );
        
        shard.set_egress_sender(egress_tx);
        assert!(shard.egress_sender.is_some());
    }

    #[test]
    fn test_shard_process_event() {
        let (_, wakeup_rx) = unbounded();
        let queue = Arc::new(ArrayQueue::new(100));
        let (egress_tx, egress_rx) = unbounded();
        
        let mut shard = Shard::new(
            "BTCUSD".to_string(),
            queue,
            wakeup_rx,
        );
        
        shard.set_egress_sender(egress_tx);
        
        // Create a buy order event
        let event = Event::new_order(
            Side::BUY,
            10000,
            100,
            "BTCUSD".to_string(),
        );
        
        shard.process_event(event);
        
        // No trades expected with empty book
        assert!(egress_rx.try_recv().is_err());
    }

    #[test]
    fn test_shard_get_stats() {
        let (_, wakeup_rx) = unbounded();
        let queue = Arc::new(ArrayQueue::new(100));
        
        let shard = Shard::new(
            "BTCUSD".to_string(),
            queue,
            wakeup_rx,
        );
        
        let (bids, asks) = shard.get_stats();
        assert_eq!(bids, 0);
        assert_eq!(asks, 0);
    }

    #[test]
    fn test_shard_get_all_stats() {
        let (_, wakeup_rx) = unbounded();
        let queue = Arc::new(ArrayQueue::new(100));
        
        let shard = Shard::new(
            "BTCUSD".to_string(),
            queue,
            wakeup_rx,
        );
        
        let stats = shard.get_all_stats();
        assert!(stats.contains("Hierarchical"));
        assert!(stats.contains("bids"));
        assert!(stats.contains("asks"));
    }

    #[test]
    fn test_shard_process_matching_events() {
        let (_, wakeup_rx) = unbounded();
        let queue = Arc::new(ArrayQueue::new(100));
        let (egress_tx, egress_rx) = unbounded();
        
        let mut shard = Shard::new(
            "BTCUSD".to_string(),
            queue,
            wakeup_rx,
        );
        
        shard.set_egress_sender(egress_tx);
        
        // Add a sell order
        let sell_event = Event::new_order(
            Side::SELL,
            10000,
            50,
            "BTCUSD".to_string(),
        );
        shard.process_event(sell_event);
        
        // Add a matching buy order
        let buy_event = Event::new_order(
            Side::BUY,
            10000,
            50,
            "BTCUSD".to_string(),
        );
        shard.process_event(buy_event);
        
        // Should have a trade
        assert!(shard.total_trades > 0);
        assert!(egress_rx.try_recv().is_ok());
    }
}