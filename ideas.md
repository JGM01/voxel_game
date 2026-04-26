## Refactor ideas

### Client-Side Prediction & Server Source-of-Truth
**Problem:** The client calculates its own exact position using `PlayerController` and sends a `ClientMessage::MovePlayer` to the server. The server blindly accepts this position, meaning a player could hack the client to teleport.
**Solution:**
- Move `PlayerController`, movement physics, and collision detection (AABB vs. Voxel) completely into `shared/src/physics.rs`.
- The client sends `ClientMessage::PlayerInput` (e.g., "W pressed, looking at yaw/pitch") rather than exact coordinates. 
- Both the client and server run the exact same `update_player(&mut player, input, delta_time)` function from the shared library. The client predicts where it will be so the game has no latency, but the server calculates the official position. If the client diverges from the server (due to lag or cheating), the server should correct the client's position in the next `WorldUpdate`. 

### Binary Serialization
**Problem** `protocol.rs` is using `serde_json` as text frames over WebSockets. JSON is bloated bc float arrays `[0.1234, 1.2345, -0.5678]` turn into large strings.
**Solution** 
- Both endpoints share `protocol.rs`, just switch the `serde` backend to `bincode` or `postcard`.

### Unified Voxel Raycasting logic
**Problem** The client relies on `Chunk::raycast()` to figure out what block they are looking at to break/place. The server just blindly accepts `ClientMessage::BreakBlock` at any distance. A hacked client could break blocks 1,000 units away.
**Solution**
- Since `Chunk::raycast` is already shared, the server can use it to validate interactions.
