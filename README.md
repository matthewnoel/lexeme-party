# edif.io

edif.io is a multiplayer prompt-race game with:

- a single SvelteKit client (`client`)
- shared Rust core runtime (`core`)
- pluggable game adapters (`adapters/*`)
- one game server binary (`server`)

Players choose a game mode on the pregame screen when creating a room. Room game mode is authoritative for all players in that room.

## Prerequisites

### Rust

[Installation instructions](https://rust-lang.org/tools/install/)

### Node.js version `./client/.nvmrc`

Recommended Node Version Managers: [fnm](https://github.com/Schniz/fnm) or [nvm](https://github.com/nvm-sh/nvm)

## Run Locally

### 1. Start the unified game server

```sh
cargo run -p server
```

### 2. Start the client

In a second terminal:

```sh
cd client
nvm use
npm install
npm run dev
```

## Test and Validation

```sh
cargo test --workspace
```

```sh
cd client
npm run check
npm run lint
npm run test:unit -- --run
```

## Adding a New Game Mode

1. Add a new adapter crate under `adapters/` implementing `core::GameAdapter`.
2. Register the adapter in `server/src/main.rs`.
3. Add the mode to the client pregame selector in `client/src/routes/+page.svelte`.

## Current Game Modes

 - Keyboarding: *Type the global prompt correctly first to gain points.*
 - Arithmetic: *Proof of concept intended to implement the functionality of: [https://github.com/matthewnoel/arithmetic-practice](https://github.com/matthewnoel/arithmetic-practice)*
