# High-Throughput NEAR FT Transfer Service

## 1. Project Overview

This project is a high-throughput API service designed to meet the demands of a NEAR bounty for handling **100+ fungible token (FT) transfers per second**. It is engineered for performance, scalability, and reliability by leveraging advanced asynchronous patterns in Rust, transaction batching, and a decoupled persistence layer.

The core challenge is to overcome the sequential nature of transaction nonces on a single account key, which traditionally limits throughput. This service solves that by implementing a sophisticated architecture involving concurrent transaction submission and a robust queuing system.

---

## 2. Core Architecture & Design Philosophy

The service is built on a decoupled, multi-layered architecture to ensure that no single component becomes a bottleneck.

- **API Layer (Actix Web)**: A lightweight, asynchronous web server that ingests transfer requests. Its sole responsibility is to validate the request, assign it a unique ID, persist it to Redis with a `Queued` status, and push it into a work queue. It responds to the client immediately, ensuring low latency.
- **Queueing Layer (Tokio MPSC Channel)**: An in-memory, multi-producer, single-consumer (MPSC) channel that acts as a buffer between the fast API layer and the batch-processing worker. This prevents backpressure on the API during high traffic.
- **Persistence Layer (Redis)**: A high-speed, in-memory database used to track the lifecycle of every transfer request. It provides atomicity and durability for status updates without blocking the critical transaction submission path.
- **Worker Layer (Background Tokio Task)**: The engine of the service. This long-running background task consumes jobs from the queue, intelligently batches them, and submits them to the NEAR blockchain concurrently.

### Architectural Diagram

```text
+-----------+      +----------------+      +-----------------+      +--------------------+
|           |      |                |      |                 |      |                    |
|  Client   |----->|   Actix Web    |----->|  MPSC Channel   |----->|   Batching Worker  |
|           |      |   (API Layer)  |      | (Queueing Layer)|      |   (Worker Layer)   |
+-----------+      +-------+--------+      +-----------------+      +----------+---------+
                         |                                                    |
                         | 1. Create & Queue                                  | 3. Send to NEAR
                         v                                                    |
+------------------------+-----------------------+                            |
|                                                |                            |
|                Redis (Persistence)             |                            v
|                                                |                  +---------+----------+
|  - HSET txn:{id} {status: "Queued", ...}       |                  |                    |
|  - LPUSH user_txns:{receiver} {id}             |                  |  NEAR RPC Endpoint |
|                                                |                  |                    |
|  - HSET txn:{id} {status: "Success", ...} <----|------------------+                    |
|                                                | 4. Update Status |                    |
+------------------------------------------------+ (Non-blocking)   +--------------------+

```

---

## 3. Deep Dive: Achieving 100+ TPS

Here’s a detailed breakdown of the techniques used to achieve high throughput.

### a. Fully Non-Blocking I/O

Every operation, from handling HTTP requests to database queries and RPC calls, is fully asynchronous using Rust's `async/await` and the Tokio runtime.

- **API**: Actix Web is built on Tokio and handles thousands of concurrent connections with minimal overhead.
- **Database**: We use `deadpool-redis` to manage a pool of asynchronous Redis connections. This prevents the API from ever blocking while writing the initial transaction record.
- **Worker**: The worker's interaction with the NEAR RPC is asynchronous. Crucially, after the transaction is sent, the subsequent Redis status update is **spawned into a separate, non-blocking Tokio task**. This frees the main worker loop to immediately begin processing the next batch, effectively hiding the latency of database writes.

### b. Transaction Batching

Instead of sending one NEAR transaction per transfer request, the worker intelligently batches multiple FT transfers into a **single transaction**.

- **How it Works**: The worker pulls jobs from the MPSC channel. It waits for a short duration (`batch_timeout_secs`) or until a certain number of transfers (`batch_size`) have accumulated. It then constructs a single NEAR transaction and adds an `ft_transfer` action for each request in the batch.
- **Benefits**:
  1. **Reduced Gas Costs**: A single transaction with many actions is significantly cheaper than many individual transactions.
  2. **Higher Throughput**: It dramatically reduces the number of RPC calls and the overhead associated with transaction signing and propagation.
  3. **Atomic Execution**: All transfers within a batch either succeed or fail together (within the context of that single transaction), simplifying state management.

### c. Concurrency and Access Key Management

The primary bottleneck in sending many transactions from a single account is the sequential nonce. To overcome this, the service uses a pool of access keys.

- **The Strategy**: Instead of using the master account's full access key for every transaction, the service should be configured with a pool of function-call-only access keys. Each key has its own independent nonce, allowing for parallel transaction submission.
- **Implementation**:
  1. **Key Generation**: Before starting, a script should be run to generate a number of access keys (`num_pool_keys`) and add them to the master account. Each key is given a specific allowance (`key_allowance_near`) to pay for gas.
  2. **Signer Pool**: The `Signer` object in the service is configured with this pool of keys.
  3. **Concurrent Submission**: The worker uses a `Semaphore` to limit concurrency to a safe level (`concurrency`). When sending a batch, it acquires a permit from the semaphore and asks the `Signer` for the next available key. Since each key has its own nonce, multiple transactions can be "in-flight" to the RPC node simultaneously without conflict.

This parallel execution model is the key to scaling beyond the limits of a single key.

---

## 4. Developer Guide

### a. Prerequisites

- **Rust**: Install via [rustup](https://rustup.rs/).
- **Docker**: For containerized deployment.
- **Redis**: A running Redis instance.

### b. Configuration

1. **`.env` File**: Create a `.env` file for your secrets.

    ```env
    NEAR_ACCOUNT_ID="your-account.testnet"
    NEAR_MASTER_KEY="your 12-word seed phrase for the master account"
    REDIS_URL="redis://127.0.0.1/"
    ```

2. **`Settings.toml`**: Adjust performance parameters.

    ```toml
    rpc_url = "https://rpc.testnet.near.org"
    ft_contract_id = "token.testnet"
    account_id = "your-account.testnet"
    ft_decimals = 8
    batch_size = 10
    concurrency = 10 # Should be less than num_pool_keys
    ```

### c. Build and Run

**Local Development:**

```bash
# Build the project
cargo build --release

# Run the service
cargo run --release
```

**Docker Deployment:**

The `Dockerfile` uses a multi-stage build for a small, secure production image.

```bash
# 1. Build the image
docker build -t nearn-ft-service .

# 2. Run the container using your .env file
docker run -p 8000:8000 --env-file .env --name nearn-ft-api nearn-ft-service
```

---

## 5. API Reference

The API is documented via Swagger UI, available at `http://localhost:8000/`.

- `POST /transfer`: Accepts a transfer request, queues it for processing, and returns a unique transaction ID.
- `GET /transaction/{id}`: Fetches the status and details of a specific transaction by its ID.
- `GET /transactions/{receiver_id}`: Returns a paginated list of all transfers sent to a specific receiver.
- `GET /transactions`: Returns a paginated list of all transactions processed by the service.
