use crate::types::{Event, OrderIn, Side, Trade};
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::post,
    Router,
};

use crossbeam_channel::{Receiver, Sender};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
use std::thread;
use tracing::{debug, error, info};

use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncBufReadExt, BufReader};
use tokio::task::JoinHandle;

#[derive(Clone)]
pub struct AppState {
    pub ingress_sender: Sender<Event>,
    pub egress_receiver: Arc<Mutex<Option<Receiver<Trade>>>>,
    pub valid_symbols: Arc<Mutex<HashSet<String>>>,
}

pub fn create_router(state: AppState) -> Router {
    // Spawn FIX listener in the background to accept FIX messages over TCP
    // FIX will be accepted on 0.0.0.0:9878 by default. We spawn a Tokio task
    // so the existing HTTP server (if any) can still run alongside it.
    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(e) = start_fix_listener(state_clone, "0.0.0.0:9878").await {
            error!("FIX listener exited with error: {}", e);
        }
    });

    // Keep minimal HTTP endpoints for health and symbol management so callers
    // that expect an axum Router (like main.rs) continue to work.
    Router::new()
        .route("/health", post(health_handler))
        .route("/symbol", post(create_symbol_handler))
        .with_state(state)
}

async fn create_symbol_handler(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    create_symbol(state, body).await
}

// Note: Order ingress is handled by the FIX listener. The HTTP handlers were
// removed for /buy and /sell to enforce FIX-based API usage. Symbol and
// health APIs remain.

async fn handle_order(
    state: AppState,
    order: OrderIn,
    side: Side,
) -> Result<Json<Value>, StatusCode> {
    // Validate the symbol
    if !is_valid_symbol(&state.valid_symbols, &order.symbol) {
        error!("Invalid symbol: {}", order.symbol);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate price and quantity
    if order.price == 0 || order.qty == 0 {
        error!("Invalid price or quantity: price={}, qty={}", order.price, order.qty);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Create event with specified algorithm (defaults to Hierarchical if not provided)
    let event = Event::new_order_with_algorithm(
        side,
        order.price,
        order.qty,
        order.symbol.clone(),
        order.algorithm,
    );

    // Send to ingress channel
    match state.ingress_sender.send(event.clone()) {
        Ok(_) => {
            Ok(Json(json!({
                "status": "accepted",
                "side": format!("{:?}", side),
                "symbol": order.symbol,
                "price": order.price,
                "qty": order.qty,
                "order_id": event.order_id,
                "algorithm": format!("{:?}", order.algorithm),
                "timestamp": event.timestamp.to_rfc3339()
            })))
        }
        Err(_) => {
            error!("Failed to send order to ingress channel");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}


async fn health_handler() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "matching-engine"
    }))
}

// ================================================================================================
// FIX LISTENER
// ================================================================================================

/// Start a simple FIX TCP listener which accepts FIX messages and converts
/// NewOrderSingle (35=D) into internal Events. This is intentionally simple
/// and forgiving: it accepts both SOH (\x01) and pipe '|' separators and
/// extracts the common tags used by NewOrderSingle:
/// - 35 = MsgType (expect 'D')
/// - 54 = Side (1=Buy, 2=Sell)
/// - 44 = Price
/// - 38 = OrderQty
/// - 55 = Symbol
async fn start_fix_listener(state: AppState, addr: &str) -> Result<(), String> {
    info!("Starting FIX listener on {}", addr);

    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Failed to bind FIX listener: {}", e))?;

    loop {
        let (socket, peer) = listener
            .accept()
            .await
            .map_err(|e| format!("Failed to accept connection: {}", e))?;

        info!("Accepted FIX connection from {}", peer);
        let state_conn = state.clone();

        // Spawn a task per connection
        tokio::spawn(async move {
            if let Err(e) = handle_fix_connection(state_conn, socket).await {
                error!("Error in FIX connection handler: {}", e);
            }
        });
    }
}

async fn handle_fix_connection(state: AppState, socket: tokio::net::TcpStream) -> Result<(), String> {
    let mut reader = BufReader::new(socket);
    let mut buf = Vec::new();

    loop {
        buf.clear();
        // Read until newline or until EOF; FIX messages often don't include newlines,
        // but many clients send messages followed by a newline; we accept both.
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) => {
                // Connection closed
                info!("FIX connection closed by peer");
                return Ok(());
            }
            Ok(_) => {
                let raw = String::from_utf8_lossy(&buf).to_string();
                // Try to parse possibly multiple FIX messages in the buffer
                for msg in split_fix_messages(&raw) {
                    if let Some(event) = parse_fix_message(&msg) {
                        // Send to ingress channel
                        if let Err(e) = state.ingress_sender.send(event.clone()) {
                            error!("Failed to send FIX-derived event: {:?}", e);
                        } else {
                            info!("Accepted FIX order: {:?}", event);
                        }
                    } else {
                        debug!("Ignored non-order or unparsable FIX msg: {}", msg);
                    }
                }
            }
            Err(e) => {
                return Err(format!("Failed to read from FIX socket: {}", e));
            }
        }
    }
}

/// Split a raw buffer into candidate FIX messages. Accepts either SOH (\x01)
/// separated messages or '|' separated messages. This helper returns message
/// strings with internal separators kept as '|' for simpler parsing.
fn split_fix_messages(raw: &str) -> Vec<String> {
    // Normalize: replace SOH with '|' then split on double-10 (end of checksum is often tag 10)
    let normalized = raw.replace('\x01', "|");
    // Some FIX senders include trailing newline; split by newline first
    let mut parts = Vec::new();
    for line in normalized.lines() {
        let candidate = line.trim();
        if candidate.is_empty() {
            continue;
        }
        // There can be multiple messages in a single line separated by '|' and ending with tag 10
        // We'll treat each substring that contains an 8= (BeginString) and 10= as a message
        if candidate.contains("8=") && candidate.contains("10=") {
            parts.push(candidate.to_string());
        } else {
            // If it's not a full FIX message, still push it so parser can try
            parts.push(candidate.to_string());
        }
    }

    parts
}

/// Parse a single FIX message (with '|' separators) and return an Event if it's
/// a NewOrderSingle (35=D) with required fields.
fn parse_fix_message(msg: &str) -> Option<Event> {
    // Build a map of tags
    let mut tags = std::collections::HashMap::new();
    for kv in msg.split('|') {
        if kv.is_empty() {
            continue;
        }
        if let Some(pos) = kv.find('=') {
            let (k, v) = kv.split_at(pos);
            let v = &v[1..];
            tags.insert(k.to_string(), v.to_string());
        }
    }

    // MsgType
    let msg_type = tags.get("35")?.as_str();
    if msg_type != "D" {
        // Not a NewOrderSingle
        return None;
    }

    // Required fields: 54 (Side), 44 (Price), 38 (OrderQty), 55 (Symbol)
    let side_tag = tags.get("54")?;
    let price_tag = tags.get("44")?;
    let qty_tag = tags.get("38")?;
    let symbol_tag = tags.get("55")?;

    let side = match side_tag.as_str() {
        "1" => Side::BUY,
        "2" => Side::SELL,
        _ => return None,
    };

    // Price may be decimal; we'll parse as f64 then convert to u64 by rounding.
    let price = match price_tag.parse::<f64>() {
        Ok(p) => p.round() as u64,
        Err(_) => return None,
    };

    let qty = match qty_tag.parse::<u64>() {
        Ok(q) => q,
        Err(_) => return None,
    };

    let symbol = symbol_tag.clone();

    let event = Event::new_order_with_algorithm(side, price, qty, symbol, crate::types::MatchingAlgorithm::default());
    Some(event)
}

fn is_valid_symbol(valid_symbols: &Arc<Mutex<HashSet<String>>>, symbol: &str) -> bool {
    match valid_symbols.lock() {
        Ok(symbols) => symbols.contains(symbol),
        Err(_) => {
            error!("Failed to acquire lock on valid_symbols");
            false
        }
    }
}

impl AppState {
    pub fn get_initial_symbols() -> Vec<String> {
        vec![
            "Pranesh".to_string(),
            "Superman".to_string(),
            "Arnimzola".to_string(),
            "Kathir".to_string(),
        ]
    }

    pub fn new(ingress_sender: Sender<Event>) -> Self {
        let initial_symbols: HashSet<String> = Self::get_initial_symbols().into_iter().collect();
        
        Self {
            ingress_sender,
            egress_receiver: Arc::new(Mutex::new(None)),
            valid_symbols: Arc::new(Mutex::new(initial_symbols)),
        }
    }

    pub fn set_egress_receiver(&self, receiver: Receiver<Trade>) {
        if let Ok(mut egress) = self.egress_receiver.lock() {
            *egress = Some(receiver);
            info!("Egress receiver configured in AppState");
        }
    }
}

async fn create_symbol(state: AppState, body: Value) -> Result<Json<Value>, StatusCode> {
    let symbol = match body["symbol"].as_str() {
        Some(s) => s.to_string(),
        None => {
            error!("Missing or invalid 'symbol' field in request body");
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    if symbol.is_empty() {
        error!("Symbol cannot be empty");
        return Err(StatusCode::BAD_REQUEST);
    }

    match state.valid_symbols.lock() {
        Ok(mut symbols) => {
            if symbols.contains(&symbol) {
                return Ok(Json(json!({
                    "status": "symbol already exists",
                    "symbol": symbol
                })));
            }
            
            symbols.insert(symbol.clone());
            
            Ok(Json(json!({
                "status": "symbol created",
                "symbol": symbol
            })))
        }
        Err(_) => {
            error!("Failed to acquire lock on valid_symbols");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ================================================================================================
// EGRESS THREAD IMPLEMENTATION
// ================================================================================================
//
// The egress thread system provides a mechanism to process trade outputs from the matching engine.
// This is the reverse flow of the ingress system:
//
// INGRESS FLOW:  HTTP API → Ingress Channel → Fabric → Shard Queues → Shards
// EGRESS FLOW:   Shards → Egress Channel → Egress Workers → External Systems
//
// USAGE IN MAIN.RS:
// 
// 1. Create egress channel:
//    let (egress_sender, egress_receiver) = unbounded::<Trade>();
//
// 2. Pass egress_sender to each shard so they can publish trades
//
// 3. Configure AppState with egress receiver:
//    let app_state = AppState::new(ingress_sender);
//    app_state.set_egress_receiver(egress_receiver.clone());
//
// 4. Spawn egress workers:
//    let egress_handles = spawn_egress_workers(egress_receiver, 3); // 3 workers
//
// 5. Egress workers will automatically process all trades and can:
//    - Publish to message queues (Kafka, RabbitMQ, Redis)
//    - Store in databases (PostgreSQL, MongoDB)
//    - Send via WebSocket to connected clients
//    - Update real-time analytics/metrics
//    - Trigger notifications or alerts
//
// ================================================================================================

/// Spawn egress worker threads to process trade outputs
pub fn spawn_egress_workers(egress_receiver: Receiver<Trade>, num_workers: usize) -> Vec<thread::JoinHandle<()>> {
    let mut handles = Vec::new();

    for worker_id in 0..num_workers {
        let receiver_clone = egress_receiver.clone();
        
        let handle = thread::Builder::new()
            .name(format!("egress-{}", worker_id))
            .spawn(move || {
                info!("Egress worker {} started (not pinned to specific core)", worker_id);
                run_egress_worker(receiver_clone, worker_id);
            })
            .expect("Failed to create egress worker thread");
        
        handles.push(handle);
    }

    info!("Spawned {} egress worker threads", num_workers);
    handles
}

/// Main egress worker loop - processes trades from shards
fn run_egress_worker(receiver: Receiver<Trade>, worker_id: usize) {
    info!(
        "Egress worker {} started on thread '{}'",
        worker_id,
        std::thread::current().name().unwrap_or("unnamed")
    );

    loop {
        match receiver.recv() {
            Ok(trade) => {
                debug!("Egress worker {} received trade: {:?}", worker_id, trade);
                process_trade_output(trade, worker_id);
            }
            Err(_) => {
                debug!("Egress channel closed for worker {}", worker_id);
                break;
            }
        }
    }

    info!("Egress worker {} shutting down", worker_id);
}

/// Process individual trade - can be extended for publishing to external systems
fn process_trade_output(trade: Trade, worker_id: usize) {
    info!(
        "Egress worker {} processing trade {}: Buy {} & Sell {} matched {} @ {} at {}",
        worker_id,
        trade.trade_id,
        trade.buy_order_id,
        trade.sell_order_id,
        trade.qty,
        trade.price,
        trade.timestamp.to_rfc3339()
    );
    
    debug!("Trade {} successfully processed by egress worker {}", trade.trade_id, worker_id);
}
