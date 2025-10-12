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

License: MIT (see `LICENSE`)
