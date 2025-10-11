# =================================================================
# Stage 1: Alpine MUSL Builder
# =================================================================
FROM rust:alpine as builder

# Add linux-headers (for hidapi) and perl (for openssl)
RUN apk add --no-cache build-base eudev-dev clang linux-headers perl

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
FROM scratch

COPY --from=builder /app/target/release/nearn_ft /nearn_ft

CMD ["/nearn_ft"]