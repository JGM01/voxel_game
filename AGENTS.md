# Repository Guidelines

## Project Structure & Module Organization

This is a Rust workspace with three crates:

- `client/`: WGPU/Winit game client. Rust source lives in `client/src/`; WGSL shaders sit beside renderer code as `*.wgsl`; `client/index.html` supports browser builds.
- `server/`: Axum/Tokio multiplayer server entry point in `server/src/main.rs`.
- `shared/`: Common game data and math-facing types used by client and server.

Keep cross-crate contracts in `shared` and avoid duplicating protocol or world data structures in `client` and `server`.

Current multiplayer-related client modules:

- `client/src/net.rs`: native WebSocket client bridge. It parses server address CLI arguments and sends/receives `shared::protocol` messages through channels.
- `client/src/player.rs`: player-owned movement state. The camera mirrors the player transform each frame; gameplay interactions should use the player transform, not the camera as the authority.
- `client/src/scene.rs`: applies server snapshots/updates, owns chunk state, remeshes chunks only when block data changes, and renders simple remote-player markers.

Current server state:

- `server/src/game.rs`: authoritative in-memory world loop. It tracks connected players, one shared chunk, pending dirty player/block changes, and broadcasts `WorldUpdate`s at `TICK_HZ`.
- `server/src/net.rs`: WebSocket upgrade and protocol translation layer.
- The server intentionally does not echo a client's own movement/block updates back to that same client, so the native client uses optimistic local updates.

## Build, Test, and Development Commands

- `cargo check --workspace`: type-check every crate quickly.
- `cargo build --workspace`: build all native workspace targets.
- `cargo test --workspace`: run unit and integration tests when present.
- `cargo run --bin server`: start the local-only server on `127.0.0.1:3000`.
- `cargo run --bin server -- 0.0.0.0:3000`: start a server reachable by other machines, assuming firewall/router/VPN setup allows it.
- `cargo run --bin client`: run the native client and connect to `ws://127.0.0.1:3000/ws`.
- `cargo run --bin client -- 4000`: connect to `ws://127.0.0.1:4000/ws`.
- `cargo run --bin client -- 100.64.1.2:3000`: connect to another host using `ws://100.64.1.2:3000/ws`.
- `cargo run --bin client -- ws://203.0.113.10:3000/ws`: connect using a full WebSocket URL.
- `cd client && trunk serve --features webgpu`: serve the browser client with WebGPU enabled.

Use root workspace commands for changes that touch shared APIs.

For web compile validation, use `cargo check -p client --target wasm32-unknown-unknown`. Native networking is implemented; web networking is currently a compile-safe no-op/stub path.

## Coding Style & Naming Conventions

Use Rust 2024 idioms and standard `rustfmt` formatting. Run `cargo fmt --all` before submitting broad edits. Prefer clear modules over large files; renderer, mesh, scene, GPU setup, and platform code are already separated.

Use `snake_case` for functions, modules, variables, and WGSL filenames; `PascalCase` for types and traits; `SCREAMING_SNAKE_CASE` for constants. Keep platform-specific code under `client/src/platform/`.

Keep multiplayer protocol wire shapes in `shared/src/protocol.rs`. Do not create parallel client/server protocol enums. If a protocol change is needed, update shared serialization tests and both consumers.

Movement and interaction logic should remain player-owned. The camera is a view object mirrored from `Player` for now, not the gameplay actor. This matters for future third-person or smart-follow camera work.

## Testing Guidelines

There is no dedicated test suite yet. Add focused Rust unit tests next to pure logic, especially in `shared` and deterministic mesh/world code. Use integration tests for crate-level behavior that needs public APIs. Run `cargo test --workspace` and `cargo check --workspace` before opening a PR.

For rendering or platform changes, also manually verify the native client and the browser client path affected by the change.

For multiplayer changes, prefer focused tests around pure state transitions:

- address parsing in `client/src/net.rs`
- protocol serialization in `shared/src/protocol.rs`
- server world command handling in `server/src/game.rs`
- chunk dirty/remesh decisions and server update application in `client/src/scene.rs`

When changing native networking or shared client code, run:

- `cargo fmt --all`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo check -p client --target wasm32-unknown-unknown`

Manual multiplayer smoke test:

1. Start `cargo run --bin server`.
2. Start two native clients with `cargo run --bin client`.
3. Confirm both clients share chunk edits and remote player marker movement.
4. Confirm movement-only updates do not trigger chunk remeshing.

## Commit & Pull Request Guidelines

Existing commits use short summaries such as `web fixes, though with a new error` and `platform seperation`. Keep commit subjects concise and specific; mention the subsystem when helpful, for example `client: fix camera resize handling`.

Pull requests should include a short description, commands run, affected targets (`client`, `server`, `shared`, or web), and screenshots or recordings for visible rendering/UI changes.

## Agent-Specific Instructions

Do not commit generated build output from `target/`. Treat `Cargo.lock` as workspace-owned and update it only when dependency changes require it. Keep shader edits close to the Rust renderer code that consumes them.

Networking notes:

- Server bind defaults to `127.0.0.1:3000`; use `0.0.0.0:3000` only when remote clients should connect.
- Internet play needs an externally reachable server address: VPN overlay, port forwarding, or hosted VPS.
- Client CLI accepts no arg, port-only, `host:port`, or full `ws://`/`wss://` URL.
- `Esc` releases cursor grab. `Q` quits the client.
