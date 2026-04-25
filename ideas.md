# Shared Library Refactoring & Optimization Ideas

Based on an analysis of the `server/` and `client/` workspaces, here are candidates for moving functionality into the `shared/` library, as well as architectural optimizations to improve the game's performance and structure.

## Immediate Candidates for the Shared Library

### 1. Game Constants & Configuration
Both the server and client rely on synchronized but hardcoded game constants. Moving these to a `shared::config` module ensures they never drift out of sync:
- **Spawn Position:** `glam::Vec3::new(8.0, 15.0, -15.0)` is hardcoded in both `server/src/game.rs` and `client/src/scene.rs`.
- **Tick Rates & Timing:** The server ticks at `TICK_HZ = 5` (200ms) but the client attempts to send moves every `50ms` (via `MOVE_SEND_INTERVAL`). These numbers should be centralized so you can easily balance network pressure versus game responsiveness.

### 2. Math & Type Conversions
The network protocol serializes types like positions and rotations as simple `[f32; 3]` and `[f32; 4]` arrays to satisfy `serde`.
- Both the client and server manually convert between these arrays and `glam` types (e.g., `quat_from_array` and `quat_to_array` inside `server/src/game.rs`, and similar logic sprinkled throughout `client/src/scene.rs`).
- **Solution:** Add an `Into` / `From` or helper methods in `shared/src/lib.rs` (e.g., `shared::math`) to uniformly handle `glam` <-> `[f32; N]` array serialization conversions across both workspaces.

### 3. Block Definitions (The "Magic Numbers")
Currently, blocks are identified by "magic numbers" (0 = air, 1, 2, 3). 
- In `shared/src/chunk.rs`, chunk generation hardcodes layers based on `1`, `2`, and `3`.
- In `client/src/mesher.rs`, colors are hardcoded via a `match block_id { 1 => [0.2, 0.8, 0.2]... }` statement.
- **Solution:** Create a `shared::block::BlockType` enum or a registry struct. This would tie the block ID to its properties. The client could read the registry for rendering colors/textures, while the server could read it for physics/collision behaviors (e.g., checking if a block type is solid or fluid).

### 4. Bounds Checking
`server/src/game.rs` implements a `chunk_contains(position: glam::IVec3)` function before processing block placements. The `shared::chunk::Chunk` struct *already* does this bounds checking inside its `set_block` and `get_block` methods. `chunk_contains` can be moved onto `impl Chunk` as a public method so both the server and client can cleanly validate coordinates before initiating raycasts or block updates.

---

## 🚀 Creative & Optimizing Ideas for Shared-Use

By refactoring how the server and client communicate, you can vastly improve the game's performance and cheat resistance. These optimizations will naturally pull major gameplay systems into your `shared/` library.

### 1. Client-Side Prediction & Shared Physics (Authoritative Server)
**The Problem:** Currently, the client calculates its own exact position using `PlayerController` and sends a `ClientMessage::MovePlayer` to the server. The server blindly accepts this position. This means a player could easily hack the client to fly or teleport.
**The Optimization:**
- Move `PlayerController`, movement physics, and collision detection (AABB vs. Voxel) completely into `shared/src/physics.rs`.
- **Protocol Change:** The client sends `ClientMessage::PlayerInput` (e.g., "W pressed, looking at yaw/pitch") rather than exact coordinates. 
- **The Shared Loop:** Both the client and server run the exact same `update_player(&mut player, input, delta_time)` function from the shared library. The client *predicts* where it will be so the game feels lag-free, but the server calculates the *official* position. If the client diverges from the server (due to lag or cheating), the server forcefully corrects the client's position in the next `WorldUpdate`. 

### 2. Deterministic World Generation (Seed-Based Chunk Loading)
**The Problem:** When a player joins, the server sends a `WorldSnapshot` that contains `chunk_blocks: Vec<u8>`. For a single `64x64x64` chunk, this isn't terrible (262KB). But as you scale to multiple chunks, sending entire voxel arrays over the network will freeze the client and crash your bandwidth.
**The Optimization:** 
- Move world generation (currently just a flat heightmap in `Chunk::new`) into a `shared::worldgen` module, utilizing a noise algorithm (like `simdnoise`).
- **Protocol Change:** The server only sends a `world_seed` in the `Welcome` message, along with a tiny list of *dirty/modified* blocks (blocks placed or broken by players).
- **The Shared Loop:** The client's scene initializes by running the shared deterministic generator using the seed, then applies the modified block list. This drops your world-sync payload from hundreds of kilobytes down to a few bytes!

### 3. Binary Serialization
**The Problem:** `protocol.rs` is using `serde_json` as text frames over WebSockets. JSON is highly bloated for high-frequency game loops because arrays of floats `[0.1234, 1.2345, -0.5678]` turn into massive strings.
**The Optimization:** 
- Since both endpoints are written in Rust and share `protocol.rs`, you can switch your `serde` backend to `bincode` or `postcard`.
- **Shared Code impact:** You can expose a `shared::net::serialize` and `shared::net::deserialize` wrapper that seamlessly converts your existing protocol structs into raw `Vec<u8>`. Update the Axum backend and Wasm frontend to send/receive `Message::Binary` instead of `Message::Text`. You'll likely see a 5x-10x reduction in network bandwidth immediately with no structural logic changes.

### 4. Unified Voxel Raycasting logic
**The Problem:** The client relies on `Chunk::raycast()` to figure out what block they are looking at to break/place. The server, however, blindly accepts `ClientMessage::BreakBlock` at any distance. A hacked client could break blocks 1,000 units away.
**The Optimization:**
- Since `Chunk::raycast` is already shared, the server can use it to validate interactions! When the server receives a block break/place event, it runs the shared raycast from the player's current known server-side position and rotation to ensure the requested block coordinate is actually within reach (e.g., `< 10.0` units) and has line-of-sight.