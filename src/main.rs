// main.rs - hey 3rd time
mod order;
mod shard;
mod affinity;

use order::{Order, Side};
use shard::Shard;
use std::thread;

fn main() {
    let mut shard0 = Shard::new(0, "AAPL");
    shard0.add_order(Order::new(1, "AAPL", Side::Buy, 150.0, 100));
    shard0.add_order(Order::new(2, "AAPL", Side::Sell, 151.0, 50));

    thread::spawn(move || {
        affinity::pin_to_core(0);
        let mut s = shard0;
        s.process();
    }).join().unwrap();
}
