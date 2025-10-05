use crate::algorithms::{fifo, hybrid, pro_rata};
use crate::engine::{OrderBookRef, Request};
use concurrent_queue::ConcurrentQueue;
use std::sync::Arc;
use tokio::sync::Notify;

const BATCH_SIZE: usize = 100; // Max requests to batch per cycle
const POLL_INTERVAL_MS: u64 = 1; // Milliseconds between queue polls

/// Worker that processes orders for a specific shard
pub struct ShardWorker {
    pub shard_id: usize,
    pub queue: Arc<ConcurrentQueue<Request>>,
    pub order_book: Arc<OrderBookRef>,
    pub shutdown: Arc<Notify>,
}

impl ShardWorker {
    /// Create a new shard worker with specified configuration
    pub fn new(
        shard_id: usize,
        queue: Arc<ConcurrentQueue<Request>>,
        order_book: Arc<OrderBookRef>,
        shutdown: Arc<Notify>,
    ) -> Self {
        Self { shard_id, queue, order_book, shutdown }
    }

    /// Main event loop: polls queue and processes batches until shutdown
    pub async fn run(self) {
        loop {
            tokio::select! {
                _ = self.shutdown.notified() => break, // Graceful shutdown
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(POLL_INTERVAL_MS)) => {
                    if let Some(batch) = self.collect_batch() {
                        self.process_batch(batch).await;
                    }
                }
            }
        }
    }

    /// Collect up to BATCH_SIZE requests belonging to this shard
    fn collect_batch(&self) -> Option<Vec<Request>> {
        let mut batch = Vec::with_capacity(BATCH_SIZE);
        while let Ok(req) = self.queue.pop() {
            if req.shard_id == self.shard_id {
                batch.push(req);
                if batch.len() >= BATCH_SIZE { break; }
            } else {
                let _ = self.queue.push(req); // Wrong shard, requeue
                break;
            }
        }
        if batch.is_empty() { None } else { Some(batch) }
    }

    /// Process batch of requests in parallel using Rayon
    async fn process_batch(&self, batch: Vec<Request>) {
        let order_book = Arc::clone(&self.order_book);
        tokio::task::spawn_blocking(move || {
            use rayon::prelude::*;
            batch.par_iter().for_each(|req| {
                Self::route_request(req, &order_book);
            });
        })
        .await
        .expect("Batch processing panicked");
    }

    /// Route request to appropriate matching algorithm
    fn route_request(req: &Request, order_book: &Arc<OrderBookRef>) {
        match req.request_type {
            crate::engine::RequestType::Fifo => { fifo::process(req.clone(), order_book); },
            crate::engine::RequestType::ProRata => { pro_rata::process(req.clone(), order_book); },
            crate::engine::RequestType::Hybrid => { hybrid::process(req.clone(), order_book); },
        }
    }
}

/// Spawn multiple shard workers and return their join handles
pub fn spawn_shard_workers(
    num_shards: usize,
    queue: Arc<ConcurrentQueue<Request>>,
    order_book: Arc<OrderBookRef>,
    shutdown: Arc<Notify>,
) -> Vec<tokio::task::JoinHandle<()>> {
    (0..num_shards)
        .map(|id| {
            let worker = ShardWorker::new(id, Arc::clone(&queue), Arc::clone(&order_book), Arc::clone(&shutdown));
            tokio::spawn(async move { worker.run().await })
        })
        .collect()
}

