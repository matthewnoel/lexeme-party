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
- `tokio` + `tokio-tungstenite` for networking
- `serde` / `serde_json` for wire protocol
- HTML5 Canvas + vanilla JS for the primary web client (`static/index.html`)
- `wgpu` + `winit` for the optional native client (legacy, still functional)
- `font8x8` for bitmap text overlays in the native client

## How To Run

Start the server:

```bash
cargo run -- server
```

Then open http://127.0.0.1:9002 in one or more browser tabs to play.

The native wgpu client is still available for development/testing:

```bash
cargo run -- client ws://127.0.0.1:9002 Alice
```

## Binary Modes

`src/main.rs` supports:

- `server [bind_addr]`
- `client [ws_url] [player_name]`

Defaults:

- server bind: `127.0.0.1:9002`
- client url: `ws://127.0.0.1:9002`
- client name: `player`

## Source Layout

- `src/main.rs` - mode switch entrypoint
- `src/protocol.rs` - shared websocket message schema
- `src/words.rs` - word bank and random word selection (no consecutive repeats)
- `src/server.rs` - authoritative game state, round/scoring logic, HTTP+WebSocket serving
- `static/index.html` - self-contained web client (HTML + CSS + JS, embedded into server binary via `include_str!`)
- `src/client/` - native wgpu client module directory (optional/legacy)
  - `mod.rs` - event loop glue, module re-exports, `run_client` entrypoint
  - `net.rs` - websocket networking thread, `NetworkEvent`, `spawn_network`
  - `game.rs` - game state, player sync, physics simulation, input handling
  - `render.rs` - wgpu render state, GPU pipelines, vertex types, draw calls
  - `hud.rs` - bitmap text rasterization (word display, leaderboard)
- `src/shaders` - WebGPU shader files (native client only)

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

## Rendering Notes

### Web client (`static/index.html`)

The web client renders via HTML5 Canvas 2D:

- full-viewport canvas with dark background and center glow
- player circles with radial gradients, glow, and name labels
- per-letter colored target word at top center
- typed input display with blinking cursor
- leaderboard panel (top-left, semi-transparent)
- round info (top-right)
- animated winner banner

Physics, letter coloring, and input handling are ported from the Rust native client with identical constants and behavior.

### Native client (legacy)

The native client renders via wgpu:

- circle instances for each player
- top-center target word text
- top-left leaderboard text

Text is generated CPU-side into RGBA textures each frame only when content/style changes, then drawn as textured quads.

## Gameplay/UX Notes

- local typed letters are colored per character:
  - correct: green
  - incorrect: red
- crowd progress from other players adds a cool/blue emphasis to letters they have correctly progressed through
- local player circle is highlighted
- window title still includes debug/status info (round, word, typed, score, winner)

## Agent Guardrails For Future Changes

1. Keep protocol changes synchronized across:
   - `src/protocol.rs`
   - server message handling
   - web client decode/apply logic (`static/index.html`)
   - native client decode/apply logic (if still maintained)
2. Preserve server authority for scoring and round transitions.
3. Prefer additive message fields/enums over breaking wire changes.
4. Avoid heavy allocations in per-frame render paths unless cached.
5. If adding graphics features, keep fallback behavior simple and debuggable.
6. The web client is a single self-contained HTML file; keep it that way for simplicity.

## Server Architecture

The server listens on a single TCP port and handles both HTTP and WebSocket:

1. On each new TCP connection, the server peeks at the request headers (without consuming them).
2. If the request contains `Upgrade: websocket`, it routes to the WebSocket game handler.
3. Otherwise, it serves the web client HTML page (or 404).

The web client HTML is embedded into the binary at compile time via `include_str!("../static/index.html")`, making the server fully self-contained.

## Recommended Next Iterations

- add countdown/round-transition effects in the web client
- add reconnect/session logic
- add tests for server round/scoring and protocol serialization
- add mobile/touch input support to the web client
- consider removing the native wgpu client once the web client is fully mature
- add TLS support or deploy behind a reverse proxy for production

## Verification Checklist

After all changes:

1. Run `cargo check`

After significant changes:

1. Run `cargo check`
2. Run the server and open two browser tabs at http://127.0.0.1:9002
3. Verify:
   - both clients receive same word
   - first correct submit increments only one player score
   - leaderboard order matches scores
   - circles resize with score
   - word/typing colors update for local and remote progress
   - name labels appear below circles
   - winner banner displays after each round

