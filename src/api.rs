use crate::types::{Event, OrderIn, Side};
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::post,
    Router,
};
use crossbeam_channel::Sender;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tracing::{debug, error};

#[derive(Clone)]
pub struct AppState {
    pub ingress_sender: Sender<Event>,
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

    // Create event
    let event = Event::new_order(side, order.price, order.qty, order.symbol.clone());

    // Send to ingress channel
    match state.ingress_sender.send(event) {
        Ok(_) => {
            debug!("Successfully sent {:?} order for symbol '{}'", side, order.symbol);
            Ok(Json(json!({
                "status": "accepted",
                "side": side,
                "symbol": order.symbol,
                "price": order.price,
                "qty": order.qty
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
            "Kathir".to_string(),
        ]
    }

    pub fn new(ingress_sender: Sender<Event>) -> Self {
        let initial_symbols: HashSet<String> = Self::get_initial_symbols().into_iter().collect();
        
        Self {
            ingress_sender,
            valid_symbols: Arc::new(Mutex::new(initial_symbols)),
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
