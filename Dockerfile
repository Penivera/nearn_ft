# =================================================================
# Stage 1: Alpine MUSL Builder
# =================================================================
FROM rust:alpine as builder

# Add all necessary build-time dependencies
RUN apk add --no-cache build-base eudev-dev clang linux-headers perl curl

WORKDIR /app

COPY ./Cargo.toml ./Cargo.lock* ./

# Build a dummy project to cache dependencies
RUN mkdir src && \
    echo "fn main() {println!(\"if you see this, the build broke\")}" > src/main.rs && \
    cargo build --release && \
    rm -f target/release/deps/nearn_ft*

COPY ./src ./src

# Build the actual application
RUN cargo build --release

# =================================================================
# Stage 2: Final Static Image
# =================================================================
# The 'scratch' image is completely empty, providing a minimal attack surface.
FROM scratch

# Set a working directory for the application.
WORKDIR /app

# --- FIX ---
# Copy the required Settings.toml file into the final image.
COPY ./Settings.toml .

# Copy the compiled static binary from the builder stage.
COPY --from=builder /app/target/release/nearn_ft .

# Set the command to run your application from the working directory.
CMD ["./nearn_ft"]