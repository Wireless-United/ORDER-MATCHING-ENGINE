# High-Performance Order Matching Engine

A concurrent and parallel order matching system implemented in Rust using lock-free data structures and modern async/parallel computation patterns.

## Architecture

```
Ingress Tasks (Tokio) → Lock-Free Queue → Shard Workers (Tokio) → Parallel Processing (Rayon)
```

## Key Features

- **Lock-Free Communication**: Uses `ConcurrentQueue` for all inter-task communication - no mutexes
- **Async Orchestration**: Tokio runtime for async ingress and task coordination
- **Parallel Compute**: Rayon for CPU-bound matching algorithm execution
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

## Configuration

Edit `main.rs` to configure:
- `NUM_SHARDS`: Number of parallel workers (default: 4)
- `NUM_INGRESS_TASKS`: Number of ingress tasks (default: 2)
- `ORDERS_PER_INGRESS`: Orders generated per task (default: 50)
  - Accessing **remote RAM** across sockets → slower  
- **Solution**:  
  - Pin threads to a socket  
  - Allocate memory (order book) in that socket’s RAM  
- **Example**:  
  - Socket 0 (Core 0–11 + 128GB RAM) → runs AAPL shard  
  - Socket 1 (Core 12–23 + 128GB RAM) → runs TSLA shard  

---

### 5. Kernel Bypassing with DPDK
- **What**: DPDK = Data Plane Development Kit.  
- **Why**: Skips the Linux kernel’s networking stack and lets user-space programs talk directly to the **network card (NIC)**.  
- **Benefit**: Drastically reduces packet processing latency — critical for order ingestion.

---

### 6. Remote Direct Memory Access (RDMA)
- **What**: Allows one machine to **read/write another’s memory directly** over the network, without CPU/OS involvement.  
- **Why**:  
  - Needed for **clustering multiple servers**.  
  - Enables ultra-fast state sharing across nodes (e.g., replicating order books for fault-tolerance).  

---


---
