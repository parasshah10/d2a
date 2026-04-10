# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Rust API proxy exposing free DeepSeek model endpoints. Translates standard OpenAI-compatible requests to DeepSeek's internal protocol with account pool rotation, PoW challenge handling, and streaming response support.

## Principles

### 1. Single Responsibility
- `config.rs`: Configuration loading only, no client creation or business logic
- `client.rs`: Raw HTTP calls only, no token caching, retry, or SSE parsing
- `accounts.rs`: Account pool management only, no network requests
- `pow.rs`: WASM computation only, no account management or request sending

### 2. Minimal Viable
- No premature abstractions: Extract traits/structs when needed, not before
- No redundant code: Remove unused imports, avoid over-documenting, no pre-written tests
- Delay dependency introduction: pingora temporarily removed, will add when proxy layer needed

### 3. Control Complexity
- Explicit over implicit: Dependencies injected via parameters, no global state
- Composition over inheritance: Small modules composed via functions, no deep inheritance
- Clear boundaries: Modules interact via explicit interfaces, no internal logic leakage

## Architecture

```
src/
├── main.rs             # Entry point (stub)
├── lib.rs              # Library exports: DeepSeekCore, OpenAIAdapter, Config, etc.
├── config.rs           # Config loader: -c flag, config.toml default
├── openai_adapter.rs   # OpenAI adapter interface (todo!() bodies, submodules TBD)
├── openai_adapter/     # Reserved for future adapter submodules
├── ds_core.rs          # DeepSeek module facade (v0_chat entry)
├── ds_core/
│   ├── accounts.rs     # Account pool: init validation, round-robin selection
│   ├── pow.rs          # PoW solver: WASM loading, DeepSeekHashV1 computation
│   ├── completions.rs  # Chat orchestration: SSE streaming, account guard
│   └── client.rs       # Raw HTTP client: API endpoints, zero business logic
└── qw_core.rs          # Qwen module stub (not started)
```

**Account Pool Model**: 1 account = 1 session = 1 concurrency. Scale via more accounts.

**Request Flow**: `v0_chat()` → `get_account()` → `compute_pow()` → `edit_message(payload)` → `GuardedStream`

`completions.rs` hardcodes `message_id: 1` in `EditMessagePayload` because the health check during initialization already writes message 0 into the session.

### Key Architectural Patterns

**GuardedStream & Account Lifecycle**
`AccountGuard` marks an account as `busy` and automatically releases it on `Drop`. `GuardedStream` wraps the SSE stream with an `AccountGuard`, so the account is held busy until the stream is fully consumed or dropped. This binds account concurrency to stream lifetime without explicit cleanup logic.

**Account Initialization Flow**
`AccountPool::init()` spins up all accounts concurrently. Per-account initialization (`try_init_account`) follows this sequence:
1. `login` — obtain Bearer token
2. `create_session` — create chat session
3. `health_check` — send a test completion (with PoW) to verify the session is writable
4. `update_title` — rename session to "managed-by-ai-free-api"

Health check is required because an empty session will fail on `edit_message` with `invalid message id`.

**Error Translation Chain**
Errors propagate upward with translation at module boundaries:
- `client.rs`: `ClientError` (HTTP, business errors, JSON parse)
- `accounts.rs`: `PoolError` (`ClientError` | `PowError` | validation errors)
- `ds_core.rs`: `CoreError` (`Overloaded` | `ProofOfWorkFailed` | `ProviderError` | `Stream`)

`client.rs` parses DeepSeek's wrapper envelope `{code, msg, data: {biz_code, biz_msg, biz_data}}` via `Envelope::into_result()`.

**PoW Fragility**
`pow.rs` loads a WASM module downloaded from DeepSeek's CDN. The solver hardcodes the wasm-bindgen-generated symbol `__wbindgen_export_0` for memory allocation. If DeepSeek recompiles the WASM and changes export ordering, instantiation will fail with `PowError::Execution`. The WASM URL is configurable in `config.toml` to allow quick updates.

## Where to Look

| Task | Location | Notes |
|------|----------|-------|
| Config loading | `src/config.rs` | Single unified entry, `-c` flag support |
| DeepSeek chat flow | `src/ds_core/` | accounts → pow → completions → client |
| OpenAI adapter | `src/openai_adapter.rs` | Interface defined, implementations are `todo!()` |
| CLI example | `examples/ds_core_cli.rs` | Interactive and script modes; see `examples/ds_core_cli-script.txt` |
| Code style / logging | `docs/code-style.md`, `docs/logging-spec.md` | Comments, naming, targets, levels |
| API reference | `docs/deepseek-api-reference.md` | DeepSeek endpoint details |

## Conventions

- **Config**: Uncommented values in `config.toml` = required; commented = optional with default
- **Module files**: `foo.rs` declares sub-modules, `foo/` contains implementation
- **Comments**: Chinese in source files (team preference)
- **Errors**: Chinese error messages for user-facing output

## Anti-Patterns

- Do NOT create separate config entry points — `src/config.rs` is the single source
- Do NOT implement provider logic outside its `*_core/` module
- Do NOT commit `config.toml` (only `config.toml.example`)
- Do NOT use `println!`/`eprintln!` in library code — use `log` crate with target

## Commands

```bash
# Setup (do not commit config.toml)
cp config.toml.example config.toml

# One-pass check (check + clippy + fmt)
just check

# Run ds_core_cli example (primary testing mechanism)
just ds-core-cli
RUST_LOG=debug just ds-core-cli
just ds-core-cli -- source examples/ds_core_cli-script.txt

# Individual checks
cargo check
cargo clippy -- -D warnings
cargo fmt --check

# Build library
cargo build

# cargo run is currently a no-op; main.rs is a stub
cargo run  # does nothing useful yet

# There are currently no unit tests in src/
```

## Implementation Status

- `ds_core::client` ✅ — HTTP layer complete
- `ds_core::pow` ✅ — WASM PoW solver
- `ds_core::accounts` ✅ — Account pool with lifecycle management
- `ds_core::completions` ✅ — SSE streaming with GuardedStream
- `main.rs` ⚠️ — Stub only, server bootstrap pending
- `openai_adapter` ⚠️ — Interface sketched, implementations pending
- `qw_core` ⚠️ — Not started
