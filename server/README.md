# Voxel Game Server

game server for the voxel game.

## Server Arguments

Run with no argument for local-only:

```sh
cargo run --bin server
```

This binds to:

```text
127.0.0.1:3000
```

Pass a bind address to choose where the server listens:

```sh
cargo run --bin server -- 127.0.0.1:4000
cargo run --bin server -- 0.0.0.0:3000
```

Use `127.0.0.1:PORT` when only clients on the same computer should connect.
Use `0.0.0.0:PORT` when other computers should be able to connect through LAN, VPN or port forwarding.

## Local Play

Start the server:

```sh
cargo run --bin server
```

Then start one or more clients:

```sh
cargo run --bin client
```

## Internet or LAN Play

Start the server on an address reachable by other computers:

```sh
cargo run --bin server -- 0.0.0.0:3000
```

Then clients connect to the server computer's reachable address:

```sh
cargo run --bin client -- 192.168.1.50:3000
cargo run --bin client -- 100.64.1.2:3000
cargo run --bin client -- ws://203.0.113.10:3000/ws
```

For internet play, make sure TCP port `3000` reaches the server:

- Use Tailscale and give players the VPN IP.
- Forward TCP port `3000` on your router to the server computer.
