# =================================================================
# Stage 1: Alpine MUSL Builder
# - Uses Alpine Linux, which is musl-based, to simplify the build.
# =================================================================
FROM rust:1.82-alpine as builder

# Install build tools and the musl-compatible version of libudev (eudev-dev)
RUN apk add --no-cache build-base eudev-dev clang

WORKDIR /app

COPY ./Cargo.toml ./Cargo.lock* ./

# Build a dummy project to cache dependencies
RUN mkdir src && \
    echo "fn main() {println!(\"if you see this, the build broke\")}" > src/main.rs && \
    cargo build --release && \
    rm -f target/release/deps/nearn_ft*

COPY ./src ./src

# Build the actual application. No --target flag is needed because Alpine's default is musl.
RUN cargo build --release

# =================================================================
# Stage 2: Final Static Image
# =================================================================
# Use the 'scratch' image, which is completely empty
FROM scratch

# Copy the static binary from the builder stage
# The path is simpler because we are not cross-compiling anymore
COPY --from=builder /app/target/release/nearn_ft /nearn_ft

# Set the command to run your application
CMD ["/nearn_ft"]