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
- `wgpu` + `winit` for rendering and input
- `font8x8` for bitmap text overlays (word + leaderboard)

## How To Run

```bash
cargo run -- server
cargo run -- client ws://127.0.0.1:9002 Alice
cargo run -- client ws://127.0.0.1:9002 Bob
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
- `src/server.rs` - authoritative game state and round/scoring logic
- `src/client/` - client module directory
  - `mod.rs` - event loop glue, module re-exports, `run_client` entrypoint
  - `net.rs` - websocket networking thread, `NetworkEvent`, `spawn_network`
  - `game.rs` - game state, player sync, physics simulation, input handling
  - `render.rs` - wgpu render state, GPU pipelines, vertex types, draw calls
  - `hud.rs` - bitmap text rasterization (word display, leaderboard)
- `src/circle.wgsl` - player circle shader
- `src/text.wgsl` - bitmap text quad shader

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

The client currently renders:

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
   - client decode/apply logic
2. Preserve server authority for scoring and round transitions.
3. Prefer additive message fields/enums over breaking wire changes.
4. Avoid heavy allocations in per-frame render paths unless cached.
5. If adding graphics features, keep fallback behavior simple and debuggable.

## Recommended Next Iterations

- render typed input and instructions as separate HUD rows
- add countdown/round-transition effects
- add name labels near circles
- add reconnect/session logic
- add tests for server round/scoring and protocol serialization
- further decompose client modules as complexity grows (e.g. separate `sim` from `game`)

## Verification Checklist

After all changes:

1. Run `cargo check`

After significant changes:

1. Run `cargo check`
2. Run one server + two clients
3. Verify:
   - both clients receive same word
   - first correct submit increments only one player score
   - leaderboard order matches scores
   - circles resize with score
   - word/typing colors update for local and remote progress

