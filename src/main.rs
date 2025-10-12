mod api;
mod fabric;
mod shard;
mod types;
mod fix;

use fix::{start_fix_acceptor, FixState};
use fabric::Fabric;
use shard::Shard;
use types::Event;

use crossbeam_channel::{unbounded, Receiver, Sender};
use crossbeam_queue::ArrayQueue;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use tracing::{error, info, warn};
use tracing_subscriber;

const QUEUE_CAPACITY: usize = 1000;
const NUM_INGRESS_WORKERS: usize = 5; // default, can be overridden via CLI

fn check_cpu_requirements(symbols: &[String]) -> Result<Vec<core_affinity::CoreId>, String> {
    let core_ids = core_affinity::get_core_ids().unwrap_or_default();
    let num_cores = core_ids.len();
    let required_cores = symbols.len(); // Only check for shard cores

    info!("System has {} CPU cores available", num_cores);
    info!("Required cores: {} symbols (shards only)", symbols.len());

    if num_cores < required_cores {
        return Err(format!(
            "Insufficient CPU cores! Available: {}, Required: {} (shards only)",
            num_cores, required_cores
        ));
    }

    Ok(core_ids)
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

    // Default configuration
    let mut ingress_workers = NUM_INGRESS_WORKERS;
    let mut port: u16 = 8888;
    let mut symbols = vec![
        "AAPL".to_string(),
        "GOOGL".to_string(), 
        "MSFT".to_string(),
        "TSLA".to_string(),
    ];

    // Simple CLI parsing (overrides defaults)
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--ingress-workers" | "-w" => {
                if let Some(val) = args.get(i+1) {
                    if let Ok(n) = val.parse::<usize>() {
                        ingress_workers = n;
                    }
                }
                i += 2;
            }
            "--port" | "-p" => {
                if let Some(val) = args.get(i+1) {
                    if let Ok(n) = val.parse::<u16>() {
                        port = n;
                    }
                }
                i += 2;
            }
            "--symbols" | "-s" => {
                if let Some(val) = args.get(i+1) {
                    let parts: Vec<String> = val.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                    if !parts.is_empty() {
                        symbols = parts;
                    }
                }
                i += 2;
            }
            _ => {
                // unknown arg; skip it
                i += 1;
            }
        }
    }

    info!("Using symbols: {:?}", symbols);
    info!("Ingress workers: {} (can be overridden with --ingress-workers/-w)", ingress_workers);
    info!("FIX acceptor port: {} (can be overridden with --port/-p)", port);

    // Check CPU core requirements (only for shards)
    let core_ids = match check_cpu_requirements(&symbols) {
        Ok(cores) => cores,
        Err(err) => {
            error!("{}", err);
            std::process::exit(1);
        }
    };

    // Allocate cores only to shards
    let shard_cores = allocate_shard_cores(&core_ids, &symbols);

    // Create the ingress channel for routing orders
    let (ingress_sender, ingress_receiver): (Sender<Event>, Receiver<Event>) = unbounded();

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

    // Spawn ingress worker threads WITHOUT core pinning
    let mut ingress_handles = Vec::new();
    for worker_id in 0..ingress_workers {
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

    // Create FIX Protocol server
    let fix_state = FixState::new(ingress_sender);
    let bind_addr = format!("0.0.0.0:{}", port);
    let fix_handle = start_fix_acceptor(&bind_addr, fix_state);

    info!("Matching Engine Service with FIX Protocol listening on {}", bind_addr);
    info!("FIX Protocol Features:");
    info!("  - Session management (Logon/Logout)");
    info!("  - Order entry (NewOrderSingle)");
    info!("  - Order book queries (OrderStatusRequest)");
    info!("  - Symbol management (SecurityDefinitionRequest)");
    info!("  - Symbol listing (SecurityListRequest)");
    info!("  - Heartbeat and Test Request support");
    info!("Default symbols: {:?}", symbols);

    // Wait for FIX acceptor to complete (this won't happen in normal operation)
    if let Err(e) = fix_handle.await {
        error!("FIX acceptor error: {:?}", e);
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