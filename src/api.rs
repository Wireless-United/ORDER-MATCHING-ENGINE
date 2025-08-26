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
use tracing::{debug, error};

#[derive(Clone)]
pub struct AppState {
    pub ingress_sender: Sender<Event>,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/buy", post(buy_handler))
        .route("/sell", post(sell_handler))
        .route("/health", post(health_handler))
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

async fn handle_order(
    state: AppState,
    order: OrderIn,
    side: Side,
) -> Result<Json<Value>, StatusCode> {
    debug!("Received {:?} order: {:?}", side, order);

    // Validate the symbol
    if !is_valid_symbol(&order.symbol) {
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

fn is_valid_symbol(symbol: &str) -> bool {
    matches!(symbol, "Pranesh" | "Superman" | "Arnimzola")
}
