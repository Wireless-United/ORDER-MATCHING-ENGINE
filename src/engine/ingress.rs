use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use concurrent_queue::ConcurrentQueue;
use tokio::sync::Notify;
use crate::engine::{Request, RequestType, Order, Side};

static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

pub struct IngressHandler {
    queue: Arc<ConcurrentQueue<Request>>,
    num_shards: usize,
    shutdown: Arc<Notify>,
}

impl IngressHandler {
    pub fn new(queue: Arc<ConcurrentQueue<Request>>, num_shards: usize, shutdown: Arc<Notify>) -> Self {
        Self { queue, num_shards, shutdown }
    }

    pub fn submit(&self, request_type: RequestType, order: Order) -> u64 {
        let request_id = REQUEST_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
        let shard_id = (order.id as usize) % self.num_shards;
        let request = Request::new(request_id, shard_id, request_type, order);
        self.queue.push(request).expect("Queue push failed");
        request_id
    }

    pub async fn run_demo_ingress(&self, num_orders: u64, interval_ms: u64) {
        for i in 0..num_orders {
            tokio::select! {
                _ = self.shutdown.notified() => break,
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)) => {
                    let side = if i % 2 == 0 { Side::Buy } else { Side::Sell };
                    let order = Order::new(i, side, 100.0 + (i % 10) as f64, 50 + (i % 5) * 10);
                    let request_type = match i % 3 {
                        0 => RequestType::Fifo,
                        1 => RequestType::ProRata,
                        _ => RequestType::Hybrid,
                    };
                    self.submit(request_type, order);
                }
            }
        }
    }
}

pub fn spawn_ingress_tasks(
    num_tasks: usize,
    queue: Arc<ConcurrentQueue<Request>>,
    num_shards: usize,
    shutdown: Arc<Notify>,
    orders_per_task: u64,
    interval_ms: u64,
) -> Vec<tokio::task::JoinHandle<()>> {
    (0..num_tasks)
        .map(|_| {
            let handler = IngressHandler::new(Arc::clone(&queue), num_shards, Arc::clone(&shutdown));
            tokio::spawn(async move { handler.run_demo_ingress(orders_per_task, interval_ms).await })
        })
        .collect()
}
