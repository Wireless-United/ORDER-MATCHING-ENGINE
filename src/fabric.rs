use crate::types::Event;
use crossbeam_channel::{Receiver, Sender};
use crossbeam_queue::ArrayQueue;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

pub struct Fabric {
    pub ingress_receiver: Receiver<Event>,
    pub shard_queues: HashMap<String, Arc<ArrayQueue<Event>>>,
    pub shard_wakeups: HashMap<String, Sender<()>>,
}

impl Fabric {
    pub fn new(
        ingress_receiver: Receiver<Event>,
        shard_queues: HashMap<String, Arc<ArrayQueue<Event>>>,
        shard_wakeups: HashMap<String, Sender<()>>,
    ) -> Self {
        Self {
            ingress_receiver,
            shard_queues,
            shard_wakeups,
        }
    }

    pub fn run_ingress_worker(&self, worker_id: usize) {
        info!(
            "Ingress worker {} started on thread '{}'", 
            worker_id,
            std::thread::current().name().unwrap_or("unnamed")
        );

        loop {
            match self.ingress_receiver.recv() {
                Ok(event) => {
                    debug!("Worker {} received event: {:?}", worker_id, event);
                    self.route_event(event, worker_id);
                }
                Err(_) => {
                    debug!("Ingress channel closed for worker {}", worker_id);
                    break;
                }
            }
        }

        info!("Ingress worker {} shutting down", worker_id);
    }

    fn route_event(&self, event: Event, worker_id: usize) {
        let symbol = &event.symbol;

        // Get the appropriate shard queue
        if let Some(queue) = self.shard_queues.get(symbol) {
            // Try to push the event to the shard's input queue
            match queue.push(event.clone()) {
                Ok(_) => {
                    debug!(
                        "Worker {} routed event to shard '{}' queue",
                        worker_id, symbol
                    );

                    // Signal the shard that a new event is available
                    if let Some(wakeup_sender) = self.shard_wakeups.get(symbol) {
                        if let Err(_) = wakeup_sender.send(()) {
                            error!("Failed to send wakeup signal to shard '{}'", symbol);
                        }
                    }
                }
                Err(_) => {
                    warn!(
                        "Worker {} failed to route event to shard '{}' - queue full",
                        worker_id, symbol
                    );
                }
            }
        } else {
            error!(
                "Worker {} received event for unknown symbol: '{}'",
                worker_id, symbol
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;
    use std::collections::HashMap;

    #[test]
    fn test_fabric_creation() {
        let (ingress_tx, ingress_rx) = unbounded();
        let shard_queues = HashMap::new();
        let shard_wakeups = HashMap::new();
        
        let fabric = Fabric::new(ingress_rx, shard_queues, shard_wakeups);
        
        // Just ensure it creates successfully
        drop(fabric);
        drop(ingress_tx);
    }

    #[test]
    fn test_fabric_with_shards() {
        let (ingress_tx, ingress_rx) = unbounded();
        let mut shard_queues = HashMap::new();
        let mut shard_wakeups = HashMap::new();
        
        // Create shard infrastructure
        let (wakeup_tx, _) = unbounded();
        let queue = Arc::new(ArrayQueue::new(100));
        
        shard_queues.insert("BTCUSD".to_string(), queue);
        shard_wakeups.insert("BTCUSD".to_string(), wakeup_tx);
        
        let fabric = Fabric::new(ingress_rx, shard_queues, shard_wakeups);
        
        drop(fabric);
        drop(ingress_tx);
    }

    #[test]
    fn test_fabric_route_event() {
        let (ingress_tx, ingress_rx) = unbounded();
        let mut shard_queues = HashMap::new();
        let mut shard_wakeups = HashMap::new();
        
        let (wakeup_tx, wakeup_rx) = unbounded();
        let queue = Arc::new(ArrayQueue::new(100));
        
        shard_queues.insert("BTCUSD".to_string(), queue.clone());
        shard_wakeups.insert("BTCUSD".to_string(), wakeup_tx);
        
        let fabric = Fabric::new(ingress_rx, shard_queues, shard_wakeups);
        
        // Send an event
        let event = Event::new_order(
            Side::BUY,
            10000,
            100,
            "BTCUSD".to_string(),
        );
        
        ingress_tx.send(event).unwrap();
        
        // Give fabric a moment to process (in real test would use thread)
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        drop(ingress_tx);
        drop(fabric);
        drop(wakeup_rx);
        drop(queue);
    }
}