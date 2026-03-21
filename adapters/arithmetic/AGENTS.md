# arithmetic adapter Agent Guide

## Scope
- Implements arithmetic prompt racing for edif.io.
- Players race to submit the correct numeric result for each expression.
- Registered into the server via `server/src/main.rs`.

## Key Files
- `src/lib.rs`: adapter implementation and adapter-specific tests.

## Commands
- Tests: `cargo test -p edif-io-arithmetic-adapter`

## Notes
- Baseline prompts are simple addition expressions.
- Correctness requires exact numeric equality after trimming input.
