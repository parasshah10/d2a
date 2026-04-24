# ── Stage 1: Build ────────────────────────────────────────────────────────────  
FROM rust:1.85-slim AS builder  

RUN apt-get update && apt-get install -y pkg-config libssl-dev && \  
    rm -rf /var/lib/apt/lists/*  

WORKDIR /app  
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
ENTRYPOINT ["/app/entrypoint.sh"]

