# =================================================================
# Stage 1: MUSL Builder
# =================================================================
FROM rust:latest as builder

# Install MUSL tools and add the rustup target
RUN apt-get update && apt-get install -y musl-tools clang && \
    rustup target add x86_64-unknown-linux-musl

WORKDIR /app

COPY ./Cargo.toml ./Cargo.lock* ./

# Build a dummy project to cache dependencies
RUN mkdir src && \
    echo "fn main() {println!(\"if you see this, the build broke\")}" > src/main.rs && \
    cargo build --target x86_64-unknown-linux-musl --release && \
    rm -f target/x86_64-unknown-linux-musl/release/deps/nearn_ft*

COPY ./src ./src

# Build the actual application for the MUSL target
RUN cargo build --target x86_64-unknown-linux-musl --release

# =================================================================
# Stage 2: Final Static Image
# =================================================================
# Use the 'scratch' image, which is completely empty, for a minimal final image
FROM scratch

# Copy the static binary from the builder stage
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/nearn_ft /nearn_ft

# Set the command to run your application
CMD ["/nearn_ft"]