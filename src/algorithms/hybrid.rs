use std::collections::VecDeque;
use crate::engine::{Order, Side};
use crate::algorithms::fifo::Trade;

pub struct HybridConfig {
    pub fifo_percentage: f64,
}

impl Default for HybridConfig {
    fn default() -> Self {
        Self {
            fifo_percentage: 0.5,
        }
    }
}

pub struct HybridMatcher {
    pub bids: VecDeque<Order>,
    pub asks: VecDeque<Order>,
    pub config: HybridConfig,
}

impl HybridMatcher {
    pub fn new() -> Self {
        Self {
            bids: VecDeque::new(),
            asks: VecDeque::new(),
            config: HybridConfig::default(),
        }
    }

    pub fn new_with_config(config: HybridConfig) -> Self {
        Self {
            bids: VecDeque::new(),
            asks: VecDeque::new(),
            config,
        }
    }

    pub fn match_order(&mut self, incoming: Order) -> Vec<Trade> {
        if incoming.quantity == 0 || incoming.price <= 0.0 {
            return Vec::new();
        }

        if self.config.fifo_percentage < 0.0 || self.config.fifo_percentage > 1.0 {
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

        let total_quantity = incoming_buy.quantity;
        let fifo_quantity = (total_quantity as f64 * self.config.fifo_percentage).floor() as u64;
        let pro_rata_quantity = total_quantity - fifo_quantity;

        // FIFO portion
        if fifo_quantity > 0 {
            let mut fifo_order = incoming_buy.clone();
            fifo_order.quantity = fifo_quantity;
            self.apply_fifo_matching_buy(&mut fifo_order, &mut trades, best_ask_price);
            incoming_buy.quantity -= fifo_quantity - fifo_order.quantity;
        }

        // Pro-rata portion
        if pro_rata_quantity > 0 && !self.asks.is_empty() {
            let mut pro_rata_order = incoming_buy.clone();
            pro_rata_order.quantity = pro_rata_quantity;
            self.apply_pro_rata_matching_buy(&mut pro_rata_order, &mut trades, best_ask_price);
            incoming_buy.quantity -= pro_rata_quantity - pro_rata_order.quantity;
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

        let total_quantity = incoming_sell.quantity;
        let fifo_quantity = (total_quantity as f64 * self.config.fifo_percentage).floor() as u64;
        let pro_rata_quantity = total_quantity - fifo_quantity;

        // FIFO portion
        if fifo_quantity > 0 {
            let mut fifo_order = incoming_sell.clone();
            fifo_order.quantity = fifo_quantity;
            self.apply_fifo_matching_sell(&mut fifo_order, &mut trades, best_bid_price);
            incoming_sell.quantity -= fifo_quantity - fifo_order.quantity;
        }

        // Pro-rata portion
        if pro_rata_quantity > 0 && !self.bids.is_empty() {
            let mut pro_rata_order = incoming_sell.clone();
            pro_rata_order.quantity = pro_rata_quantity;
            self.apply_pro_rata_matching_sell(&mut pro_rata_order, &mut trades, best_bid_price);
            incoming_sell.quantity -= pro_rata_quantity - pro_rata_order.quantity;
        }

        if incoming_sell.quantity > 0 {
            self.asks.push_back(incoming_sell);
        }

        trades
    }

    fn apply_fifo_matching_buy(&mut self, incoming_buy: &mut Order, trades: &mut Vec<Trade>, target_price: f64) {
        while !incoming_buy.is_empty() && !self.asks.is_empty() {
            let front_ask = self.asks.front().unwrap();
            if front_ask.price != target_price {
                break;
            }

            let mut ask_order = self.asks.pop_front().unwrap();
            let trade_quantity = std::cmp::min(incoming_buy.quantity, ask_order.quantity);

            let trade = Trade::new(
                incoming_buy.id,
                ask_order.id,
                ask_order.price,
                trade_quantity,
            );
            trades.push(trade);

            incoming_buy.quantity -= trade_quantity;
            ask_order.quantity -= trade_quantity;

            if ask_order.quantity > 0 {
                self.asks.push_front(ask_order);
            }
        }
    }

    fn apply_fifo_matching_sell(&mut self, incoming_sell: &mut Order, trades: &mut Vec<Trade>, target_price: f64) {
        while !incoming_sell.is_empty() && !self.bids.is_empty() {
            let front_bid = self.bids.front().unwrap();
            if front_bid.price != target_price {
                break;
            }

            let mut bid_order = self.bids.pop_front().unwrap();
            let trade_quantity = std::cmp::min(incoming_sell.quantity, bid_order.quantity);

            let trade = Trade::new(
                bid_order.id,
                incoming_sell.id,
                bid_order.price,
                trade_quantity,
            );
            trades.push(trade);

            incoming_sell.quantity -= trade_quantity;
            bid_order.quantity -= trade_quantity;

            if bid_order.quantity > 0 {
                self.bids.push_front(bid_order);
            }
        }
    }

    fn apply_pro_rata_matching_buy(&mut self, incoming_buy: &mut Order, trades: &mut Vec<Trade>, target_price: f64) {
        let mut matching_orders: Vec<(usize, u64)> = Vec::new();
        let mut total_resting_quantity = 0u64;

        for (index, ask) in self.asks.iter().enumerate() {
            if ask.price == target_price {
                matching_orders.push((index, ask.quantity));
                total_resting_quantity += ask.quantity;
            } else {
                break;
            }
        }

        if total_resting_quantity == 0 {
            return;
        }

        let available_quantity = std::cmp::min(incoming_buy.quantity, total_resting_quantity);
        let mut allocations: Vec<(usize, u64)> = Vec::new();
        let mut total_allocated = 0u64;

        for (index, resting_qty) in &matching_orders {
            let proportion = (*resting_qty as f64) / (total_resting_quantity as f64);
            let allocated = (proportion * available_quantity as f64).floor() as u64;
            allocations.push((*index, allocated));
            total_allocated += allocated;
        }

        let mut remainder = available_quantity - total_allocated;
        let mut allocation_idx = 0;
        while remainder > 0 && allocation_idx < allocations.len() {
            allocations[allocation_idx].1 += 1;
            remainder -= 1;
            allocation_idx += 1;
        }

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
    }

    fn apply_pro_rata_matching_sell(&mut self, incoming_sell: &mut Order, trades: &mut Vec<Trade>, target_price: f64) {
        let mut matching_orders: Vec<(usize, u64)> = Vec::new();
        let mut total_resting_quantity = 0u64;

        for (index, bid) in self.bids.iter().enumerate() {
            if bid.price == target_price {
                matching_orders.push((index, bid.quantity));
                total_resting_quantity += bid.quantity;
            } else {
                break;
            }
        }

        if total_resting_quantity == 0 {
            return;
        }

        let available_quantity = std::cmp::min(incoming_sell.quantity, total_resting_quantity);
        let mut allocations: Vec<(usize, u64)> = Vec::new();
        let mut total_allocated = 0u64;

        for (index, resting_qty) in &matching_orders {
            let proportion = (*resting_qty as f64) / (total_resting_quantity as f64);
            let allocated = (proportion * available_quantity as f64).floor() as u64;
            allocations.push((*index, allocated));
            total_allocated += allocated;
        }

        let mut remainder = available_quantity - total_allocated;
        let mut allocation_idx = 0;
        while remainder > 0 && allocation_idx < allocations.len() {
            allocations[allocation_idx].1 += 1;
            remainder -= 1;
            allocation_idx += 1;
        }

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

impl Default for HybridMatcher {
    fn default() -> Self {
        Self::new()
    }
}
