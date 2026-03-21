# keyboarding adapter Agent Guide

## Scope
- Implements word-based prompt racing for edif.io.
- Players race to type the shared word correctly; first correct submission wins growth.
- Registered into the server via `server/src/main.rs`.

## Key Files
- `src/lib.rs`: adapter implementation and adapter-specific tests.

## Commands
- Tests: `cargo test -p edif-io-keyboarding-adapter`

## Notes
- Correctness is case-sensitive exact-match for baseline behavior.
- Prompt words are selected from a deterministic in-crate list via the seed.
