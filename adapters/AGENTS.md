# adapters Agent Guide

## Scope
- Houses game-mode logic crates that plug into the shared server core.
- Adapters are loaded by `server`; they are not independent server binaries.
- Each adapter controls prompt generation, progress normalization, correctness checks, and scoring weight.

## Current Adapters
- `keyboarding`: shared-word typing race behavior.
- `arithmetic`: expression-solving race behavior.

## Commands
- Workspace tests: `cargo test --workspace`
- Keyboarding adapter tests: `cargo test -p edif-io-keyboarding-adapter`
- Arithmetic adapter tests: `cargo test -p edif-io-arithmetic-adapter`

## Adapter Extension Contract
- Implement `core::GameAdapter`.
- Return stable, unique `game_key()` identifiers (used by clients/debugging and adapter registry).
- Keep prompt generation deterministic from provided seed when possible.
