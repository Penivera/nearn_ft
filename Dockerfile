# =================================================================
# Stage 1: Builder
# =================================================================
FROM rust:latest as builder

# Command to generate self-signed cert and key for local testing:
# openssl req -x509 -newkey rsa:4096 -nodes -keyout key.pem -out cert.pem -days 365 -subj '/CN=localhost'

# Install system dependencies
RUN apt-get update && apt-get upgrade -y && apt-get install -y --no-install-recommends ca-certificates libudev-dev pkg-config clang && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy your manifests
COPY ./Cargo.toml ./Cargo.lock* ./

# Build a dummy project to cache dependencies
RUN mkdir src && \
    echo "fn main() {println!(\"if you see this, the build broke\")}" > src/main.rs && \
    cargo build --release && \
    rm -f target/release/deps/nearn_ft*

# Copy the actual source code
COPY ./src ./src

# Build the actual application
RUN cargo build --release

# =================================================================
# Stage 2: Final Image
# =================================================================
FROM debian:bookworm-slim

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/nearn_ft /usr/local/bin/nearn_ft

# Set the command to run your application
CMD ["nearn_ft"]