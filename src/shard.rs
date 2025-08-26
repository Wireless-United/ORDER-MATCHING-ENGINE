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
        info!("Shard for symbol '{}' started", self.symbol);

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

    pub fn get_stats(&self) -> (usize, usize) {
        (self.buy_orderbook.len(), self.sell_orderbook.len())
    }
}
