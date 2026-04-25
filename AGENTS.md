# Repository Guidelines

## Project Structure & Module Organization

This is a Rust workspace with three crates:

- `client/`: WGPU/Winit game client. Rust source lives in `client/src/`; WGSL shaders sit beside renderer code as `*.wgsl`; `client/index.html` supports browser builds.
- `server/`: Axum/Tokio server entry point in `server/src/main.rs`.
- `shared/`: Common game data and math-facing types used by client and server.

Keep cross-crate contracts in `shared` and avoid duplicating protocol or world data structures in `client` and `server`.

## Build, Test, and Development Commands

- `cargo check --workspace`: type-check every crate quickly.
- `cargo build --workspace`: build all native workspace targets.
- `cargo test --workspace`: run unit and integration tests when present.
- `cargo run --bin server`: start the local server.
- `cargo run --bin client`: run the native client.
- `cd client && trunk serve --features webgpu`: serve the browser client with WebGPU enabled.

Use root workspace commands for changes that touch shared APIs.

## Coding Style & Naming Conventions

Use Rust 2024 idioms and standard `rustfmt` formatting. Run `cargo fmt --all` before submitting broad edits. Prefer clear modules over large files; renderer, mesh, scene, GPU setup, and platform code are already separated.

Use `snake_case` for functions, modules, variables, and WGSL filenames; `PascalCase` for types and traits; `SCREAMING_SNAKE_CASE` for constants. Keep platform-specific code under `client/src/platform/`.

## Testing Guidelines

There is no dedicated test suite yet. Add focused Rust unit tests next to pure logic, especially in `shared` and deterministic mesh/world code. Use integration tests for crate-level behavior that needs public APIs. Run `cargo test --workspace` and `cargo check --workspace` before opening a PR.

For rendering or platform changes, also manually verify the native client and the browser client path affected by the change.

## Commit & Pull Request Guidelines

Existing commits use short summaries such as `web fixes, though with a new error` and `platform seperation`. Keep commit subjects concise and specific; mention the subsystem when helpful, for example `client: fix camera resize handling`.

Pull requests should include a short description, commands run, affected targets (`client`, `server`, `shared`, or web), and screenshots or recordings for visible rendering/UI changes.

## Agent-Specific Instructions

Do not commit generated build output from `target/`. Treat `Cargo.lock` as workspace-owned and update it only when dependency changes require it. Keep shader edits close to the Rust renderer code that consumes them.
