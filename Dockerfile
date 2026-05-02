# ── Dockerfile ───────────────────────────────────────────────────────────
# 本地交叉编译 + Docker 打包（Mac ARM → x86 Linux，~2 分钟）
#
# 用法：
#   cargo zigbuild --release --target x86_64-unknown-linux-gnu
#   cd web && npm install && npm run build && cd ..
#   docker build --platform linux/amd64 -t ds-free-api .
#   docker save ds-free-api | gzip > ds-free-api.tar.gz
#
# 服务器原生构建 / CI：
#   直接在 x86 服务器上运行上述命令（无需交叉编译，去掉 --platform 参数）
#   或使用 GitHub Actions 自动构建（见 .github/workflows/）

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app

COPY target/x86_64-unknown-linux-gnu/release/ds-free-api /app/ds-free-api
COPY web/dist /app/web/dist
COPY config.example.toml /app/config.example.toml

VOLUME /app/data

ENV RUST_LOG=info
ENV DS_DATA_DIR=/app/data

EXPOSE 5317

ENTRYPOINT ["/app/ds-free-api"]
