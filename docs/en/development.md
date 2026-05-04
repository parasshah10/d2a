# Development Guide

## Prerequisites

- Rust **1.95.0+** (see `rust-toolchain.toml`)
- Node.js **18+** (for web panel development)

## Building from Source

```bash
# 1. Build web frontend (compiled into binary, required for release builds)
cd web && npm install && npm run build && cd ..

# 2. Build release binary
cargo build --release

# 3. Run
./target/release/ds-free-api
```

> **Dev mode**: If `web/dist/` exists, the server reads from the filesystem (supports Vite HMR);
> otherwise it falls back to the embedded assets. Run `npm run dev` (Vite HMR) alongside `just serve` for hot-reload.

## Docker

```bash
# 1. Cross-compile Rust binary (Mac ARM → x86 Linux)
cargo zigbuild --release --target x86_64-unknown-linux-gnu

# 2. Build frontend
cd web && npm install && npm run build && cd ..

# 3. Build Docker image
docker build -f docker/Dockerfile -t ds-free-api .

# 4. Export and transfer to server
docker save ds-free-api | gzip > ds-free-api.tar.gz
scp ds-free-api.tar.gz user@server:/tmp/

# 5. Load and run on server
ssh user@server
docker load < /tmp/ds-free-api.tar.gz
docker compose -f docker/docker-compose.yaml up -d
```

## Commands

```bash
# One-pass check (check + clippy + fmt + audit + unused deps)
just check

# Run tests
cargo test

# Run HTTP server
just serve

# Unified protocol debug CLI
just adapter-cli

# Start server with e2e config
just e2e-serve
```

## e2e Testing

The `py-e2e-tests/` framework uses JSON-driven scenarios (no pytest required):

| Layer       | Command            | Coverage                                              |
| ----------- | ------------------ | ----------------------------------------------------- |
| **Basic**   | `just e2e-basic`   | Core functionality, both OpenAI + Anthropic endpoints |
| **Repair**  | `just e2e-repair`  | Tool call malformed format repair (OpenAI only)       |
| **Stress**  | `just e2e-stress`  | All scenarios × 3 iterations, safe concurrency + 1    |

Scenario files are organized under `scenarios/`:

```
py-e2e-tests/
├── scenarios/
│   ├── basic/
│   │   ├── openai/         # 10 scenarios (chat, reasoning, streaming, tools, files, images, HTTP links)
│   │   └── anthropic/      # 6 scenarios (chat, reasoning, tools, documents, images, HTTP links)
│   └── repair/             # 10 malformed tool call scenarios
├── runner.py               # Single-run entry
├── stress_runner.py        # Multi-iteration stress test entry
└── config.toml             # e2e server configuration
```

Each scenario is a standalone JSON file with request params and validation rules:

```json
{
  "name": "Scenario name",
  "endpoint": "openai|anthropic",
  "category": "basic|repair",
  "models": ["deepseek-default", "deepseek-expert"],
  "messages": [{"role": "user", "content": "..."}],
  "tools": [...],
  "tool_choice": "auto",
  "request": {"stream": false},
  "checks": {
    "has_tool_calls": true,
    "tool_names": ["get_weather"],
    "finish_reason": "tool_calls",
    "no_error": true
  }
}
```

### e2e CLI Arguments

**`just e2e-basic` / `just e2e-repair` (single run):**

| Argument | Description |
|----------|-------------|
| `scenario_dir` | Scenario directory, e.g. `scenarios/basic` |
| `--endpoint` | Filter by endpoint: `openai` / `anthropic` |
| `--model` | Filter by model: `deepseek-default` / `deepseek-expert` |
| `--filter` | Filter by scenario name keywords (space-separated) |
| `--parallel` | Parallelism, default `accounts ÷ 2` |
| `--show-output` | Show model response summary |
| `--report` | Output JSON report path |

**`just e2e-stress` (stress test):**

| Argument | Description |
|----------|-------------|
| `--iterations` | Iterations per scenario, default 3 |
| `--models` | Filter by model list |
| `--filter` | Filter by scenario name keywords |
| `--parallel` | Parallelism, default `accounts ÷ 2 + 1` |
| `--show-output` | Show model output |
| `--report` | Output JSON report path |

Examples:

```bash
# Quick verification of new file upload scenarios
just e2e-basic --filter file image --show-output

# Run OpenAI-only with expert model
just e2e-basic --endpoint openai --model deepseek-expert

# Serial debugging
just e2e-basic --endpoint openai --parallel 1 --show-output

# Stress test: repair scenarios × 5 iterations
just e2e-stress --filter repair --iterations 5

# Output JSON report
just e2e-basic --report result.json
```

## More Documentation

- [Code Style](code-style.md)
- [Logging Spec](logging-spec.md)
- [DeepSeek API Reference](deepseek-api-reference.md)
- [Prompt Injection Strategy](deepseek-prompt-injection.md)

> Note: All docs are currently in Chinese. English translations are planned.
