# core Agent Guide

## Scope
- Shared Rust core crate for multiplayer runtime and rule enforcement.
- Exposes adapter interface for game-specific prompt behavior.
- Hosts websocket JSON protocol types used by the unified server and shared by the web client.
- Not a standalone deployable server; runtime entrypoint lives in `server`.

## Modules
- `src/adapter.rs`: `GameAdapter` trait contract.
- `src/protocol.rs`: client/server websocket message definitions.
- `src/game.rs`: room/player state and consume/win rule evaluation.
- `src/server.rs`: axum websocket server runtime and room lifecycle.

## Commands
- Build/tests: `cargo test -p core`
- Full workspace: `cargo test --workspace`

## Implementation Rules
- Keep server authoritative for prompt outcomes and size updates.
- Preserve win rules:
  - 2 players: largest wins when `largest > 2 * other`.
  - 3+ players: largest wins when `largest > sum(others)`.
- Keep minimum size gate before any consumption can happen.
