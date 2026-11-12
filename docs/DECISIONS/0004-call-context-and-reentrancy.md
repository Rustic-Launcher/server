# ADR 0004: Call context and re-entrancy

- Status: accepted
- Date: 2026-11-10

## Context

A wasmtime host function only receives `&mut StoreData` (which must be `'static`). From inside
an import such as `events.subscribe` or `players.send-message` the host needs (1) the core state
of the `Rustic` runtime and (2) the `&mut dyn Backend` the engine handed to the current runtime
entry point (`fire`, `tick`, `dispatch_command`...). The backend borrow is short-lived and
non-`'static`; storing it in `StoreData` is impossible without `unsafe`, and passing it through
every wasm call is not something wasmtime offers. Calls nest: a plugin's import can make the
engine call back into the runtime (`server.dispatch-command` → Horizon → `player-command`
event → another plugin), which hands a *new* backend borrow to the runtime.

## Decision

`rusticmc_core::context` keeps three thread-local, scoped slots, set by the runtime and
restored on exit by drop guards (also during unwinding):

1. **Backend**: `with_backend(&mut dyn Backend, f)` installs the borrow for the closure;
   `backend_mut(|b| ...)` lends it out. While lent, the slot reads as empty, so a nested
   `backend_mut` returns `Err(HostError::Other("... re-entrant access"))` instead of aliasing a
   `&mut` or panicking; with no backend installed (async threads, outside an entry point) it
   returns `Err(HostError::MainThreadOnly)`. A nested `with_backend` (the engine calling back
   with a fresh borrow) shadows the slot and restores it. Implemented with `scoped-tls-hkt`
   (`scoped_thread_local!(static mut BACKEND: for<'a> BackendSlot<'a>)`, a higher-kinded
   scoped TLS): this crate contains **no `unsafe`**; the crate's own `unsafe` is a few audited
   lifetime casts guarded by the scope.
2. **Call stack**: `with_call(CallContext { plugin_id, is_main }, f)`; `current_call()` gives
   the innermost plugin executing. The runtime pushes it around every call into a plugin; the
   host pushes `is_main: false` frames around async-instance calls.
3. **Runtime**: the `Rustic` (as a `Weak`) whose entry point is running; `context::runtime()`
   upgrades it and `context::api()` returns the `PluginApi` of the plugin currently executing
   in the main instance. Main-instance host functions use `context::api()` and therefore cannot
   act as another plugin; `StoreData` needs nothing but its plugin id (still `Send`).

Double borrows of core state are impossible by construction: subsystems are separate
`RefCell`s, no borrow is held while a plugin runs, and the few places that could meet a live
borrow use `try_borrow*` and report an error (`"the event bus is busy"`) instead of panicking.
Re-entering a plugin instance is refused through the instance `RefCell` (`CallError::Busy`):
an event is never delivered into a plugin that is executing.

`ExecutionTracker` (atomics) records the innermost executing plugin and since when; a
`Watchdog` thread reads it and calls back once per stuck call. The host's epoch deadline is
what interrupts the call; the watchdog reports.

## Consequences

* Host import implementations look like
  `context::api()?.subscribe(...)` and `context::backend_call(|b| b.players_kick(p, &t))`.
* Async-instance imports never see a backend: game calls return `main-thread-only`, as the WIT
  specifies, with no extra checks in the host.
* The runtime is `!Send`; everything shared with other threads lives in `SharedHandle`.

## Alternatives considered

* **Raw pointer in a `Cell<Option<NonNull<dyn Backend>>>`** (a small audited `unsafe`
  module): equivalent semantics, but needs `#[allow(unsafe_code)]` and lifetime erasure we
  would maintain ourselves; `scoped-tls-hkt` already encapsulates exactly this pattern.
* **`Rc<Rustic>` in `StoreData`**: makes `Store<StoreData>` `!Send` and still does not solve
  the backend borrow.
* **Command buffer** (imports enqueue, the runtime applies after the call): breaks
  synchronous imports with results (`subscribe -> handler-id`, `players.name`).
