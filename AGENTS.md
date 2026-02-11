# AGENTS.md

This document gives AI coding agents and human contributors a fast orientation for `lexeme-party`.

## Project Purpose

`lexeme-party` is a multiplayer typing race game:

- all connected players see the same target word each round
- players race to type the word correctly first
- first correct submission wins the round and earns 1 point
- each player is rendered as a circle; higher score means larger circle
- circles are simulated in a center-seeking, collision-separated clump
- rounds continue indefinitely while players remain connected

## Current Tech Stack

- Rust 2024 edition
- `axum` for HTTP serving and WebSocket upgrades (single port)
- `tower-http` for static file serving (web client)
- `tokio` for async runtime
- `tokio-tungstenite` for native client WebSocket connections
- `serde` / `serde_json` for wire protocol
- `wgpu` + `winit` for native client rendering and input (legacy)
- `font8x8` for native client bitmap text overlays (legacy)
- HTML / CSS / Canvas 2D / vanilla JS for the web client

## How To Run

### Web (primary)

```bash
cargo run -- server
# Open http://127.0.0.1:9002 in one or more browser tabs
```

### Native client (legacy, useful for dev testing)

```bash
cargo run -- server
cargo run -- client ws://127.0.0.1:9002/ws Alice
cargo run -- client ws://127.0.0.1:9002/ws Bob
```

## Binary Modes

`src/main.rs` supports:

- `server [bind_addr]`
- `client [ws_url] [player_name]`

Defaults:

- server bind: `127.0.0.1:9002`
- client url: `ws://127.0.0.1:9002/ws`
- client name: `player`

## Source Layout

- `src/main.rs` - mode switch entrypoint
- `src/protocol.rs` - shared websocket message schema
- `src/words.rs` - word bank and random word selection (no consecutive repeats)
- `src/server.rs` - axum-based server: HTTP static files + WebSocket game logic
- `src/client/` - native client module directory (legacy)
  - `mod.rs` - event loop glue, module re-exports, `run_client` entrypoint
  - `net.rs` - websocket networking thread, `NetworkEvent`, `spawn_network`
  - `game.rs` - game state, player sync, physics simulation, input handling
  - `render.rs` - wgpu render state, GPU pipelines, vertex types, draw calls
  - `hud.rs` - bitmap text rasterization (word display, leaderboard)
- `src/shaders` - WebGPU shader files (native client)
- `web/index.html` - web client (HTML + CSS + JS, single file)

## Server Architecture

The server uses `axum` to handle both HTTP and WebSocket on a single port:

- `GET /ws` - WebSocket upgrade for game connections
- All other requests - serves static files from `web/` directory (web client)

The `web/` directory is resolved from `CARGO_MANIFEST_DIR` at compile time, so `cargo run` always finds the right files.

## Networking Model

Server is authoritative for:

- player IDs
- current round word
- scores
- per-player typed progress
- winner and round transitions

Client sends:

- `Join { name }`
- `TypedProgress { typed }`
- `SubmitWord { word }`

Server broadcasts:

- `Welcome { player_id }`
- `State { round, current_word, players, winner_last_round }`

Wire format uses `#[serde(tag = "type", content = "data")]` adjacently-tagged enums, e.g.:
```json
{"type": "Join", "data": {"name": "Alice"}}
{"type": "State", "data": {"round": 1, "current_word": "forest", "players": [...], "winner_last_round": null}}
```

## Web Client Notes

The web client (`web/index.html`) is a single self-contained file:

- **Join screen**: name input, connect button
- **Game canvas**: Circle physics rendered via Canvas 2D API
- **HUD overlay**: DOM elements for word display, typed text, leaderboard, winner banner
- **Physics**: ports the Rust center-seeking gravity, damping, and collision separation
- **Letter coloring**: same logic as native client (green/red for local, blue crowd boost)
- **Auto-submit**: word is submitted automatically when typed text matches
- **Player name labels**: rendered on canvas below each circle

## Gameplay/UX Notes

- local typed letters are colored per character:
  - correct: green
  - incorrect: red
- crowd progress from other players adds a cool/blue emphasis to letters they have correctly progressed through
- local player circle is highlighted (yellow with white border)
- name labels appear below each player circle
- winner banner appears briefly after each round

## Agent Guardrails For Future Changes

1. Keep protocol changes synchronized across:
   - `src/protocol.rs`
   - server message handling in `src/server.rs`
   - web client decode/apply logic in `web/index.html`
   - native client decode/apply logic (if still maintained)
2. Preserve server authority for scoring and round transitions.
3. Prefer additive message fields/enums over breaking wire changes.
4. The web client is the primary client — prioritize it over the native client.
5. Keep `web/index.html` as a single file for simplicity unless it grows unwieldy.

## Recommended Next Iterations

- add countdown/round-transition effects
- add reconnect/session logic
- add sound effects or visual feedback on correct/incorrect typing
- mobile-friendly layout or touch keyboard support
- add TLS/WSS support for production deployment
- add tests for server round/scoring and protocol serialization
- deploy behind a reverse proxy (nginx, Caddy) for production

## Verification Checklist

After all changes:

1. Run `cargo check`

After significant changes:

1. Run `cargo check`
2. Run the server and open two browser tabs at `http://127.0.0.1:9002`
3. Verify:
   - both tabs receive same word
   - first correct submit increments only one player score
   - leaderboard order matches scores
   - circles resize with score
   - word/typing colors update for local and remote progress
   - winner banner appears after a round win
