# High-Performance Order Matching Engine

A concurrent and parallel order matching system implemented in Rust using lock-free data structures and modern computation engine
## Key Features

- **Lock-Free Communication**: Uses `ConcurrentQueue` for all inter-task communication - no mutexes
- **Async Orchestration**: Tokio runtime for async ingress and task coordination
- **Shard-Based Processing**: Distributes orders across multiple workers for scalability
- **Multiple Algorithms**: FIFO, Pro-Rata, and Hybrid matching strategies

## Building

```bash
cargo build --release
```

## Running

```bash
cargo run --release
```

## Testing

```bash
cargo test
```

## Project Structure

```
src/
  algorithms/     - Matching algorithms (FIFO, Pro-Rata, Hybrid)
  engine/         - Core engine components (Order, Request, Shard, Ingress)
  utils/          - Utility functions
```
