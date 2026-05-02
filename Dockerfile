# ── Stage 1: Build Rust binary ──────────────────────────────────────────
FROM rust:1.95.0-bookworm AS rust-builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
# Create dummy main to cache dependencies
RUN mkdir src && echo 'fn main(){}' > src/main.rs
RUN cargo build --release 2>/dev/null || true
# Now copy real source
COPY src/ src/
COPY rust-toolchain.toml ./
# Rebuild with real code (dependency layer cached)
RUN touch src/main.rs && cargo build --release

# ── Stage 2: Runtime ────────────────────────────────────────────────────
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app

COPY --from=rust-builder /build/target/release/ds-free-api /app/ds-free-api
COPY web/dist /app/web/dist
COPY config.example.toml /app/config.example.toml

# Data volume for admin.json, api_keys.json
VOLUME /app/data

ENV RUST_LOG=info
ENV DS_DATA_DIR=/app/data

EXPOSE 5317

ENTRYPOINT ["/app/ds-free-api"]
