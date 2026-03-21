# Edif.io Monorepo Agent Guide

## Architecture Snapshot
- Monorepo with one Rust server binary (`server`) and one shared web client (`client`).
- Game modes are adapters loaded into the unified server at startup, not separate server processes.

## Repository Structure
- `client`: SvelteKit frontend (single shared client build).
- `core`: shared Rust core for protocol, rules, and websocket runtime.
- `adapters/*`: game-mode adapters implementing prompt/validation/scoring behavior.
- `server`: single Rust server binary hosting all registered adapters.

## Product Boundaries
- Server is authoritative for room state, prompt lifecycle, scoring, player size/color, and win conditions.
- Client is authoritative for visual blob positioning only (gravity, orbit, motion).
- Adapters own game-specific prompt generation and answer correctness.

## Common Commands
- Rust workspace tests: `cargo test --workspace`
- Unified server: `cargo run -p server`
- Client dev: `npm run dev` (from `client`)
- Client check/lint/tests: `npm run check && npm run lint && npm run test:unit -- --run`

## Conventions
- Keep websocket payloads JSON, tagged with `type` in camelCase.
- Preserve server/client boundary: never add server-authoritative blob positions.
- Prefer additive adapter interfaces so new game modes can be added without breaking existing ones.

## CI Workflow Guidance
- CI is intentionally split: `/.github/workflows/ci-autofix.yml` for branch `push` auto-fixes and `/.github/workflows/ci-validate.yml` for `pull_request`/`main` validation.
- Use `push` for branch automation (format/fix commits), and use `pull_request` for merge-gating validation.
- Keep monorepo checks path-aware: Rust paths (`Cargo.*`, `core/**`, `adapters/**`, `server/**`), web paths (`client/**`), and workflow paths (`.github/workflows/**`).
- Prefer separate validate jobs (`Validate (rust)`, `Validate (web)`) instead of matrix-only naming when branch-protection clarity matters.
- Avoid mixed-event job gating in one workflow when possible; separate workflows reduce noisy "Skipped" checks in PR UI.
- When renaming workflows/jobs, update GitHub branch protection required checks to match new names.
