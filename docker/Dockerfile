FROM alpine:3.21

RUN apk add --no-cache ca-certificates && \
    adduser -D -h /app -s /sbin/nologin app && \
    mkdir -p /app/config /app/data && \
    chown app:app /app/config /app/data


WORKDIR /app

COPY target/x86_64-unknown-linux-musl/release/ds-free-api /app/ds-free-api
COPY web/dist /app/web/dist
COPY config.example.toml /app/config/config.toml

ENV RUST_LOG=info \
    DS_DATA_DIR=/app/data \
    DS_CONFIG_PATH=/app/config/config.toml

EXPOSE 5317

ENTRYPOINT ["/app/ds-free-api"]
