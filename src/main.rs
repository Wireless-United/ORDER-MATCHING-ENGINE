mod api;
mod fabric;
mod shard;
mod types;

use api::{create_router, AppState};
use fabric::Fabric;
use shard::Shard;
use types::Event;

use crossbeam_channel::{unbounded, Receiver, Sender};
use crossbeam_queue::ArrayQueue;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use tokio::net::TcpListener;
use tracing::{error, info};
use tracing_subscriber;

const QUEUE_CAPACITY: usize = 1000;
const NUM_INGRESS_WORKERS: usize = 5;
const SYMBOLS: &[&str] = &["Pranesh", "Superman", "Arnimzola"];

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Matching Engine Service");

    // Create the ingress channel for routing orders
    let (ingress_sender, ingress_receiver): (Sender<Event>, Receiver<Event>) = unbounded();

    // Initialize shard infrastructure
    let mut shard_queues: HashMap<String, Arc<ArrayQueue<Event>>> = HashMap::new();
    let mut shard_wakeups: HashMap<String, Sender<()>> = HashMap::new();
    let mut shard_handles = Vec::new();

    // Create shards for each symbol
    for &symbol in SYMBOLS {
        info!("Initializing shard for symbol: {}", symbol);

        // Create input queue for this shard
        let input_queue = Arc::new(ArrayQueue::new(QUEUE_CAPACITY));
        shard_queues.insert(symbol.to_string(), input_queue.clone());

        // Create wakeup channel for this shard
        let (wakeup_sender, wakeup_receiver) = unbounded();
        shard_wakeups.insert(symbol.to_string(), wakeup_sender);

        // Create and spawn shard
        let mut shard = Shard::new(symbol.to_string(), input_queue, wakeup_receiver);
        let handle = thread::spawn(move || {
            shard.run();
        });
        shard_handles.push(handle);
    }

    // Create fabric for routing
    let fabric = Arc::new(Fabric::new(ingress_receiver, shard_queues, shard_wakeups));

    // Spawn ingress worker threads
    let mut ingress_handles = Vec::new();
    for worker_id in 0..NUM_INGRESS_WORKERS {
        let fabric_clone = fabric.clone();
        let handle = thread::spawn(move || {
            fabric_clone.run_ingress_worker(worker_id);
        });
        ingress_handles.push(handle);
    }

    // Create HTTP server
    let app_state = AppState { ingress_sender };
    let app = create_router(app_state);

    // Start the server
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("Matching Engine Service listening on http://0.0.0.0:3000");
    info!("Available endpoints:");
    info!("  POST /buy   - Submit buy orders");
    info!("  POST /sell  - Submit sell orders");
    info!("  POST /health - Health check");
    info!("Supported symbols: {:?}", SYMBOLS);

    if let Err(e) = axum::serve(listener, app).await {
        error!("Server error: {}", e);
    }

    // Wait for all threads to complete (this won't happen in normal operation)
    for handle in shard_handles {
        let _ = handle.join();
    }

    for handle in ingress_handles {
        let _ = handle.join();
    }

    info!("Matching Engine Service shutting down");
}