# ADR 0003: Core runtime design (rusticmc-core, -permissions, -timings)

- Status: accepted
- Date: 2026-11-10

## Context

Rustic needs an engine-agnostic runtime between the Horizon adapter and the wasmtime host:
plugin lifecycle, events, scheduler, commands, permissions, services, storage, timings. It must
be testable without WebAssembly and without the game, fast on hot paths (`player-move` fires
for every accepted move packet), and a misbehaving plugin must never take the server down.

## Decision

**Single-threaded façade, `Rc` + interior mutability.** `Rustic` is an `Rc<Inner>` living on
the server thread; each subsystem (plugin table, bus, command registry, permission service) is
its own `RefCell`. No borrow is held across a call into a plugin, so plugins can call back into
the runtime (subscribe during dispatch, fire custom events, run commands, disable plugins)
without double borrows. Borrows that could collide use `try_borrow*` and degrade (error / skip)
instead of panicking. What the async instance pool needs (scheduler, storage, services,
timings, execution tracker, tick counter) lives in an `Arc<Shared>` (`SharedHandle`,
`Send + Sync`).

**Plugin incarnations are slots.** Each load creates an `Rc<PluginSlot>` (state, instance,
owned game resources). Listeners hold the slot, so dispatch needs no lookups; the instance sits
in a `RefCell<Option<Box<dyn PluginInstance>>>` whose `try_borrow_mut` doubles as the
re-entrancy guard (a plugin that is executing is never entered again: the event is skipped with
a warning). Disabling a plugin that is executing (it disabled itself, or a nested call faulted
a dependency) marks it disabled, cleans its registrations at once and defers `on-disable` and
dropping the instance until the outermost call returns.

**Event bus.** Built-in events are indexed by `EventKind` in a dense array of
`Rc<[Listener]>` snapshots sorted by (priority, subscription sequence). `has_listeners_for` is
one array load; firing with no listener is a borrow-flag check and returns the event
(measured below). Dispatch clones the `Rc` snapshot, so (un)subscribing during dispatch takes
effect next time. Nested dispatch is limited to 8 levels. MONITOR results are ignored;
`Event::apply_mutable_from` copies only WIT `mut` fields and `cancelled`.

**Faults.** Every call into a plugin goes through one function that sets the call context,
the watchdog tracker and timings, wraps the call in `catch_unwind`, and turns `Err(PluginFault)`
or a panic into "log + disable this plugin and its dependents". The server never sees a panic
from a plugin call.

**Scheduler.** Thread-safe (`Mutex`), sync tasks by tick in task-id order with a per-tick
budget (overflow rolls over and resumes where it stopped, so no task starves), async tasks and
jobs only tracked (`due_async` hands each out once, `async_finished` reschedules), plus the
`post-to-main` inbox. Time comes from a `Clock` trait (`FakeClock` in tests).

**Commands.** Usages are merged into a typed tree; parsing is a backtracking DFS returning every
full match, ranked by literals matched then declaration order, so permissions can pick the
first usable match. Arguments are parsed with a `Resolver` (online players, locations, worlds)
implemented over the backend. Bare names go to the first registrant, `<plugin>:<name>` always
works, and bare names are re-assigned when their owner goes away. The tree is exported in a
neutral form and as Horizon usage strings (lossy mapping for kinds Horizon lacks; an `item`
argument gets an extra `<x_count:integer>` variant).

**Permissions.** Declared nodes (copy-on-write `Rc` table with a child → parents index) plus a
pluggable explicit-rule source: the provider plugin through a host-implemented
`PermissionProvider`, else the default engine (`rusticmc-permissions`), with per-lookup
fallback to the engine if the provider fails.

**Timings.** A separate crate, `Mutex`-protected maps per plugin and section kind with
count/total/max and a log-linear histogram (8 sub-buckets per power of two, about 6%); when off,
the hot path costs one relaxed atomic load (`start()` returns `None` without reading the clock).

**Built-ins.** The core owns `/rustic` (plugins, info, enable, disable, reload, timings,
version) under `rustic.admin` (default op).

## Measurements

`cargo bench -p rusticmc-core --bench bus` (Windows 11, release, criterion, 2026-11-10):

| case | time |
|---|---|
| `has_listeners_for(kind)` | 0.73 ns |
| `has_listeners("player-move")` | 4.2 ns |
| `fire(block-break)`, 0 listeners | 18.8 ns (mostly moving the ~200-byte `Event` in and out) |
| `fire(block-break)`, 1 no-op native listener | 202 ns |
| `fire(block-break)`, 10 no-op native listeners | 1.69 µs |

The per-listener cost (~170 ns) is dominated by the no-op handler cloning the event (string
allocations), `Instant::now()` for the watchdog tracker and the context push; the WASM boundary
will dominate it in production. The adapter should call `has_listeners_for` before building an
event.

## Dependencies (all maintained, licences compatible with MIT OR Apache-2.0)

| crate | version | licence | why |
|---|---|---|---|
| serde (+ derive, rc) | 1.0.229 | MIT OR Apache-2.0 | model mirrors, manifests, configs |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | text components, timings export |
| toml | 1.2.1 | MIT OR Apache-2.0 | manifests, `rustic.toml`, `permissions.toml` |
| semver | 1.0.28 | MIT OR Apache-2.0 | plugin versions and requirements |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | error types |
| log | 0.4.35 | MIT OR Apache-2.0 | logging facade (target `rustic`) |
| zip | 9.5.0 (no default features, `deflate-flate2-zlib-rs`) | MIT | `.rplugin` read/write |
| flate2 / zlib-rs (via zip) | 1.1 / 0.6 | MIT OR Apache-2.0 / Zlib | deflate, pure Rust |
| scoped-tls-hkt | 0.1.5 | MIT OR Apache-2.0 | scoped `&mut dyn Backend` slot (ADR 0004) |
| uuid (optional feature) | 1.29 | MIT OR Apache-2.0 | `From` conversions only |
| criterion (dev) | 0.8.2 | MIT OR Apache-2.0 | benchmarks |
| tempfile (dev) | 3.27 | MIT OR Apache-2.0 | tests |

Transitive runtime additions: crc32fast, typed-path, zeroize, getrandom, indexmap, hashbrown,
winnow (MIT), memchr (Unlicense OR MIT), constant_time_eq (CC0/MIT-0/Apache), all permissive.

## Consequences

* The host implements `PluginInstance` / `PluginLoader`, routes main-instance imports through
  `context::api()` / `context::backend_call` and async ones through `SharedHandle::async_api`.
* The test harness drives the same runtime with `NativePlugin`/`NativeLoader` or real
  components, and an in-memory `Backend`.
* `Rustic` is `!Send`: the Horizon adapter keeps it on the server thread (it already is).

## Alternatives considered

* **`Arc<Mutex<_>>` everywhere**: needed only for the async side; on the main thread it adds
  locking to every event and still deadlocks on re-entrancy instead of failing cleanly.
* **An ECS-style message queue for all plugin → core calls**: avoids re-entrancy but makes
  `subscribe`, `register` and `call-custom` asynchronous, which the WIT does not allow.
* **Brigadier-style per-node executors**: the WIT gives usages, not nodes; a merged tree plus
  usage indices keeps the contract simple.
