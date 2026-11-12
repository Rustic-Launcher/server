# ADR 0041: Rustic Bridge IPC: nested request/reply frames over loopback TCP

- Status: accepted
- Date: 2026-11-11

## Context

The sidecar (ADR 0040) needs a channel between `rustic-server` and the JVM that works on Linux
(primary) and Windows, needs nothing outside both standard libraries, and supports *nested*
calls: while the server waits for a Bukkit listener to finish, the listener calls the API
(`player.getHealth()`, `sendMessage`, `hasPermission`), which must be answered by the server
synchronously, possibly calling into another Bukkit plugin (a `dispatchCommand` running another
plugin's command).

Options: anonymous pipes on the child's stdin/stdout, a Unix domain socket (std on Unix only;
Rust's std has no AF_UNIX on Windows), Windows named pipes (needs a crate), loopback TCP.

## Decision

- **Loopback TCP**, `127.0.0.1`, ephemeral port, `TCP_NODELAY` both ends. The host listens, the
  JVM connects and first sends `RUSTIC-BRIDGE/1 <token>\n` with a 128-bit token passed in the
  `RUSTIC_BRIDGE_TOKEN` environment variable (another local process cannot take the connection).
  stdout/stderr stay free for logs (`R|LEVEL|source|text` lines forwarded to the `log` crate,
  target `bukkit`), so a plugin printing to `System.out` cannot corrupt the protocol.
- **Frames**: `u32 length (big-endian) | u8 kind | u32 id | payload`, kind 1 request, 2 reply,
  3 error reply (payload: the exception text); payload UTF-8 JSON, a request is an object with
  `op`. Ids are per direction.
- **Nesting**: both sides are single-threaded on the connection. A side waiting for its reply
  serves the other side's requests on the same thread, so replies come back in LIFO order and
  no lock or reader thread is needed. On the Java side "the thread that serves the host" is the
  Bukkit main thread.
- **Messages**: host → JVM `hello`, `scan`, `load`, `enable`, `disable`, `unload`, `event`,
  `command`, `complete`, `tick`, `shutdown`, `ping`; JVM → host `subscribe`, `has_permission`,
  `declare_permission`, `dispatch_command`, `send_message`, `broadcast`, `player`, `player_set`,
  `teleport`, `kick`, `title`, `action_bar`, `sound`, `server`, `offline_player`, `world`,
  `world_set`, `block`, `set_block`, `disable_plugin`. Events travel as the serde form of
  `rusticmc_core::model::Event` (`{"player-chat": {...}}`); the JVM returns the record with the
  mutable fields and `cancelled` updated, and the core applies only those.
- **Deadline**: the host's socket read timeout is `call_timeout_ms`; a timeout means a stuck
  plugin and the JVM is killed (ADR 0040).

## Consequences

- One loopback round trip is the floor of every call: 38 µs p50 on the Windows dev machine
  (`docs/BRIDGE.md`); Linux loopback is typically several times cheaper.
- JSON is easy to debug and to evolve (unknown keys are ignored) at a few µs per message; the
  hot events (`player-move`, `player-chat`) can move to a binary encoding later without changing
  the framing.
- u64 values (UUID halves) above `Long.MAX_VALUE` are kept exact by the Java JSON reader (it
  writes back the digits it read).

## Alternatives considered

- **stdin/stdout pipes**: no port, no token, but any `System.out.println` in a plugin (common)
  would corrupt the stream unless every write path is hijacked, including native code.
- **Unix domain socket**: Linux-only in Rust's std; worth adding as a Linux fast path later.
- **Shared memory ring buffers**: lowest latency, but needs `unsafe`, futex/event wakeups and a
  native library on the Java side; not before the simple path is the bottleneck.
- **gRPC / protobuf**: dependencies and code generation on both sides for no gain here.
