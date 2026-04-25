# Voxel Game Client

Native and browser frontend for the voxel game.

## Browser Multiplayer

Serve the browser client with Trunk:

```sh
cd client && trunk serve --features webgpu
```

The browser page shows a bare server URL field and a Connect button. The default is:

```text
ws://127.0.0.1:3000/ws
```

Start the game server separately before connecting:

```sh
cargo run --bin server
```

For a remote server, enter the reachable WebSocket URL in the browser field before pressing Connect.

## Native Multiplayer Arguments

The native client always connects to a server. Run it with no argument for the default local server:

```sh
cargo run --bin client
```

This connects to:

```text
ws://127.0.0.1:3000/ws
```

pass a port to connect to localhost on another port:

```sh
cargo run --bin client -- 4000
```

This connects to:

```text
ws://127.0.0.1:4000/ws
```

pass `host:port` for another machine:

```sh
cargo run --bin client -- 100.64.1.2:3000
```

This connects to:

```text
ws://100.64.1.2:3000/ws
```

pass a full WebSocket URL:

```sh
cargo run --bin client -- ws://203.0.113.10:3000/ws
cargo run --bin client -- wss://game.example.com/ws
```

## Local Play

Start server in one terminal:

```sh
cargo run --bin server
```

Start one or more native clients in separate terminals:

```sh
cargo run --bin client
```

## Internet or LAN

If the server is running on another computer, connect to that computer's reachable address:

```sh
cargo run --bin client -- 192.168.1.50:3000
cargo run --bin client -- 100.64.1.2:3000
cargo run --bin client -- ws://203.0.113.10:3000/ws
```

For internet play, the server computer must be reachable from the client. Use one of:

- A VPN overlay like Tailscale, then connect to the VPN IP.
- Port forward on TCP port `3000`, then connect to the public IP.
