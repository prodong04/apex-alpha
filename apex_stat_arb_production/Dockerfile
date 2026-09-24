# ==============================================================================
# APEX ALPHA - MULTI-STAGE SECURE DOCKERFILE
# Stage 1: Build binary in Rust environment
# Stage 2: Distroless / Slim runtime (No source code, No secrets baked in)
# ==============================================================================

FROM rust:1.80-slim-bullseye AS builder

WORKDIR /usr/src/apex-engine

# Install build essentials
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy cargo definitions first for caching
COPY engine/Cargo.toml engine/Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release || true

# Copy actual source code
COPY engine/src ./src

# Build optimized production binary
RUN cargo build --release --bin apex-stat-arb-production

# ------------------------------------------------------------------------------
# Stage 2: Minimal Runtime (Debian Slim)
# ------------------------------------------------------------------------------
FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y ca-certificates libssl1.1 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /usr/src/apex-engine/target/release/apex-stat-arb-production /app/apex-stat-arb-production
COPY config/stat_arb_config.toml /app/config/stat_arb_config.toml

# Non-root secure user
RUN useradd -m -u 1000 apexuser && chown -R apexuser:apexuser /app
USER apexuser

# Binary entrypoint
ENTRYPOINT ["/app/apex-stat-arb-production"]
