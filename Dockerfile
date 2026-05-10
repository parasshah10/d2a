# ── Stage 0: cargo-chef installer ─────────────────────────────────────────────  
FROM rust:slim-bookworm AS chef  
RUN cargo install cargo-chef --locked
WORKDIR /app  
  
# ── Stage 1a: Generate dependency recipe ──────────────────────────────────────  
FROM chef AS planner  
COPY . .  
RUN cargo chef prepare --recipe-path recipe.json  
  
# ── Stage 1b: Build dependencies only (cached layer) ──────────────────────────  
FROM chef AS builder  
RUN apt-get update && apt-get install -y pkg-config libssl-dev build-essential cmake && \  
    rm -rf /var/lib/apt/lists/*  
COPY --from=planner /app/recipe.json recipe.json  
RUN cargo chef cook --release --recipe-path recipe.json  
  
# ── Stage 1c: Build application code ──────────────────────────────────────────  
COPY . .  
RUN cargo build --release  
  
# ── Stage 2: Runtime ──────────────────────────────────────────────────────────  
FROM debian:bookworm-slim  
  
RUN apt-get update && apt-get install -y ca-certificates && \  
    rm -rf /var/lib/apt/lists/*  
  
# Create a non-root user for HF Spaces  
RUN useradd -m -u 1000 user  
WORKDIR /app  
  
COPY --from=builder /app/target/release/ds-free-api /usr/local/bin/ds-free-api  
COPY entrypoint.sh /app/entrypoint.sh  
RUN sed -i 's/\r$//' /app/entrypoint.sh && \  
    chmod +x /app/entrypoint.sh && \  
    chown -R 1000:1000 /app  
  
# Switch to non-root user  
USER 1000  
  
EXPOSE 7860  
ENTRYPOINT["/app/entrypoint.sh"]
