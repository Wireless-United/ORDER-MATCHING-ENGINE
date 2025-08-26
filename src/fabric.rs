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
        info!("Ingress worker {} started", worker_id);

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
                    // In a real system, you might want to implement backpressure or overflow handling
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
