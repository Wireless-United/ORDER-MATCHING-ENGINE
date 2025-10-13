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
use std::thread;
use tracing::{debug, error, info};

#[derive(Clone)]
pub struct AppState {
    pub ingress_sender: Sender<Event>,
    pub egress_receiver: Arc<Mutex<Option<Receiver<Trade>>>>,
    pub valid_symbols: Arc<Mutex<HashSet<String>>>,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/buy", post(buy_handler))
        .route("/sell", post(sell_handler))
        .route("/health", post(health_handler))
        .route("/symbol", post(create_symbol_handler))
        .with_state(state)
}

async fn buy_handler(
    State(state): State<AppState>,
    Json(order): Json<OrderIn>,
) -> Result<Json<Value>, StatusCode> {
    handle_order(state, order, Side::BUY).await
}

async fn sell_handler(
    State(state): State<AppState>,
    Json(order): Json<OrderIn>,
) -> Result<Json<Value>, StatusCode> {
    handle_order(state, order, Side::SELL).await
}


async fn create_symbol_handler(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    create_symbol(state, body).await
}

async fn handle_order(
    state: AppState,
    order: OrderIn,
    side: Side,
) -> Result<Json<Value>, StatusCode> {
    debug!("Received {:?} order: {:?}", side, order);

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
            debug!("Successfully sent {:?} order for symbol '{}'", side, order.symbol);
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
    debug!("Received request to create symbol: {:?}", body);

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
            debug!("Successfully added symbol: {}", symbol);
            
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
