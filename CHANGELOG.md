# Changelog

## [0.1.0] — 2026-06-06

### Initial release

First public release of `wc3_local_api`.

**Library**
- `connect_auto()` — one-call connection: finds WC3 process, extracts GUID from memory, detects port, connects
- `connect_when_ready(interval)` — same, but waits for the game to start instead of returning an error
- `connect(config)` — manual connection with explicit ports and path
- `get_w3_guid()` / `get_w3_port()` — low-level discovery functions for custom use cases
- `ConnectionConfig` builder with configurable retry delay and connect timeout
- `ConnectionHandle` with separate channels for messages and connection status events
- Automatic reconnection on disconnect
- Clean shutdown on drop

**CLI — `wc3-spy.exe`**
- Dumps raw WebSocket messages to stdout
- Prints connection events to stderr
- Standalone binary, no dependencies required
