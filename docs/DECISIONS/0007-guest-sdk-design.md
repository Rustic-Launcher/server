# ADR 0007: Guest SDK design (`rusticmc-api`, `rusticmc-api-macros`)

- Status: accepted
- Date: 2026-11-10

## Context

Plugin authors write Rust compiled to `wasm32-wasip2` components implementing
`rustic:api/rustic-plugin@0.1.0`. Raw wit-bindgen output is usable but low-level: every plugin
would implement two `Guest` traits with 32 functions, track handler/command/task ids itself, and
build JSON text and SNBT by hand. The host runs each plugin as two instances (main and async)
sharing no memory, and the WIT `run-task` export only carries a task id.

## Decision

1. **The SDK owns the bindings and the exports.** `rusticmc-api` runs
   `wit_bindgen::generate!` once (path `../rusticmc-api-wit/wit`, world `rustic-plugin`,
   `pub_export_macro: true`, `default_bindings_module: "rusticmc_api::bindings"`,
   `additional_derives: [PartialEq]`). Plugin crates never run bindgen. `export_plugin!(T)` (what
   `#[rustic::plugin]` expands to) defines a private guest type in the plugin crate that forwards
   every export to generic runtime functions (`__rt::on_enable::<T>`, `__rt::dispatch::<E>`, ...)
   and invokes the generated `export!` with `with_types_in $crate::bindings` (works when the SDK
   is renamed, e.g. `rustic = { package = "rusticmc-api" }`). All 25 `handle-*` exports are
   implemented by a declarative macro inside the SDK, so a new event is one line there; unused
   events cost a table lookup returning the record unchanged. The plugin implements only what it
   subscribes to.
2. **Dispatch tables keyed by host ids**, in thread-locals: handler id → erased closure
   (`Rc<RefCell<dyn FnMut(&mut dyn Any)>>`, downcast to the typed event), command id → executors
   per usage, task id → closure, (menu id, slot) → click handler, job name → async closure,
   topic → message handler. Closures are cloned out before being called, so they can register
   more. The plugin value lives in a `RefCell<Option<Box<dyn Any>>>`, built with `Default`;
   `with_plugin::<P>` reaches it from closures.
3. **Macros.** `#[plugin]` (struct) → `export_plugin!`. `#[handlers]` (inherent impl) collects
   `#[event_handler(priority, ignore_cancelled, custom)]`, `#[command(name, aliases, permission,
   usage, description, players_only, completer)]` and `#[completer("cmd")]` methods into
   `impl HandlerSet`, registered after `enable`. Whether the plugin type has a `HandlerSet` impl is
   resolved with autoref specialisation on a concrete type in the generated glue, so plugins
   without a `#[handlers]` block need nothing. The builder APIs (`ctx.on`, `Command::new(..)`) are
   always available; macros are sugar over them.
4. **Typed events** wrap each record: getters for all fields, setters only for fields the host
   applies (`mut` in WIT) and `Cancellable` for cancellable events, so writing a read-only field is
   a compile error.
5. **Async instance.** `load`/`enable` run in both instances; SDK registration calls are no-ops
   outside the main instance (`scheduler.is-main`, cached). Async jobs are registered by name in
   `enable` and started by name with a JSON payload through `scheduler.run-async-job(job,
   payload, delay-ms)`; the host calls the `plugin.run-job(task, job, payload)` export in the async
   instance and the SDK runs the closure registered under `job`. Async timers are main-instance
   tick timers that start the job each period (the WIT has no job timer; cancelling the main
   timer stops it). Results return with `post-to-main` → `on-message` →
   `ctx.on_message(topic, ..)`. (The first version passed job names through reserved storage
   keys because `run-task` only carried an id; the WIT gained `run-async-job`/`run-job` instead.)
6. **Pure logic is host-testable.** Text, usage parsing, items, config migration, event wrappers
   and the dispatch tables have unit tests on the host target. wit-bindgen's native import stubs
   are `unreachable!()`; every wrapper calls `host::guard(name)` first, which panics off-wasm with
   a message naming the import.
7. **Text** emits current JSON field names (`click_event`/`hover_event`, `command`/`url`/`value`;
   Horizon also accepts the camel-case ones). The `&`-legacy and MiniMessage-like parsers are
   written from scratch for Rustic (no Adventure/MiniMessage code).
8. **Unsafe**: the only unsafe code is wit-bindgen's generated glue, confined to the `bindings`
   module (`#[allow(unsafe_code)]`) and to the export functions the macros expand in plugin crates.
   The SDK's own code has none.
9. **Services.** The base world `rustic-plugin` imports
   `rustic:services/{economy,permission-provider,chat-meta}@0.1.0` (the host always defines them,
   answering `no-provider` when nobody provides). Consumers call `Economy::balance(&player)`,
   `PermissionProvider::groups(..)`, `ChatMeta::prefix(..)` (`ServiceResult<T>`, converting into
   `Error`/`CommandError`). Providers target a provider world selected by SDK cargo features:
   `provide-permissions` → `rustic-plugin-permissions` (exports permission-provider + chat-meta),
   `provide-economy` → `rustic-plugin-economy`, both → `rustic-plugin-permissions-economy`; the
   SDK `cfg`-selects one `generate!` invocation. The plugin implements plain traits
   (`ProvidePermissions`, `ProvideChatMeta`, `ProvideEconomy`, with a `PRIORITY` const and
   defaults where sensible); `export_plugin!` expands cfg-selected glue macros defined in the SDK
   (so the feature is evaluated in the SDK, not the plugin crate) that implement the exported
   `Guest` traits by forwarding to the trait methods on the plugin instance, and declares the
   services with `services.provide` in `on-load`. A normal plugin's component exports nothing
   but `plugin` and `handlers`. The exported permission interface has its own copy of `tristate`;
   the SDK converts it to the consumer-side `Tristate` the trait uses.

## Dependencies

| crate | why | licence |
|---|---|---|
| `wit-bindgen` 0.63 (features `macros`, `realloc`, `std`) | bindings and export glue | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| `serde`, `serde_json` | typed storage/data/payloads, JSON text parsing in `Text::plain` | MIT OR Apache-2.0 |
| `toml` 1 (`preserve_order`, optional feature `config`, default on) | `config.toml` | MIT OR Apache-2.0 |
| `syn` 3, `quote`, `proc-macro2` (macros crate) | attribute macros | MIT OR Apache-2.0 |
| `proc-macro-crate` 3 (macros crate) | resolve the SDK's name when renamed in Cargo.toml | MIT OR Apache-2.0 |

All are maintained (2026 releases) and licence-compatible with the core rules.

## Consequences

- A minimal plugin (`plugins/hello`) is ~70 lines; release build ≈ 461 KiB (471 771 bytes) with
  `opt-level = "s"`, LTO, strip; most of it is std formatting, serde_json and the TOML parser.
  Plugins that do not read TOML can disable the `config` feature.
- The SDK must be updated in lockstep with the WIT (one macro line per new event, new wrappers,
  one world + glue macro per new standard service).
- Provider worlds multiply with services (2^n combinations); with more standard services we
  would switch to one feature per interface and generate the world list.
- Async job timers are paced by the tick (period in ticks), not wall-clock milliseconds.

## Alternatives

- **Each plugin runs `generate!`**: duplicate 20k lines of bindings per plugin, every plugin
  implements both `Guest` traits; rejected.
- **The SDK exports a fixed guest type** (`export!(rusticmc_api::Guest)`): needs a way to register
  the plugin factory before the first export call (no life-before-main in WASM); rejected for a
  per-plugin generated guest type.
- **`inventory`/`linkme` for collecting `#[event_handler]` functions**: relies on linker sections
  or constructors that are not dependable on `wasm32-wasip2`; an impl-level attribute is explicit.
- **Async closures keyed by task id**: impossible across instances (no shared memory); deterministic
  id prediction would depend on host internals.
- **Storage hand-off for async jobs** (first version): worked with the original WIT but raced the
  write and leaked SDK keys into plugin storage; replaced by `run-async-job` / `run-job`.
- **Services exported by every plugin** with "no-provider" defaults: simpler (one world) but every
  component would export services and the host would have to tell real providers apart; rejected
  for feature-selected provider worlds.
- **A `#[rustic::provide(Economy)]` attribute**: a trait impl already says the same thing and is
  checked by the compiler; the attribute would add nothing.
