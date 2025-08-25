use chrono::{DateTime, Utc};

/// Represents the side of an order in the matching engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// A buy order (bid)
    Buy,
    /// A sell order (ask) 
    Sell,
}

/// Represents a trading order with all necessary information for matching.
/// 
/// # Examples
/// 
/// ```rust
/// use order_matching_engine::engine::{Order, Side};
/// 
/// let buy_order = Order::new(1, Side::Buy, 100.50, 250);
/// assert!(buy_order.is_buy());
/// assert!(!buy_order.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Order {
    /// Unique identifier for this order
    pub id: u64,
    /// Whether this is a buy or sell order
    pub side: Side,
    /// Price per unit for this order
    pub price: f64,
    /// Quantity of units to trade
    pub quantity: u64,
    /// Timestamp when the order was created
    pub timestamp: DateTime<Utc>,
}

impl Order {
    /// Creates a new order with the current timestamp.
    /// 
    /// # Arguments
    /// 
    /// * `id` - Unique identifier for the order
    /// * `side` - Whether this is a buy or sell order
    /// * `price` - Price per unit (must be positive)
    /// * `quantity` - Number of units to trade (must be positive)
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use order_matching_engine::engine::{Order, Side};
    /// 
    /// let order = Order::new(1, Side::Buy, 99.95, 100);
    /// assert_eq!(order.id, 1);
    /// assert_eq!(order.quantity, 100);
    /// ```
    pub fn new(id: u64, side: Side, price: f64, quantity: u64) -> Self {
        Self {
            id,
            side,
            price,
            quantity,
            timestamp: Utc::now(),
        }
    }

    /// Returns true if this is a buy order.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use order_matching_engine::engine::{Order, Side};
    /// 
    /// let buy_order = Order::new(1, Side::Buy, 100.0, 50);
    /// assert!(buy_order.is_buy());
    /// 
    /// let sell_order = Order::new(2, Side::Sell, 100.0, 50);
    /// assert!(!sell_order.is_buy());
    /// ```
    pub fn is_buy(&self) -> bool {
        matches!(self.side, Side::Buy)
    }

    /// Returns true if this is a sell order.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use order_matching_engine::engine::{Order, Side};
    /// 
    /// let sell_order = Order::new(1, Side::Sell, 100.0, 50);
    /// assert!(sell_order.is_sell());
    /// 
    /// let buy_order = Order::new(2, Side::Buy, 100.0, 50);
    /// assert!(!buy_order.is_sell());
    /// ```
    pub fn is_sell(&self) -> bool {
        matches!(self.side, Side::Sell)
    }

    /// Returns true if the order has no remaining quantity.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use order_matching_engine::engine::{Order, Side};
    /// 
    /// let mut order = Order::new(1, Side::Buy, 100.0, 50);
    /// assert!(!order.is_empty());
    /// 
    /// order.quantity = 0;
    /// assert!(order.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.quantity == 0
    }

    /// Returns the total value of this order (price × quantity).
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use order_matching_engine::engine::{Order, Side};
    /// 
    /// let order = Order::new(1, Side::Buy, 10.50, 100);
    /// assert_eq!(order.total_value(), 1050.0);
    /// ```
    pub fn total_value(&self) -> f64 {
        self.price * self.quantity as f64
    }

    /// Reduces the order quantity by the specified amount.
    /// 
    /// # Arguments
    /// 
    /// * `amount` - Amount to reduce the quantity by
    /// 
    /// # Panics
    /// 
    /// Panics if amount is greater than current quantity.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use order_matching_engine::engine::{Order, Side};
    /// 
    /// let mut order = Order::new(1, Side::Buy, 100.0, 50);
    /// order.reduce_quantity(30);
    /// assert_eq!(order.quantity, 20);
    /// ```
    pub fn reduce_quantity(&mut self, amount: u64) {
        assert!(amount <= self.quantity, "Cannot reduce quantity by more than available");
        self.quantity -= amount;
    }
}
