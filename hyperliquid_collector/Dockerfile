# ==============================================================================
# Stage 1: Build binary using latest official Rust image
# ==============================================================================
FROM rust:latest as builder

WORKDIR /app

# Install build dependencies for OpenSSL / native-tls
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy dependency files for layer caching
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# Copy actual source code and build final binary
COPY src ./src
RUN touch src/main.rs && cargo build --release

# ==============================================================================
# Stage 2: Minimal runtime image
# ==============================================================================
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime certificates and python for inspection script
RUN apt-get update && apt-get install -y ca-certificates python3 && rm -rf /var/lib/apt/lists/*

# Copy compiled release binary from builder
COPY --from=builder /app/target/release/market-collector /app/market-collector
COPY scripts /app/scripts

# Create data directory mount point
RUN mkdir -p /app/data/live

# Environment defaults
ENV RUST_LOG=info
ENV TARGET_COIN=BTC
ENV DATA_DIR=/app/data/live
ENV ENABLE_BINANCE_SPOT=true
ENV ENABLE_BINANCE_FUTURES=true
ENV ENABLE_HYPERLIQUID=true

VOLUME ["/app/data"]

CMD ["/app/market-collector"]
