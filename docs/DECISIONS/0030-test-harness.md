# ADR 0030: The plugin test harness (`rusticmc-test`)

- Status: accepted
- Date: 2026-11-11

## Context

Plugin authors need to test plugins without a game server, as Bukkit developers do with
MockBukkit. A Rustic plugin is a WebAssembly component whose behaviour depends on three layers:
the guest SDK, the host's bindings (`rusticmc-host`) and the runtime (`rusticmc-core`: events,
commands, permissions, scheduler, storage). A mock that re-implements those layers would test a
different program than the one that ships.

## Decision

1. **Mock only the game.** `MockServer` is a real `Rustic` runtime (fake clock, temporary
   server directory) plus `MockBackend`, an in-memory implementation of **every** `Backend`
   method over a `GameState` (worlds with block storage and flat terrain, players with 41-slot
   inventories and vanilla stacking, menus, entities, scoreboards, teams, boss bars, tab list,
   plugin channels, a few vanilla commands). Plugins load through the real host, so tests run
   the exact `.wasm` that ships. Native closure plugins (`NativeLoader`) load through the same
   runtime for the harness's own tests.
2. **Simulated actions follow the server's rules.** Each `MockPlayer` action fires the event the
   server fires and applies the returned outcome (cancelled chat not broadcast, cancelled break
   keeps the block, `drop-items`, death keep-inventory/level and drops, respawn location, menu
   clicks cancelled by default, moves below the threshold not reported...). Events caused by a
   plugin's own calls (teleport, game mode) are fired synchronously like the runtime expects
   (the calling plugin is skipped as busy); deaths, kicks and menu closes requested by a plugin
   are queued and processed when the outermost harness operation returns.
3. **Handles share one `Rc<Harness>`**, so `MockPlayer` and `MockWorld` act and assert without
   borrowing the server; the server is `!Send`, like the runtime, and lives on its test's thread.
4. **Deterministic time.** `tick()` advances the fake clock 50 ms and runs the scheduler. Async
   tasks of native plugins run inline on the test thread. For WebAssembly plugins the host's
   async pool (one worker) owns the async queue; after a tick in which an async task became due
   or the async bookkeeping changed, the harness waits until nothing is due and the pool's
   in-flight count (`HostStats::async_in_flight`: tasks taken from the scheduler and not yet
   finished, fault reporting included) is 0. The host is created lazily at the first `.wasm` load, so native-only
   servers start no engine. A server mixing native async tasks with WebAssembly plugins hands
   every async task to the host pool (documented limitation).
5. **`build_plugin(path)`** builds a plugin crate with `cargo build --target wasm32-wasip2
   --release --message-format=json` (cached per process; cargo is incremental) and reads the
   artifact path from cargo's JSON. Running cargo from inside `cargo test` with the same target
   directory works (cargo releases the build lock before running tests). Crates without a
   manifest (fixtures) get one synthesised from the package name.
6. **Logs** are captured by a process-wide `log` logger that routes records to the innermost
   live server of the logging thread, so parallel tests never mix their consoles.

## Consequences

* A passing harness test exercises the SDK, the host bindings and the runtime as deployed.
* The first test run of a plugin crate compiles wasmtime (minutes); later runs take seconds.
  The on-disk compiled-module cache is shared under `<tmp>/rusticmc-test-cache`.
* The async wait is an exact handshake on the pool's in-flight count (it replaced a 5 ms
  quiescence window that was flaky under load, 2026-11-11). Async tasks still run on the pool's
  worker thread, not inline on the test thread.
* Physics, AI, terrain generation and item use effects are not simulated.

## Alternatives

* A fake `PluginInstance` interpreting the SDK natively (compile the plugin for the host target
  and call its Rust code): tests would skip the component boundary, the host and the async
  instance model, where most integration bugs live. Rejected.
* Recording-only backend (MockBukkit without world rules): cheaper, but plugin tests would have
  to re-implement what "cancelled" means for every event. Rejected.
* A real server in tests (Horizon headless): far too slow and heavy for unit-level tests.
