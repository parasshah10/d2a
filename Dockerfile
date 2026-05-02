# ── Stage 0: Build web frontend ────────────────────────────────────────
FROM node:22-bookworm-slim AS web-builder
WORKDIR /build/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# ── Stage 1: Build Rust binary (with embedded web) ──────────────────────
FROM rust:1.95.0-bookworm AS rust-builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
# Create dummy main to cache dependencies
RUN mkdir src && echo 'fn main(){}' > src/main.rs
RUN cargo build --release 2>/dev/null || true
# Now copy real source + embedded web assets
COPY src/ src/
COPY --from=web-builder /build/web/dist /build/web/dist
COPY rust-toolchain.toml ./
# Rebuild with real code (dependency layer cached, web assets embedded)
RUN touch src/main.rs && cargo build --release

# ── Stage 2: Runtime ────────────────────────────────────────────────────
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app

COPY --from=rust-builder /build/target/release/ds-free-api /app/ds-free-api
COPY config.example.toml /app/config.example.toml

# Data volume for admin.json, api_keys.json
VOLUME /app/data

ENV RUST_LOG=info
ENV DS_DATA_DIR=/app/data

EXPOSE 5317

ENTRYPOINT ["/app/ds-free-api"]
