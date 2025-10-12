# Developer Guide: Building and Deploying the NEAR FT Transfer Service

This document provides instructions for developers on how to build, configure, and deploy the high-throughput NEAR Fungible Token (FT) transfer service.

## 1. Prerequisites

Before you begin, ensure you have the following installed:

- **Rust**: The service is written in Rust. If you don't have it, install it via [rustup](https://rustup.rs/).
- **Docker**: For containerized builds and deployment.
- **Redis**: A running Redis instance is required for the persistence layer. You can run it locally or use a cloud-based service.

## 2. Configuration

The service uses a combination of a `.env` file for secrets and a `Settings.toml` file for general configuration.

### Step 1: Create the `.env` File

Create a `.env` file in the root of the project. This file is ignored by Git and should contain your sensitive credentials.

```env
# .env

# The master account that will fund the transfers.
NEAR_ACCOUNT_ID="your-account.testnet"

# The seed phrase (private key) for the master account.
# IMPORTANT: Keep this secret and secure.
NEAR_MASTER_KEY="your 12-word seed phrase here"

# The URL for the Redis instance.
REDIS_URL="redis://127.0.0.1/"
```

### Step 2: Review `Settings.toml`

This file contains non-sensitive configuration for the service's behavior. Adjust these values as needed for your environment.

```toml
# Settings.toml

# NEAR RPC endpoint URL (e.g., testnet, mainnet).
rpc_url = "https://rpc.testnet.near.org"

# The account ID of the fungible token contract.
ft_contract_id = "token.testnet"

# The master account ID (should match NEAR_ACCOUNT_ID in .env).
account_id = "your-account.testnet"

# Number of decimals for the fungible token.
ft_decimals = 8

# --- Throughput and Batching ---
batch_size = 10
batch_timeout_secs = 3
concurrency = 10
```

## 3. Local Development

For local testing and development, you can run the service directly using Cargo.

### Build

Compile the project to ensure all dependencies are fetched and the code is valid:

```bash
cargo build
```

### Run

Start the API service:

```bash
cargo run
```

The server will start, and you should see log output indicating it's running on `http://127.0.0.1:8000`.

## 4. Docker Deployment

The included `Dockerfile` provides a multi-stage build process to create a minimal, optimized production image.

### Step 1: Build the Docker Image

Build the container image using the following command. This will compile the Rust binary in a builder stage and copy it to a lightweight final image.

```bash
docker build -t nearn-ft-service .
```

### Step 2: Run the Docker Container

Run the service using the image you just built. You must pass the environment variables from your `.env` file to the container.

You can do this in two ways:

**Option A: Using `--env-file` (Recommended)**

This is the cleanest method. It reads all variables from your `.env` file.

```bash
docker run -p 8000:8000 --env-file .env --name nearn-ft-api nearn-ft-service
```

**Option B: Using `-e` for each variable**

If you prefer to specify each variable manually:

```bash
docker run -p 8000:8000 \
  -e NEAR_ACCOUNT_ID="your-account.testnet" \
  -e NEAR_MASTER_KEY="your 12-word seed phrase here" \
  -e REDIS_URL="redis://your-redis-host/" \
  --name nearn-ft-api \
  nearn-ft-service
```

**Note on Networking**: The `REDIS_URL` inside the container might need to be adjusted to point to the correct host if Redis is not running on `localhost` or if you are using Docker networking.

## 5. API Usage

Once the service is running, you can interact with it via its REST API.

- **Swagger UI**: Interactive API documentation is available at `http://localhost:8000/`.
- **Transfer Endpoint**: `POST /transfer`
- **Status Endpoints**:
  - `GET /transaction/{id}`
  - `GET /transactions/{receiver_id}`
  - `GET /transactions`

This documentation provides a solid foundation for any developer to get the service up and running.
