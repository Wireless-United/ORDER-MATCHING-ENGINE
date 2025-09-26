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
use tracing::{error, info, warn};
use tracing_subscriber;

const QUEUE_CAPACITY: usize = 1000;
const NUM_INGRESS_WORKERS: usize = 5;
const SYMBOLS: &[&str] = &["Pranesh", "Superman", "Arnimzola"];

fn check_cpu_requirements() -> Result<Vec<core_affinity::CoreId>, String> {
    let core_ids = core_affinity::get_core_ids().unwrap_or_default();
    let num_cores = core_ids.len();
    let required_cores = SYMBOLS.len() + NUM_INGRESS_WORKERS;

    info!("System has {} CPU cores available", num_cores);
    info!("Required cores: {} symbols + {} ingress workers = {}", 
          SYMBOLS.len(), NUM_INGRESS_WORKERS, required_cores);

    if num_cores < required_cores {
        return Err(format!(
            "Insufficient CPU cores! Available: {}, Required: {} (symbols: {}, ingress: {})",
            num_cores, required_cores, SYMBOLS.len(), NUM_INGRESS_WORKERS
        ));
    }

    Ok(core_ids)
}

fn allocate_cores(core_ids: &[core_affinity::CoreId]) -> (HashMap<String, core_affinity::CoreId>, Vec<core_affinity::CoreId>) {
    let mut shard_cores = HashMap::new();
    let mut ingress_cores = Vec::new();

    // Allocate first N cores to shards
    for (i, &symbol) in SYMBOLS.iter().enumerate() {
        shard_cores.insert(symbol.to_string(), core_ids[i]);
    }

    // Allocate remaining cores to ingress workers
    for i in SYMBOLS.len()..SYMBOLS.len() + NUM_INGRESS_WORKERS {
        ingress_cores.push(core_ids[i]);
    }

    info!("Core allocation:");
    for (symbol, core_id) in &shard_cores {
        info!("  Shard '{}' -> Core {:?}", symbol, core_id);
    }
    for (i, core_id) in ingress_cores.iter().enumerate() {
        info!("  Ingress worker {} -> Core {:?}", i, core_id);
    }

    (shard_cores, ingress_cores)
}

fn get_current_core_id() -> Option<usize> {
    // Try to get current CPU core (Linux specific)
    std::fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|content| {
            content.split_whitespace()
                .nth(38) // CPU field in /proc/stat
                .and_then(|s| s.parse().ok())
        })
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Matching Engine Service");

    // Check CPU core requirements
    let core_ids = match check_cpu_requirements() {
        Ok(cores) => cores,
        Err(err) => {
            error!("{}", err);
            std::process::exit(1);
        }
    };

    // Allocate cores to shards and ingress workers
    let (shard_cores, ingress_cores) = allocate_cores(&core_ids);

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

        // Get assigned core for this shard
        let assigned_core = shard_cores[symbol];

        // Create and spawn shard thread with core pinning and naming
        let symbol_owned = symbol.to_string();
        let mut shard = Shard::new(symbol_owned.clone(), input_queue, wakeup_receiver);
        
        let handle = thread::Builder::new()
            .name(format!("shard-{}", symbol))
            .spawn(move || {
                // Pin to assigned core
                if !core_affinity::set_for_current(assigned_core) {
                    error!("Failed to pin shard '{}' to core {:?}", symbol_owned, assigned_core);
                } else {
                    info!("Shard '{}' pinned to core {:?}", symbol_owned, assigned_core);
                }

                // Log current core (verification)
                if let Some(current_core) = get_current_core_id() {
                    info!("Shard '{}' verified running on core {}", symbol_owned, current_core);
                } else {
                    warn!("Could not verify core assignment for shard '{}'", symbol_owned);
                }

                // Run the shard
                shard.run();
            })
            .expect("Failed to create shard thread");
            
        shard_handles.push(handle);
    }

    // Create fabric for routing
    let fabric = Arc::new(Fabric::new(ingress_receiver, shard_queues, shard_wakeups));

    // Spawn ingress worker threads with optional core pinning
    let mut ingress_handles = Vec::new();
    for worker_id in 0..NUM_INGRESS_WORKERS {
        let fabric_clone = fabric.clone();
        let assigned_core = ingress_cores.get(worker_id).copied();
        
        let handle = thread::Builder::new()
            .name(format!("ingress-{}", worker_id))
            .spawn(move || {
                // Pin to assigned core if available
                if let Some(core) = assigned_core {
                    if !core_affinity::set_for_current(core) {
                        error!("Failed to pin ingress worker {} to core {:?}", worker_id, core);
                    } else {
                        info!("Ingress worker {} pinned to core {:?}", worker_id, core);
                    }
                } else {
                    info!("Ingress worker {} running without specific core pinning", worker_id);
                }

                // Run the ingress worker
                fabric_clone.run_ingress_worker(worker_id);
            })
            .expect("Failed to create ingress worker thread");
            
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