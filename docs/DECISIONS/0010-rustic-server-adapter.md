# ADR 0010: The rustic-server adapter (Horizon trait mod ↔ rusticmc-core)

- Status: accepted
- Date: 2026-11-11

## Context

ADR 0001 links Rustic into Horizon's dedicated engine as one trait mod. The adapter
(`crates/rusticmc-server`) must meet Horizon's constraints (docs/ARCHITECTURE.md §1-§3): the server
runs on the process main thread with an 8 MB stack; a panic in a hook crash-stops the server and
Horizon's panic hook writes a crash report for every panic, caught or not; 44 server hooks get no
`Host`; `Host` calls fire hooks again; `status_response`, `handshake` and the login hooks run on
connection threads; the supervisor (`horizon-server`) spawns `engine.path` with its own command line;
Horizon's logger prints only `horizon*` targets on the console. `rusticmc_core::Rustic` is `!Send`.

## Decision

- **One executable, always dedicated.** `rustic-server` calls `entry::main_with` with
  `RusticHooks`, after `name_mods(["rustic"])`, with Horizon's allocator and `api_level::MARK`. The
  game reads `std::env::args`, so without `--dedicated` the binary re-executes itself with
  `--dedicated --nogui` (and `--resources` found from `RUSTIC_RESOURCES`, Horizon's defaults or the
  checkout); the supervisor's command line passes through untouched. `build.rs` links
  `/STACK:8388608` (MSVC) / `-Wl,--stack,8388608` (GNU) on Windows, as Horizon's own executable.
- **Runtime in a thread local of the server thread**, started at the first `server_tick` or
  `register_commands`, dropped explicitly at `server_stopping` (the thread-local cells are
  `ManuallyDrop`: at process exit Windows has already ended the host's worker threads, and joining
  them in a TLS destructor panicked).
- **Every hook body in `catch_unwind`** (`hooks::guard`) with a per-thread "guarded" depth; a panic
  hook installed in front of Horizon's logs a caught panic as one line and leaves every other panic
  to Horizon (no crash report for a panic Rustic contains).
- **Backend with or without a host.** With a host, calls go to the game; without (no-host hooks),
  reads come from a snapshot taken at each `ServerTickStart` and writes are queued and run at
  `ServerTickEnd`. Nested entries (re-entrancy) get their own queue, merged into the outer one, and the
  same snapshot. Events are built only when `has_listeners_for` says a plugin listens.
- **Connection-thread pings** are handed to the server thread through a bounded queue and answered
  within 250 ms, only when a plugin listens to `server-list-ping`.
- **Console visibility.** Plugin and Rustic log lines (target `rustic`) are echoed to stdout from
  Horizon's `log_line` hook, as the log file has them; console command replies use `Host::feedback`,
  so RCON, `horizon-server cmd` and the panel receive them.
- **Commands**: the core's usages as `NativeCommand`s with their permission nodes (namespaced
  `<plugin>:<name>` labels left out: Horizon's command words take no `:`), each with
  `replace_vanilla(true)` so a plugin's `/msg` replaces vanilla's as on Paper (2026-11-11; before,
  Horizon dropped such usages and kept vanilla's); Rustic's permission engine answers
  `permission_check` for every node.
- **Loader**: `rusticmc-host`'s `WasmHost` for every package, native in-process plugins by id for
  the built-in test plugins (`RUSTIC_DEMO_PLUGINS=1`).

## Consequences

- The server never sees a Rustic panic; a deliberately panicking hook (`RUSTIC_TEST_PANIC=<hook>`) is
  logged and ignored (crates/rusticmc-server/README.md, "Evidence").
- Writes from no-host hooks land at the end of the tick, not instantly (documented for plugin
  authors with the no-host event list).
- Directly started, the server is two processes (the launcher of the re-exec and the server); under
  the supervisor or `rustic run --` with `--dedicated`, one.
- Namespaced command labels are typed only when Horizon accepts `:` (upstream proposal).

## Alternatives

- A patch adding `entry::main_with_args` to Horizon: cleaner than re-executing, but a patch for
  something a process can do itself.
- An `Arc<Mutex<Rustic>>` reachable from every thread: the core is designed single-threaded (ADR 0003)
  and the connection-thread hook is one ping; a hand-off is simpler and keeps plugins on one thread.
- Logging under a `horizon*` target to reach the console: it would drop the `[rustic]` prefix and the
  console's line format; the `log_line` echo keeps the file's exact lines.
