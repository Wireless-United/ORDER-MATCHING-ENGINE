// order.rs
#[derive(Debug, Clone, PartialEq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct Order {
    pub id: u64,
    pub symbol: String,
    pub side: Side,
    pub price: f64,
    pub qty: u64,
}

impl Order {
    pub fn new(id: u64, symbol: &str, side: Side, price: f64, qty: u64) -> Self {
        Self {
            id,
            symbol: symbol.to_string(),
            side,
            price,
            qty,
        }
    }
}
