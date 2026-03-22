# Server Agent Guide

## Scope
- Deployable Rust server binary for the monorepo.
- Boots the websocket runtime from `core` and registers all adapters.
- Represents the single-server architecture (no per-mode server binaries).

## Key Files
- `src/main.rs`: runtime entrypoint, config parsing, adapter registration order.
- `Cargo.toml`: package identity and dependency wiring to core/adapters.

## Runtime Configuration
- `BIND_ADDR`: socket bind address (default `0.0.0.0:4000`).
- `GROWTH_PER_ROUND_WIN`: minimum growth floor awarded to round winners (default `4.0`).
- `MATCH_DURATION_SECS`: match length in seconds from first prompt (default `60`).

## Commands
- Run server: `cargo run -p server`
- Package tests (if present): `cargo test -p server`
- Full workspace tests: `cargo test --workspace`

## Implementation Rules
- Keep this crate thin: orchestration and composition only, not gameplay logic.
- Register adapters explicitly and preserve stable `game_key()` behavior across deployments.
- Prefer additive adapter registration so new game modes do not break existing clients/rooms.
