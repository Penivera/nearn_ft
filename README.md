# High-Throughput NEAR FT Transfer Service (Developer Guide)

This repository implements a high-throughput service that batches fungible token (FT) transfers and submits them to NEAR in parallel using a pool of access keys.

This README is developer-focused: it documents the current runtime, configuration surface, build/run instructions, and where the important code lives.

- Binary name: `nearn_ft` ([src/main.rs](src/main.rs))
- HTTP port: 8080 (see [`main`](src/main.rs))
- Swagger UI: served at root (`/`) and OpenAPI JSON at `/api-docs/openapi.json` (see [`ApiDoc`](src/lib.rs))
- Persistence: Redis (configured via `REDIS_URL`)

Core modules and symbols

- Configuration: [`config::Settings`](src/config.rs)
- HTTP API handlers: [`ft_transfer`](src/lib.rs), [`get_transaction_by_id`](src/lib.rs), [`get_transactions_by_receiver`](src/lib.rs), [`get_all_transactions`](src/lib.rs)
- Worker: [`run_worker`](src/worker.rs)
- Types: [`TokenTransferRequest`](src/types.rs), [`TransactionRecord`](src/types.rs)

Quick links to files

- [src/main.rs](src/main.rs)
- [src/lib.rs](src/lib.rs)
- [src/worker.rs](src/worker.rs)
- [src/config.rs](src/config.rs)
- [src/types.rs](src/types.rs)
- [Dockerfile](Dockerfile)
- [Settings.toml](Settings.toml)
- [README.md](README.md)
- [DEPLOY.md](DEPLOY.md)

Requirements (dev)

- Rust (rustup)
- Docker (optional for container builds)
- A running Redis instance (local or remote)

Configuration

- Settings file: `Settings.toml` (read by [`config::Settings`](src/config.rs)). Notable keys:
  - `rpc_urls` (array of NEAR RPC endpoints)
  - `ft_contract_id` (token contract account)
  - `account_id` (master account used to create/send keys)
  - `batch_size`, `batch_timeout_secs`, `concurrency`, `num_pool_keys`
  - `network` (e.g. `"testnet"`)

- Environment variables (loaded via `.env` / environment):
  - `NEAR_MASTER_KEY` — seed phrase used to create the master signer (required by startup key generation)
  - `REDIS_URL` — Redis connection string (e.g. `redis://127.0.0.1/`)
  - Optionally `NEAR_ACCOUNT_ID`, `NEAR_PRIVATE_KEY`, `FT_CONTRACT_ID`, `FT_DECIMALS` are present in the example `.env` but the code primarily uses `Settings.toml` + `NEAR_MASTER_KEY`.

Build (local)

1. Fetch deps and build:

```sh
cargo build --release
```

2. Run:

```sh
cargo run --release
```

The server binds to port 8080. Swagger UI is available at `/` and OpenAPI JSON at `/api-docs/openapi.json`.

Run tests

```sh
cargo test
```

VSCode launch configurations are available in `.vscode/launch.json`.

Docker (production-ish)

The repository contains a multi-stage `Dockerfile`. Note: the final image expects `Settings.toml` and environment variables to be provided to the container at runtime.

Build:

```sh
docker build -t nearn-ft-service .
```

Run (recommended):

```sh
# Use your .env file to pass secrets (NEAR_MASTER_KEY, REDIS_URL, etc.)
docker run -p 8080:8080 --env-file .env --name nearn-ft-api nearn-ft-service
```

Important Docker notes

- The `Dockerfile` was adjusted to copy `Settings.toml` into the final image so the binary can read runtime settings. If you prefer to mount `Settings.toml` at runtime, use `-v "$(pwd)/Settings.toml:/app/Settings.toml:ro"`.
- Ensure `REDIS_URL` inside the container points to a reachable Redis instance (use Docker networking or a remote URL).

Runtime behavior highlights (developer)

- On startup the service:
  1. Loads settings via [`config::Settings`](src/config.rs).
  2. Creates a Redis pool (deadpool).
  3. Constructs a `Signer` from the master seed (`NEAR_MASTER_KEY`) and populates a signer pool by generating `num_pool_keys` keys and adding them to the master account (see `src/main.rs`).
  4. Spawns the background worker [`run_worker`](src/worker.rs) which batches requests from an in-memory Tokio mpsc channel and sends multi-action NEAR transactions.

- API request flow (`POST /transfer`, [`ft_transfer`](src/lib.rs)):
  - Validates the payload type [`TokenTransferRequest`](src/types.rs).
  - Creates a `TransactionRecord` (queued), writes it to Redis asynchronously, and pushes (id, request) onto the mpsc channel for the worker.
  - Handler is non-blocking: Redis writes are spawned and the request returns quickly with the generated transaction id.

- Worker behavior:
  - Batches up to `batch_size` requests or until `batch_timeout_secs` expires.
  - Constructs a single NEAR `Transaction` with multiple `ft_transfer` actions (see [`run_worker`](src/worker.rs)).
  - Submits transactions with a pooled signer to allow concurrency without nonce collision.
  - On completion (success/failure) it updates each tracked `txn:{id}` record in Redis. Redis writes here are intentionally done in spawned tasks so the worker can continue.

Useful developer commands

- Lint/format:

```sh
cargo fmt
cargo clippy
```

- Run inside dev container and open Swagger UI in host browser:

```sh
# after starting service in container
$BROWSER "http://localhost:8080/"
```

Contributing / debugging tips

- Add log statements in [`src/lib.rs`] and [`src/worker.rs`] — logs are initialized with `env_logger`.
- To reproduce transfer flows locally, start a local Redis and configure `Settings.toml` to point to `rpc.testnet.near.org` (or a sandbox RPC).
- Inspect Redis keys:
  - `txn:{id}` — full serialized `TransactionRecord`
  - `user_txns:{receiver}` — list of transaction IDs for a receiver

  ## Troubleshooting: nonce ("nounce") errors and rate limits

  What people commonly call a "nounce" error is a nonce mismatch or access-key collision when submitting transactions from the same account/keyset. On NEAR this most often happens when:

  - multiple transactions are submitted in parallel using the same access key (nonces must increase monotonically per key),
  - the client re-uses the same key while a previous transaction is still in-flight and the network/RPC hasn't accepted the new nonce yet, or
  - the RPC node enforces rate limits and drops or delays requests which causes the client to retry with an out-of-date nonce.

  Recommended mitigations and best practices

  - Increase the signer pool size and number of function-call access keys
    - Provision multiple function-call-only access keys under the same account. Each key has its own nonce sequence, allowing true parallelism.
    - Use a signer pool or round-robin allocator so the worker can pick different keys for concurrent submissions.
  - Limit concurrency with a semaphore
    - Don't send unbounded concurrent requests. Use a semaphore (configured by `concurrency` in `Settings.toml`) to cap the number of in-flight transactions per configured signer pool.
  - Rotate keys and maintain per-key queues
    - Optionally maintain a small, per-key in-memory queue so that nonces are strictly increasing per key while still allowing other keys to proceed.
  - Add exponential backoff + nonce refresh on retry
    - If a submission fails with a nonce error, refresh the nonce (query the account state) and retry with backoff. Avoid tight retry loops.
  - Use multiple (paid) RPC endpoints / provider pool
    - Distribute requests across several reliable RPC endpoints (private or paid providers) to avoid per-node rate limits. Configure `rpc_urls` in `Settings.toml` and rotate requests across the list.
  - Monitor and alert on nonce errors
    - Emit metrics/logs when nonce errors happen; track which key caused the issue and increase throttling for that key if it repeatedly fails.
  - Ensure clock sync and low client-side latency
    - Very large time skew or slow client-to-RPC latency can exacerbate race conditions; keep hosts NTP-synced and colocate where appropriate.

  Small checklist to try when you see nonce errors

  1. Increase `num_pool_keys` and `concurrency` in `Settings.toml` (keep `concurrency` <= `num_pool_keys`).
  2. Add or rotate RPC URLs in `rpc_urls` to spread traffic.
  3. Enable more conservative retries + exponential backoff in the worker.
  4. If needed, add a short per-key serialization (per-key queue) to guarantee nonce ordering.

  ## ft-load-tester (documentation)

  Location: `ft-load-tester/src/main.rs`

  What it does

  - A small Tokio program that repeatedly POSTs `TokenTransferRequest`-like JSON payloads to the API `POST /transfer` endpoint to simulate load.
  - Currently it uses a few hard-coded parameters near the top of `main.rs`:
    - `target_url` — default `http://127.0.0.1:8080/transfer`
    - `requests_per_second` — number of POSTs sent per second (default `100`)
    - `duration_minutes` — total test duration in minutes (default `10`)

  How to run the tester

  1. Build and run directly from the `ft-load-tester` crate (recommended for quick tests):

  ```bash
  cd ft-load-tester
  cargo run --release
  ```

  2. To modify test parameters, edit the variables at the top of `ft-load-tester/src/main.rs` (or extend the tester to accept CLI flags/env vars).

  3. The tester reports a progress bar and prints a small summary at the end (total requests, successes, failures).

  Notes

  - Each request posts a JSON body with `reciever_id`, `amount`, and `memo`. The tester currently generates unique `receiver_id`s per request so transaction deduplication isn't an issue.
  - If you need more realistic scenarios (retries, non-unique receivers, larger payloads), extend the tester accordingly.

  ## Local workflow: build Docker image, run service, then run the load test

  This quick workflow shows how to build the production Docker image, start the service, wait for readiness, then run the load tester against the containerized service.

  1. Build the Docker image

  ```bash
  docker build -t nearn-ft-service .
  ```

  2. Run the container (detached), providing your `.env` file that contains `NEAR_MASTER_KEY`, `REDIS_URL`, and any other secrets:

  ```bash
  docker run -d --name nearn-ft-api --env-file .env -p 8080:8080 nearn-ft-service
  ```

  3. Wait for the service to become ready. A simple readiness loop checks the root Swagger UI or the OpenAPI JSON endpoint:

  ```bash
  until curl -sSf http://127.0.0.1:8080/api-docs/openapi.json >/dev/null 2>&1; do
    echo "waiting for service..."
    sleep 1
  done
  echo "service ready"
  ```

  4. Run the load tester

  ```bash
  cd ft-load-tester
  cargo run --release
  ```

  Optional: run the tester from another host or CI by ensuring `target_url` points at the host:port where the container is accessible.

  ## Example combined script

  Save the following as `scripts/run_local_load_test.sh` and run it from the project root:

  ```bash
  #!/usr/bin/env bash
  set -euo pipefail

  docker build -t nearn-ft-service .
  docker run -d --name nearn-ft-api --env-file .env -p 8080:8080 nearn-ft-service

  echo "Waiting for API to be ready..."
  until curl -sSf http://127.0.0.1:8080/api-docs/openapi.json >/dev/null 2>&1; do
    sleep 1
  done
  echo "API ready — starting load tester"

  pushd ft-load-tester >/dev/null
  cargo run --release
  popd >/dev/null

  echo "Done"
  ```

  Make the script executable:

  ```bash
  chmod +x scripts/run_local_load_test.sh
  ```

  License: MIT (see `LICENSE`)

  ## Load test report

  Attached artifacts

  - Screenshot: `/demo.png` (terminal summary / progress bar snapshot)


  Summary (from run)

  - Tester: `ft-load-tester` (see `ft-load-tester/src/main.rs`)
  - Mode: single-node Docker container running the API; tester run from host
  - Target: `http://127.0.0.1:8080/transfer`
  - Load: default hard-coded run (100 requests/sec, 10 minutes)
  - Build: release

  Observed (example interpretation)

  - Total requests sent: (see screenshot / log)
  - Successes: (see screenshot / log)
  - Failures: (see screenshot / log)

  Quick analysis checklist

  1. If failures are predominantly nonce errors, follow the "Troubleshooting: nonce" checklist above (increase `num_pool_keys`, reduce `concurrency`, add RPC urls).
  2. If failures are HTTP 429 / rate limit errors, add more reliable RPC endpoints or use paid providers and distribute requests across them.
  3. If Redis writes are slow or failing, check `REDIS_URL`, container networking, and Redis memory/IO.

 