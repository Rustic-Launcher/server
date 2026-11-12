# ADR 0021: Main and async instances, import routing and services

- Status: accepted
- Date: 2026-11-10

## Context

The WIT (`scheduler.run-async*`, `http`) promises plugins an off-tick place to block, without
ever touching the world from another thread. The core keeps world state single-threaded
(ADR 0003) and hands the host thread-safe pieces through `SharedHandle`. The SDK relies on
`on-load`/`on-enable` running in both instances (docs/HANDOFF.md).

## Decision

**Two instances per plugin**, instantiated from the same compiled component in two stores:

- **main**: owned by the core's `PluginInstance` (`WasmPlugin`), called only on the server
  thread, through the core (which sets the call context, watchdog tracker, timings and
  `catch_unwind`).
- **async**: in the host's pool (`AsyncEntry`, `Mutex<Option<WasmInstance>>`). A dispatcher
  thread polls `SharedHandle::due_async_tasks` every `async_poll_interval` (2 ms) and gives each
  task to one of `async_workers` threads; a plugin's tasks run one at a time, in order, by the
  worker holding that plugin, so a slow plugin cannot occupy every worker. `AsyncDue.job =
  Some` calls `run-job(task, job, payload)`, else `run-task(task)`; completion goes to
  `async_task_finished`, faults to `report_fault` (the core disables the plugin at the next
  tick) and the poisoned async instance is dropped at once.
- `on-load` and `on-enable` run in **both** (main first; the async instance is marked ready
  after its `on-enable`). A task starts only when the async instance is ready **and** the main
  instance is not executing, so a task never starts before the call that scheduled it
  returned. `on-disable` runs only in main; the async side is retired first (its running call is
  interrupted within ~10 ms by the epoch callback).
- Dropping `WasmPlugin` retires and unregisters its async entry (by identity, so a deferred drop
  never removes a newer incarnation's entry); stores are dropped with them, freeing memory.

**Import routing.** Main-instance imports use `context::api()` (core-owned interfaces, checked to
be this store's plugin) and `context::backend_call` (game). The thread-safe subset (scheduler,
storage, player data, logging, `services.provider`) goes through the store's `AsyncApi` in both
instances. In the async instance, world calls return `host-error::main-thread-only`; functions
without an error channel return an empty value and warn once; registrations are refused;
`server.self-meta` is answered from the manifest (the SDK asks it in both instances);
`worlds.all` and `messaging.behind-proxy` come from a snapshot refreshed by the main thread.
`http.fetch` works only in the async instance (`async-only` otherwise).

**Services.** The `rustic:services/*` imports are always linked. A consumer call (main instance)
looks up the provider in the core's registry and calls the provider's **main** instance through
`Rustic::with_plugin_instance` + downcast to `WasmPlugin`, using the generated export bindings of
the interface. No provider → `no-provider`; the provider is the caller or already executing →
`provider-failed` (warned once, re-entry refused); provider fault → it is disabled,
`provider-failed`; provider without the export → `provider-failed`. From an async instance →
`refused` (providers live on the main thread). The core's `PermissionProvider` hook uses the
same path (`permission-provider.check`), installed at the first load or by
`WasmHost::install`.

**Exports by name.** `rustic:api/plugin` and `rustic:api/handlers` functions are looked up one
by one (semver-aware: a `@0.1.x` export matches), only `on-load`/`on-enable` are required; a
missing `handle-*` makes the event a no-op for that plugin (warned once), a missing `run-job`
fails that job. Imports are linked with wasmtime's semver matching (a guest importing
`wasi:*@0.2.12` links against the host's 0.2 bindings).

## Consequences

- Twice the instantiation and idle memory per plugin (measured in docs/PERFORMANCE.md).
- Async state is a separate copy; data crosses with job payloads, storage and `post-to-main`.
- A plugin can always be disabled promptly, even mid-HTTP.

## Alternatives considered

- **One instance, async calls queued to the tick**: cannot block; HTTP would stall the server.
- **Shared memory between instances**: needs wasm threads and makes every plugin a concurrent
  program.
- **Instantiate the async instance lazily on first async task**: saves memory for plugins that
  never go async, but the SDK needs `on-enable` to have run there before any job; kept simple
  for 0.1, possible later (the entry already has a ready flag).
