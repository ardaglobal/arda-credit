# Balanced Dockerfile: Optimized but maintainable
# Single file that works for both development and production

# hadolint ignore=DL3007
FROM ghcr.io/ardaglobal/base-containers/rust-sp1-base:latest AS builder

WORKDIR /app

# Copy dependency manifests first (for better caching)
COPY Cargo.toml Cargo.lock ./
COPY api/Cargo.toml ./api/
COPY db/Cargo.toml ./db/
COPY lib/Cargo.toml ./lib/
COPY program/Cargo.toml ./program/
COPY script/Cargo.toml ./script/
COPY scripts/Cargo.toml ./scripts/

# Copy source code and SQLX query cache
COPY . .

# Build SP1 program first
WORKDIR /app/program
RUN cargo prove build --output-directory ../build

# Build server binary and sqlx CLI
WORKDIR /app/api
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin server

# Build scripts binaries and install sqlx CLI
WORKDIR /app/scripts
RUN cargo build --release --bin populate_deal_data && \
    cargo install --version 0.7.4 sqlx-cli --no-default-features --features postgres,rustls

# Foundry stage - for copying cast binary
FROM ghcr.io/foundry-rs/foundry:latest AS foundry

# Runtime stage - includes sqlx for migrations and cast for faucet
FROM debian:bookworm-slim

# hadolint ignore=DL3008
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 1001 appuser

WORKDIR /app

# Copy binaries and migrations
COPY --from=builder /app/target/release/server ./server
COPY --from=builder /app/target/release/populate_deal_data ./populate_deal_data
COPY --from=builder /usr/local/cargo/bin/sqlx ./sqlx
COPY --from=builder /app/db/migrations ./migrations
# Copy cast from Foundry for faucet functionality
COPY --from=foundry /usr/local/bin/cast /usr/local/bin/cast

# Ensure binaries are executable and set ownership
RUN chmod +x /usr/local/bin/cast ./server ./populate_deal_data ./sqlx && \
    chown -R appuser:appuser /app

USER appuser

EXPOSE 8080
CMD ["sh", "-c", "./sqlx migrate run --source ./migrations --database-url $DATABASE_URL && ./server --host 0.0.0.0 --port 8080"]
