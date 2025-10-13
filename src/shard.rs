use crate::types::{Event, Order, Trade};
use crate::algorithms::matcher_bridge::HierarchicalMatcherBridge;
use crossbeam_channel::{Receiver, Sender};
use crossbeam_queue::ArrayQueue;
use std::sync::Arc;
use tracing::{debug, info, warn};

pub struct Shard {
    pub symbol: String,
    pub hierarchical_matcher: HierarchicalMatcherBridge,
    pub input_queue: Arc<ArrayQueue<Event>>,
    pub wakeup_receiver: Receiver<()>,
    pub egress_sender: Option<Sender<Trade>>,
    pub total_trades: usize,
}

impl Shard {
    pub fn new(
        symbol: String,
        input_queue: Arc<ArrayQueue<Event>>,
        wakeup_receiver: Receiver<()>,
    ) -> Self {
        Self {
            symbol,
            hierarchical_matcher: HierarchicalMatcherBridge::new_with_config(0.4, 0.3, 0.3), // 40% FIFO, 30% Pro-Rata, 30% Hybrid
            input_queue,
            wakeup_receiver,
            egress_sender: None,
            total_trades: 0,
        }
    }

    pub fn set_egress_sender(&mut self, sender: Sender<Trade>) {
        self.egress_sender = Some(sender);
        info!("Egress sender configured for shard '{}'", self.symbol);
    }

    pub fn run(&mut self) {
        info!(
            "Shard for symbol '{}' started on thread '{}'", 
            self.symbol, 
            std::thread::current().name().unwrap_or("unnamed")
        );

        loop {
            // Wait for wake-up signal
            if self.wakeup_receiver.recv().is_err() {
                debug!("Wakeup channel closed for symbol '{}'", self.symbol);
                break;
            }

            // Process all available events in the queue
            while let Some(event) = self.input_queue.pop() {
                self.process_event(event);
            }
        }

        info!(
            "Shard for symbol '{}' shutting down. Total trades: {}",
            self.symbol, self.total_trades
        );
    }

    /// Main event processing function that handles order matching
    pub fn process_event(&mut self, event: Event) {
        debug!(
            "Processing event for symbol '{}': Order {} {:?} {} @ {}",
            self.symbol, event.order_id, event.side, event.qty, event.price
        );

        let incoming_order = Order::from_event(&event);

        // Use Hierarchical matcher (which internally runs FIFO → Pro-Rata → Hybrid sequentially)
        debug!("Using Hierarchical matcher from algorithms/hierarchical.rs");
        let trades = self.hierarchical_matcher.match_order(incoming_order);
        
        if !trades.is_empty() {
            let phase_stats = self.hierarchical_matcher.get_phase_stats();
            info!("Hierarchical matching complete: {}", phase_stats);
        }

        // Log trades
        if !trades.is_empty() {
            self.total_trades += trades.len();
            for trade in &trades {
                info!(
                    "TRADE [{}]: Buy Order {} & Sell Order {} matched {} @ {} (Trade ID: {})",
                    self.symbol,
                    trade.buy_order_id,
                    trade.sell_order_id,
                    trade.qty,
                    trade.price,
                    trade.trade_id
                );

                // Send trade to egress channel
                if let Some(ref egress_sender) = self.egress_sender {
                    if let Err(e) = egress_sender.send(trade.clone()) {
                        warn!("Failed to send trade {} to egress channel: {:?}", trade.trade_id, e);
                    } else {
                        debug!("Trade {} sent to egress channel", trade.trade_id);
                    }
                }
            }
        } else {
            debug!(
                "No trades executed for order {}. Added to orderbook.",
                event.order_id
            );
        }
    }

    /// Get orderbook statistics
    #[allow(dead_code)]
    pub fn get_stats(&self) -> (usize, usize) {
        (self.hierarchical_matcher.bid_depth(), self.hierarchical_matcher.ask_depth())
    }

    /// Get orderbook state as string
    #[allow(dead_code)]
    pub fn get_all_stats(&self) -> String {
        let (bids, asks) = self.get_stats();
        format!("Hierarchical: {} bids, {} asks", bids, asks)
    }
}
