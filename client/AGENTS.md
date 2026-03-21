# client Agent Guide

## Scope

- Contains the only frontend build for all game variants.
- Connects to the single unified websocket server runtime.
- Renders pre-game (name + room create/join) and in-game UI.
- Maintains client-side-only blob simulation for visual movement.

## Key Files

- `src/routes/+page.svelte`: lobby (name entry, room create/join).
- `src/routes/room/[code]/+page.svelte`: in-game UI (arena, blobs, prompt, debug panel).
- `src/lib/game/connection.svelte.ts`: shared reactive WebSocket state, connect/disconnect, message handlers.
- `src/lib/game/protocol.ts`: shared message typings and decode helper.
- `src/lib/game/sim.ts`: client-side clumping/orbit simulation.

## Commands

- Dev server: `npm run dev`
- Type check: `npm run check`
- Lint/format: `npm run lint`, `npm run format`
- Unit tests: `npm run test:unit -- --run`

## Implementation Rules

- Keep styles intentionally minimal.
- Do not move gameplay authority into the client (sizes, winners, room state remain server-driven).
- Keep debug panel available and lightweight for socket diagnostics.
- Keep protocol typings in `src/lib/game/protocol.ts` aligned with `core/src/protocol.rs`.
