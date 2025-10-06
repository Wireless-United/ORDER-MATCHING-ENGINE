//! High-Performance Concurrent Order Matching Engine
//! Uses Tokio (async), Rayon (parallel), and lock-free queues

mod algorithms;
mod engine;
mod utils;

use std::sync::Arc;
use std::time::Duration;
use concurrent_queue::ConcurrentQueue;
use tokio::sync::Notify;
use engine::{create_shared_orderbook, shard::spawn_shard_workers, ingress::spawn_ingress_tasks};

#[tokio::main]
async fn main() {
    #[allow(dead_code)]
    const NUM_SHARDS: usize = 4;
    #[allow(dead_code)]
    const NUM_INGRESS_TASKS: usize = 2;
    #[allow(dead_code)]
    const ORDERS_PER_INGRESS: u64 = 50;
    #[allow(dead_code)]
    const ORDER_INTERVAL_MS: u64 = 20;
    
    println!("Order Matching Engine: {} shards, {} tasks, {} orders", 
        NUM_SHARDS, NUM_INGRESS_TASKS, NUM_INGRESS_TASKS as u64 * ORDERS_PER_INGRESS);
    
    let queue = Arc::new(ConcurrentQueue::unbounded());
    let order_book = create_shared_orderbook();
    let shutdown = Arc::new(Notify::new());
    
    let shard_handles = spawn_shard_workers(NUM_SHARDS, Arc::clone(&queue), Arc::clone(&order_book), Arc::clone(&shutdown));
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let ingress_handles = spawn_ingress_tasks(NUM_INGRESS_TASKS, Arc::clone(&queue), NUM_SHARDS, Arc::clone(&shutdown), ORDERS_PER_INGRESS, ORDER_INTERVAL_MS);
    
    for handle in ingress_handles { handle.await.ok(); }
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    shutdown.notify_waiters();
    for handle in shard_handles { handle.await.ok(); }
    
    println!("Completed: {} orders processed with lock-free queues + Rayon parallelism", 
        NUM_INGRESS_TASKS as u64 * ORDERS_PER_INGRESS);
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Order, Side, Request, RequestType, OrderBookRef};
    use algorithms::fifo;
    
    #[test]
    #[allow(unused_variables)]
    fn test_basic_integration() {
        let order_book = OrderBookRef::new();
        let order = Order::new(1, Side::Buy, 100.0, 50);
        let request = Request::new(1, 0, RequestType::Fifo, order);
        
        let trades = fifo::process(request, &order_book);
        // No trades expected since there are no resting orders
        assert_eq!(trades.len(), 0);
    }
    
    #[tokio::test]
    #[allow(unused_variables)]
    async fn test_concurrent_queue() {
        let queue = Arc::new(ConcurrentQueue::unbounded());
        let order = Order::new(1, Side::Buy, 100.0, 50);
        let request = Request::new(1, 0, RequestType::Fifo, order);
        
        queue.push(request).expect("Failed to push");
        assert_eq!(queue.len(), 1);
        
        let _popped = queue.pop().expect("Failed to pop");
        assert_eq!(queue.len(), 0);
    }
}
