# NEAR High-Throughput FT Transfer Service

This repository contains a reference implementation for a high-performance REST API service designed to distribute Fungible Tokens (FTs) on the NEAR Protocol.

Built in Rust, the service handles thousands of transfers per second, making it ideal for large-scale operations such as airdrops, token launches, and marketing campaigns.


---

# Railway Demo

> Note: These instances use free-tier infrastructure (limited performance).
For reliable testing, run locally or deploy on stronger infrastructure.
The demo was originally benchmarked on a MacBook.



Railway: https://nearn-ft.up.railway.app

Render: https://nearn-ft.onrender.com



---

⚙️ Core Features

High Performance: Over 100 transfers/second with optimized async architecture.

Asynchronous Processing: Requests are queued for processing, providing immediate responses.

Transaction Batching: Groups multiple ft_transfer calls into single transactions to save costs.

Nonce Collision Avoidance: Uses a dynamic key pool to eliminate nonce errors during heavy loads.

Persistence & Tracking: Redis-based transaction tracking with queryable endpoints.

Scalable Architecture: Built with Tokio, Actix-web, and Deadpool-Redis for concurrency.



---

🧩 Architecture Overview

The architecture separates the API layer from the on-chain transaction submission process for maximum throughput.

API Layer (Actix-web):
Receives POST /transfer requests.

Request Queuing (tokio::sync::mpsc):

Generates a unique transaction ID.

Records “Queued” status in Redis.

Pushes the request to an in-memory queue.

Responds with HTTP 202 Accepted.


Background Worker:
Batches requests (by size/time) and constructs on-chain transactions.

Key Pool Management:
Uses multiple full-access keys to sign transactions concurrently — preventing nonce collisions.

Asynchronous Status Updates:
Updates Redis with Success (and hash) or Failure (and error) after submission.



---

🔑 Solving the Nonce Collision Problem

Normally, sending thousands of transactions quickly from one NEAR account triggers InvalidNonce errors.

This service eliminates the issue via a key pool mechanism:

At startup, multiple full-access keys are generated and linked to the main account.

Each worker picks a key from the pool for signing, allowing parallel “in-flight” transactions.

Each key maintains an independent nonce, removing conflicts.


This approach ensures true parallelism and high reliability — preventing nonce failures instead of retrying them.


---

🧰 Getting Started

Prerequisites

Rust toolchain

Docker (for Redis) or a Redis instance

A NEAR account with a full-access key



---

1. Configuration

a. Settings.toml

Create a Settings.toml file in the root directory:

``rpc_urls = ["https://rpc.testnet.near.org"]
network = "testnet"
redis_url = "redis://127.0.0.1/"
account_id = "your-account.testnet"
ft_contract_id = "your-ft-contract.testnet"
ft_decimals = 18``

# cargo
batch_size = 20
batch_timeout_secs = 5
concurrency = 10
num_pool_keys = 15

b. .env

NEAR_MASTER_KEY="your-seed-phrase-or-private-key"


---

2. Running Locally

Start Redis

docker run -p 6379:6379 -d redis:alpine

Build & Run

cargo run --release 
Server starts on
👉 http://0.0.0.0:8080


---

📡 API Endpoints

Interactive docs:
http://localhost:8080/

Method	Endpoint	Description

``POST	/transfer	Submit a new FT transfer
GET	/transaction/{id}	Fetch a transaction’s details
GET	/transactions/{receiver_id}	Get all transactions sent to a receiver
GET	/transactions	Paginated list of all transactions
GET /transactions/{status} Paginated list of transactions with a specific status``




---

☁️ Deployment

Containerized via Docker — deployable on Railway, Render, or any cloud provider.
See Dockerfile for build details.


---

⚡ Benchmarking

Designed to achieve 100+ transfers/sec.
Use the included ft-load-tester crate for performance testing in local or testnet environments.

