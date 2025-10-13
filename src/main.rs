// Suppress warnings for dead code and unused variables/imports
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(non_snake_case)]

mod api;
mod fabric;
mod shard;
mod types;
mod algorithms;

mod utils {
    pub mod affinity;
}

use api::{create_router, spawn_egress_workers, AppState};
use fabric::Fabric;
use shard::Shard;
use types::{Event, Trade};

use crossbeam_channel::{unbounded, Receiver, Sender};
use crossbeam_queue::ArrayQueue;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use tokio::net::TcpListener;
use tracing::{error, info};
#[allow(unused_imports)]
use tracing::warn;
use tracing_subscriber;

const QUEUE_CAPACITY: usize = 1000;
const NUM_INGRESS_WORKERS: usize = 5;
const NUM_EGRESS_WORKERS: usize = 3;

#[allow(dead_code)]
fn check_cpu_requirements(symbols: &[String]) -> Result<(usize, Vec<core_affinity::CoreId>), String> {
    let num_cores = num_cpus::get();
    let core_ids = core_affinity::get_core_ids().unwrap_or_default();
    let required_cores = symbols.len(); // Only check for shard cores

    info!("System has {} CPU cores available", num_cores);
    info!("Required cores: {} symbols (shards only)", symbols.len());

    if num_cores < required_cores {
        return Err(format!(
            "Insufficient CPU cores! Available: {}, Required: {} (shards only)",
            num_cores, required_cores
        ));
    }

    Ok((num_cores, core_ids))
}

fn allocate_shard_cores(core_ids: &[core_affinity::CoreId], symbols: &[String]) -> HashMap<String, core_affinity::CoreId> {
    let mut shard_cores = HashMap::new();

    // Allocate first N cores to shards only
    for (i, symbol) in symbols.iter().enumerate() {
        shard_cores.insert(symbol.clone(), core_ids[i]);
    }

    info!("Core allocation:");
    for (symbol, core_id) in &shard_cores {
        info!("  Shard '{}' -> Core {:?}", symbol, core_id);
    }
    info!("Ingress workers: NOT PINNED (using remaining cores freely)");
    info!("HTTP threads: NOT PINNED (using remaining cores freely)");

    shard_cores
}

#[allow(dead_code)]
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

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Matching Engine Service");

    // Get initial symbols from AppState
    let symbols = AppState::get_initial_symbols();
    info!("Using symbols: {:?}", symbols);

    // Check CPU core requirements (only for shards)
    let (total_cores, core_ids) = match check_cpu_requirements(&symbols) {
        Ok(result) => result,
        Err(err) => {
            error!("{}", err);
            std::process::exit(1);
        }
    };

    // Allocate cores only to shards
    let shard_cores = allocate_shard_cores(&core_ids, &symbols);

    // Calculate cores available for Tokio runtime
    let tokio_worker_threads = total_cores.saturating_sub(symbols.len()).max(1);
    info!("Configuring Tokio runtime with {} worker threads (total: {}, pinned: {})", 
          tokio_worker_threads, total_cores, symbols.len());

    // Build Tokio runtime manually with specific worker threads
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(tokio_worker_threads)
        .thread_name("http-worker")
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");
    
    runtime.block_on(async {
        // Create the ingress channel for routing orders
        let (ingress_sender, ingress_receiver): (Sender<Event>, Receiver<Event>) = unbounded();

        // Create the egress channel for trade outputs
        let (egress_sender, egress_receiver): (Sender<Trade>, Receiver<Trade>) = unbounded();
        info!("Created egress channel for trade outputs");

        // Initialize shard infrastructure
        let mut shard_queues: HashMap<String, Arc<ArrayQueue<Event>>> = HashMap::new();
        let mut shard_wakeups: HashMap<String, Sender<()>> = HashMap::new();
        let mut shard_handles = Vec::new();

        // Create shards for each symbol
        for symbol in &symbols {
            info!("Initializing shard for symbol: {}", symbol);

            // Create input queue for this shard
            let input_queue = Arc::new(ArrayQueue::new(QUEUE_CAPACITY));
            shard_queues.insert(symbol.clone(), input_queue.clone());

            // Create wakeup channel for this shard
            let (wakeup_sender, wakeup_receiver) = unbounded();
            shard_wakeups.insert(symbol.clone(), wakeup_sender);

            // Get assigned core for this shard
            let assigned_core = shard_cores[symbol];

            // Create and spawn shard thread with core pinning and naming
            let symbol_owned = symbol.clone();
            let mut shard = Shard::new(symbol_owned.clone(), input_queue, wakeup_receiver);
            
            // Configure egress sender for this shard
            shard.set_egress_sender(egress_sender.clone());
            
            let handle = thread::Builder::new()
                .name(format!("shard-{}", symbol))
                .spawn(move || {
                    // Pin to assigned core
                    if !core_affinity::set_for_current(assigned_core) {
                        error!("Failed to pin shard '{}' to core {:?}", symbol_owned, assigned_core);
                    } else {
                        info!("Shard '{}' pinned to core {:?}", symbol_owned, assigned_core);
                    }

                    // Log current core (verification) - Linux only
                    #[cfg(target_os = "linux")]
                    {
                        if let Some(current_core) = get_current_core_id() {
                            info!("Shard '{}' verified running on core {}", symbol_owned, current_core);
                        } else {
                            warn!("Could not verify core assignment for shard '{}'", symbol_owned);
                        }
                    }

                    // Run the shard
                    shard.run();
                })
                .expect("Failed to create shard thread");
                
            shard_handles.push(handle);
        }

        // Create fabric for routing
        let fabric = Arc::new(Fabric::new(ingress_receiver, shard_queues, shard_wakeups));

        // Spawn ingress worker threads WITHOUT core pinning
        let mut ingress_handles = Vec::new();
        for worker_id in 0..NUM_INGRESS_WORKERS {
            let fabric_clone = fabric.clone();
            
            let handle = thread::Builder::new()
                .name(format!("ingress-{}", worker_id))
                .spawn(move || {
                    info!("Ingress worker {} started (not pinned to specific core)", worker_id);

                    // Run the ingress worker
                    fabric_clone.run_ingress_worker(worker_id);
                })
                .expect("Failed to create ingress worker thread");
                
            ingress_handles.push(handle);
        }

        // Spawn egress worker threads WITHOUT core pinning
        info!("Spawning {} egress workers", NUM_EGRESS_WORKERS);
        let egress_handles = spawn_egress_workers(egress_receiver.clone(), NUM_EGRESS_WORKERS);

        // Create HTTP server
        let app_state = AppState::new(ingress_sender);
        
        // Configure egress receiver in AppState
        app_state.set_egress_receiver(egress_receiver);
        
        let app = create_router(app_state);

        // Start the server
        let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
        info!("Matching Engine Service listening on http://0.0.0.0:3000");
        info!("Available endpoints:");
        info!("  POST /buy   - Submit buy orders");
        info!("  POST /sell  - Submit sell orders");
        info!("  POST /health - Health check");
        info!("  POST /symbol - Create new symbol");
        info!("Supported symbols: {:?}", symbols);
        info!("Thread Summary:");
        info!("  - {} shard threads (pinned to cores)", symbols.len());
        info!("  - {} ingress workers (not pinned)", NUM_INGRESS_WORKERS);
        info!("  - {} egress workers (not pinned)", NUM_EGRESS_WORKERS);

        if let Err(e) = axum::serve(listener, app).await {
            error!("Server error: {}", e);
        }

        // Note: The code below is unreachable in normal operation since axum::serve runs indefinitely
        // If the server exits, the worker threads will be terminated when the process ends
        info!("Server stopped, worker threads will be cleaned up on process exit");
    });

    info!("Matching Engine Service shutting down");
}