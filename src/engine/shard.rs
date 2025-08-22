// shard.rs
use std::collections::VecDeque;
use crate::order::Order;

pub struct Shard {
    pub id: usize,
    pub symbol: String,
    pub orders: VecDeque<Order>, // lock-free alternatives can come later
}

impl Shard {
    pub fn new(id: usize, symbol: &str) -> Self {
        Self {
            id,
            symbol: symbol.to_string(),
            orders: VecDeque::new(),
        }
    }

    pub fn add_order(&mut self, order: Order) {
        self.orders.push_back(order);
    }

    pub fn process(&mut self) {
        // In a real engine, you'd do order matching here
        while let Some(order) = self.orders.pop_front() {
            println!("Shard {} processing order {:?}", self.id, order);
        }
    }
}
