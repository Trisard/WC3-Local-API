# `wc3_local_api` &nbsp;·&nbsp; [![Crates.io](https://img.shields.io/crates/v/wc3_local_api)](https://crates.io/crates/wc3_local_api) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE) ![Platform: Windows](https://img.shields.io/badge/platform-Windows-0078d7?logo=windows)

> Warcraft III — local WebSocket bridge for Rust.

A standalone Rust library for connecting to the **Warcraft III local WebSocket API**.

Automatically finds the running WC3 process, extracts its session GUID from memory, detects its listening port, and gives you a live stream of raw WebSocket messages — with no Blizzard account, no network request, and no external dependency beyond the game itself.

> **Platform:** Windows only (requires access to WC3 process memory).

---

## Installation

```toml
[dependencies]
wc3_local_api = "0.1"
```

---

## Usage

### Automatic — just works

```rust
use wc3_local_api::connect_auto;

fn main() {
    let handle = connect_auto().expect("Warcraft III must be running");

    for msg in handle.iter() {
        println!("{msg}");
    }
}
```

### Wait for the game to start

```rust
use wc3_local_api::connect_when_ready;
use std::time::Duration;

fn main() {
    // Blocks until WC3 is detected, polling every 3 seconds
    let handle = connect_when_ready(Duration::from_secs(3));

    for msg in handle.iter() {
        println!("{msg}");
    }
}
```

### React to connection events

```rust
use wc3_local_api::{connect_when_ready, ConnectionStatus};
use std::{thread, time::Duration};

fn main() {
    let handle = connect_when_ready(Duration::from_secs(3));

    // Monitor connection lifecycle in a separate thread
    let status_rx = handle.status;
    thread::spawn(move || {
        for status in status_rx {
            match status {
                ConnectionStatus::Connected { port }  => eprintln!("Connected on port {port}"),
                ConnectionStatus::Disconnected        => eprintln!("Disconnected, retrying..."),
                ConnectionStatus::Reconnecting        => eprintln!("Reconnecting in 5s..."),
                ConnectionStatus::Connecting { port } => eprintln!("Trying port {port}..."),
            }
        }
    });

    // Consume messages in the main thread
    for msg in handle.messages {
        // msg is a raw JSON string — parse it however you like
        println!("{msg}");
    }
}
```

### Manual configuration

```rust
use wc3_local_api::{connect, get_w3_guid, get_w3_port, ConnectionConfig};
use std::time::Duration;

fn main() {
    let ports = get_w3_port().expect("WC3 not found");
    let guid  = get_w3_guid().expect("GUID not found in memory");

    let config = ConnectionConfig::new(ports, format!("/webui-socket/{guid}"))
        .with_retry_delay(Duration::from_secs(10))
        .with_connect_timeout(Duration::from_secs(3));

    let handle = connect(config);

    for msg in handle.iter() {
        println!("{msg}");
    }
}
```

---

## Message format

Messages are raw JSON strings emitted by the game. Common types observed:

```json
{ "messageType": "GameList",       "payload": { "games": [ ... ] } }
{ "messageType": "GameListUpdate", "payload": { "game": { ... } } }
{ "messageType": "GameListRemove", "payload": { "id": 88 } }
```

This library delivers the raw strings — parsing and interpretation are up to you.

---

## API reference

### Functions

| Function                       | Description                                                                                      |
| ------------------------------ | ------------------------------------------------------------------------------------------------ |
| `connect_auto()`               | Discover and connect. Returns `Err` immediately if WC3 is not running.                           |
| `connect_when_ready(interval)` | Like `connect_auto`, but polls until the game is found. Blocks the calling thread.               |
| `connect(config)`              | Connect with explicit ports and path. Never fails — the background thread retries automatically. |
| `get_w3_guid()`                | Extract the session GUID from WC3 process memory.                                                |
| `get_w3_port()`                | Return all TCP ports currently listened to by WC3.                                               |

### `ConnectionHandle`

```rust
pub struct ConnectionHandle {
    pub messages: Receiver<String>,           // raw WebSocket text frames
    pub status:   Receiver<ConnectionStatus>, // lifecycle events
}
```

The handle's background thread reconnects automatically on disconnect. Dropping the handle (or calling `.shutdown()`) stops it cleanly.

### `ConnectionConfig`

```rust
ConnectionConfig::new(ports, path)
    .with_retry_delay(Duration)      // default: 5s
    .with_connect_timeout(Duration)  // default: 2s per port
```

### Errors

```rust
pub enum Wc3Error {
    ProcessNotFound,    // WC3 is not running
    CannotOpenProcess,  // insufficient permissions — try running as administrator
    GuidNotFound,       // process found but GUID not in memory yet
    PortNotFound,       // process found but no listening port detected
    Netstat(String),    // failed to query socket table
    WebSocket(..),      // tungstenite error
}
```

---

## CLI tool

A minimal `wc3-spy` binary is included. It dumps every WebSocket message to stdout and prints connection events to stderr.

Pre-built Windows binaries are available on the [Releases](../../releases) page.

```
wc3-spy.exe
```

Or build it yourself:

```bash
cargo build --release --bin wc3-spy
# → target/release/wc3-spy.exe
```

---

## Notes

- `get_w3_guid` scans the full WC3 virtual address space (~1s). This is normal.
- `get_w3_port` / `netstat` may require administrator privileges on some Windows configurations.
- The library is **read-only**: it only calls `ReadProcessMemory` with `PROCESS_VM_READ`. It does not inject code or modify the game process.

---

## License

MIT
