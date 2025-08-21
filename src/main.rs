mod engine;
mod utils;

use crossbeam::queue::SegQueue;
use std::sync::Arc;

fn main() {
    let queue = Arc::new(SegQueue::new());

    // Example: spawn a shard for AAPL
    engine::shard::start_shard("AAPL".to_string(), queue.clone());

    // Later: more shards (TSLA, GOOG, etc.)
}
