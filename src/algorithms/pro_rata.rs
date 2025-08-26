use std::collections::VecDeque;
use crate::engine::{Order, Side};
use crate::algorithms::fifo::Trade;

pub struct ProRataMatcher {
    pub bids: VecDeque<Order>,
    pub asks: VecDeque<Order>,
}

impl ProRataMatcher {
    pub fn new() -> Self {
        Self {
            bids: VecDeque::new(),
            asks: VecDeque::new(),
        }
    }

    pub fn match_order(&mut self, incoming: Order) -> Vec<Trade> {
        if incoming.quantity == 0 || incoming.price <= 0.0 {
            return Vec::new();
        }

        match incoming.side {
            Side::Buy => self.match_buy_order(incoming),
            Side::Sell => self.match_sell_order(incoming),
        }
    }

    fn match_buy_order(&mut self, mut incoming_buy: Order) -> Vec<Trade> {
        let mut trades = Vec::new();

        if self.asks.is_empty() {
            self.bids.push_back(incoming_buy);
            return trades;
        }

        let best_ask_price = self.asks.front().unwrap().price;
        if incoming_buy.price < best_ask_price {
            self.bids.push_back(incoming_buy);
            return trades;
        }

        // Find all orders at the best price level
        let mut matching_orders: Vec<(usize, u64)> = Vec::new();
        let mut total_resting_quantity = 0u64;

        for (index, ask) in self.asks.iter().enumerate() {
            if ask.price == best_ask_price {
                matching_orders.push((index, ask.quantity));
                total_resting_quantity += ask.quantity;
            } else {
                break;
            }
        }

        if total_resting_quantity == 0 {
            self.bids.push_back(incoming_buy);
            return trades;
        }

        // Calculate proportional allocations
        let available_quantity = std::cmp::min(incoming_buy.quantity, total_resting_quantity);
        let mut allocations: Vec<(usize, u64)> = Vec::new();
        let mut total_allocated = 0u64;

        for (index, resting_qty) in &matching_orders {
            let proportion = (*resting_qty as f64) / (total_resting_quantity as f64);
            let allocated = (proportion * available_quantity as f64).floor() as u64;
            allocations.push((*index, allocated));
            total_allocated += allocated;
        }

        // Handle remainder
        let mut remainder = available_quantity - total_allocated;
        let mut allocation_idx = 0;
        while remainder > 0 && allocation_idx < allocations.len() {
            allocations[allocation_idx].1 += 1;
            remainder -= 1;
            allocation_idx += 1;
        }

        // Execute trades
        allocations.sort_by(|a, b| b.0.cmp(&a.0));
        for (index, allocated_qty) in allocations {
            if allocated_qty > 0 {
                let mut ask_order = self.asks.remove(index).unwrap();
                let trade = Trade::new(
                    incoming_buy.id,
                    ask_order.id,
                    ask_order.price,
                    allocated_qty,
                );
                trades.push(trade);
                incoming_buy.quantity -= allocated_qty;
                ask_order.quantity -= allocated_qty;
                if ask_order.quantity > 0 {
                    self.asks.insert(index, ask_order);
                }
            }
        }

        if incoming_buy.quantity > 0 {
            self.bids.push_back(incoming_buy);
        }

        trades
    }

    fn match_sell_order(&mut self, mut incoming_sell: Order) -> Vec<Trade> {
        let mut trades = Vec::new();

        if self.bids.is_empty() {
            self.asks.push_back(incoming_sell);
            return trades;
        }

        let best_bid_price = self.bids.front().unwrap().price;
        if incoming_sell.price > best_bid_price {
            self.asks.push_back(incoming_sell);
            return trades;
        }

        // Find all orders at the best price level
        let mut matching_orders: Vec<(usize, u64)> = Vec::new();
        let mut total_resting_quantity = 0u64;

        for (index, bid) in self.bids.iter().enumerate() {
            if bid.price == best_bid_price {
                matching_orders.push((index, bid.quantity));
                total_resting_quantity += bid.quantity;
            } else {
                break;
            }
        }

        if total_resting_quantity == 0 {
            self.asks.push_back(incoming_sell);
            return trades;
        }

        // Calculate proportional allocations
        let available_quantity = std::cmp::min(incoming_sell.quantity, total_resting_quantity);
        let mut allocations: Vec<(usize, u64)> = Vec::new();
        let mut total_allocated = 0u64;

        for (index, resting_qty) in &matching_orders {
            let proportion = (*resting_qty as f64) / (total_resting_quantity as f64);
            let allocated = (proportion * available_quantity as f64).floor() as u64;
            allocations.push((*index, allocated));
            total_allocated += allocated;
        }

        // Handle remainder
        let mut remainder = available_quantity - total_allocated;
        let mut allocation_idx = 0;
        while remainder > 0 && allocation_idx < allocations.len() {
            allocations[allocation_idx].1 += 1;
            remainder -= 1;
            allocation_idx += 1;
        }

        // Execute trades
        allocations.sort_by(|a, b| b.0.cmp(&a.0));
        for (index, allocated_qty) in allocations {
            if allocated_qty > 0 {
                let mut bid_order = self.bids.remove(index).unwrap();
                let trade = Trade::new(
                    bid_order.id,
                    incoming_sell.id,
                    bid_order.price,
                    allocated_qty,
                );
                trades.push(trade);
                incoming_sell.quantity -= allocated_qty;
                bid_order.quantity -= allocated_qty;
                if bid_order.quantity > 0 {
                    self.bids.insert(index, bid_order);
                }
            }
        }

        if incoming_sell.quantity > 0 {
            self.asks.push_back(incoming_sell);
        }

        trades
    }

    pub fn best_bid(&self) -> Option<&Order> {
        self.bids.front()
    }

    pub fn best_ask(&self) -> Option<&Order> {
        self.asks.front()
    }

    pub fn bid_depth(&self) -> usize {
        self.bids.len()
    }

    pub fn ask_depth(&self) -> usize {
        self.asks.len()
    }

    pub fn clear(&mut self) {
        self.bids.clear();
        self.asks.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.bids.is_empty() && self.asks.is_empty()
    }

    pub fn bids_iter(&self) -> impl Iterator<Item = &Order> {
        self.bids.iter()
    }

    pub fn asks_iter(&self) -> impl Iterator<Item = &Order> {
        self.asks.iter()
    }
}

impl Default for ProRataMatcher {
    fn default() -> Self {
        Self::new()
    }
}
