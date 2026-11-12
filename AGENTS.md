# Rustic: a map for contributors and agents

Rustic is a server distribution for **Horizon** (MIT, a Rust rewrite of the 26.3 Java game server,
`horizon/` git submodule, never edited in place) plus a sandboxed **WASM plugin API**, the role
Bukkit/Spigot/Paper play for the Java server. Read this page fully before touching code.

## Non-negotiable rules

- **No Mojang code or assets** anywhere (jars, decompiled source, prepared resources: `.local/`).
- **No Bukkit/Spigot/Paper/CraftBukkit/Glowstone code copied**, not even signatures pasted from
  their sources. Concepts only, rewritten. Every inspiration is listed in `docs/THIRD_PARTY.md`.
  Exception: `bridge/java` (GPLv3, separate, optional) *compiles against* the published Bukkit API
  jar fetched at build time; it still copies no implementation code.
- The product name "Minecraft" appears in no crate, binary or product name. Crates: `rusticmc-*`.
- **A plugin can never crash the server**: traps, panics, infinite loops (fuel/epoch), OOM
  (store limits) disable that plugin with a log line; the server goes on.
- **Never invent a Horizon API.** Read `horizon/` (see "Horizon facts" below). If something is
  missing upstream, add a minimal patch in `patches/` and say so in `docs/ARCHITECTURE.md`.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, tests next to the code, criterion
  benches where performance matters. One significant decision = one ADR in `docs/DECISIONS/`
  (`NNNN-title.md`: Context / Decision / Consequences / Alternatives). Before adding a crate,
  check it is maintained and license-compatible (MIT/Apache/BSD/Zlib/MPL-2.0 ok in core; GPL
  only in `bridge/java`) and record it in the ADR or `docs/THIRD_PARTY.md`.
- Licence of Rustic itself: `MIT OR Apache-2.0` (ADR 0006). Bridge Java layer: GPL-3.0-only.
- Unsafe code is denied workspace-wide. If unavoidable, isolate it in one module with a
  `// SAFETY:` comment per block and `#[allow(unsafe_code)]` on that module only, plus an ADR.

## Agents working in parallel

- **Own your directories.** Each agent is told which paths it owns; do not edit others' paths.
  Need a change elsewhere? Append a request to `docs/HANDOFF.md` (who, what, why) and work around
  it meanwhile.
- **Do not `git commit`**; the lead commits in conventional commits at the end of each wave.
- **Disk is tight (~25 GB free).** Use the shared target dir: every cargo command for the root
  workspace uses the default `./target`. Never `cargo clean` the whole workspace. Everything that
  compiles Horizon (exploration builds, `crates/rusticmc-server`) uses
  `CARGO_TARGET_DIR=<repo>/horizon/target` so Horizon is built once. Plugins (wasm) use
  `plugins/target`. Check `df -h .` before a big build; stop and report at < 6 GB free.
- Cargo holds a lock on the target dir: if you see "Blocking waiting for file lock", wait.
- Windows host (Git Bash + PowerShell available). Linux x64 is the primary CI target: keep code
  portable (no Windows-only paths).

## Repository layout and owners

| Path | What |
|---|---|
| `crates/rusticmc-api-wit/wit/` | **The API contract**: `rustic:api@0.1.0` (`types.wit`, `events.wit`, `core.wit`, `game.wit`, `plugin.wit`) and `deps/services` (`rustic:services@0.1.0`). Changes follow `docs/API.md` policy. |
| `crates/rusticmc-api-wit` | Rust crate exposing the WIT path/text (`pub const WIT_DIR`) and a test that parses it with `wit-parser`. |
| `crates/rusticmc-core` | Engine-agnostic runtime: manifest + `.rplugin` format, discovery, dependency resolution, lifecycle, event bus, scheduler, command tree, permission checks (delegating to `rusticmc-permissions` or a service), service registry, KV storage, config, the `Backend` and `PluginInstance` traits. No wasmtime, no Horizon. |
| `crates/rusticmc-permissions` | Default permission engine (TOML file: groups, inheritance, wildcards, defaults). |
| `crates/rusticmc-timings` | Per-plugin / per-event / per-task timing collection, `/rustic timings` report, JSON export. |
| `crates/rusticmc-host` | Wasmtime component runtime: implements `PluginInstance` for `.wasm` components, binds every `rustic:api` import to core + `Backend`, fuel/epoch/memory limits, trap isolation, async instance pool, service linking. |
| `crates/rusticmc-api` (+ `rusticmc-api-macros`) | Guest SDK for plugin authors (wasm32-wasip2): generated bindings, idiomatic wrappers, `#[plugin]`, `#[event_handler]`, `#[command]`, text builder, config helpers. |
| `crates/rusticmc-test` | MockBukkit-like harness: an in-memory `Backend` (worlds, players, inventories) + helpers to load a built plugin and drive it in `cargo test`. |
| `crates/rusticmc-cli` | `rustic new/build/check/test/run/bridge analyze`. |
| `crates/rusticmc-server` | **Separate workspace.** The `rustic-server` binary: Horizon's game linked with Rustic as a trait mod (`horizon_client::mod_api::Hooks`), a `Backend` over Horizon's `Host`. |
| `patches/`, `scripts/` | Minimal patches on Horizon and the scripts that apply/rebuild them (`scripts/patches.py apply|rebuild|check`). |
| `plugins/` | Official plugins (own workspace, wasm32-wasip2): essentials-lite, perms, chat-format, menus, mini-game, plus test fixtures (hello, buggy). |
| `bridge/` | Rustic Bridge: `rusticmc-bridge-analyze` (Rust, MIT/Apache) and `java/` (GPLv3 Bukkit API layer + sidecar). |
| `docs/` | ARCHITECTURE, API, PLUGIN_FORMAT, PERFORMANCE, THIRD_PARTY, BUKKIT_MAPPING, guides, DECISIONS/. |

## Horizon facts (verified in `horizon/` at a84f637d, 2026-11-09)

- The dedicated server is the **game executable run with `--dedicated`**
  (`horizon-client`, gameplay in `crates/horizon-client/src/game/`, `dedicated.rs`); the
  `horizon-server` crate is a **supervisor** (config, console, RCON, panel, backups) that spawns it
  (`engine.path` in `server.toml` points at any engine executable).
- **Trait mods**: `horizon_client::mod_api::Hooks` (generated at build time from
  `native-hooks.json` into `OUT_DIR/mod_api_generated.rs`) has one method per hook, empty default;
  most get `host: &mut dyn mod_api::Host` and a typed `&mut XxxEvent<'_>`. An executable calls
  `horizon_client::entry::main_with(Arc::from([Box::new(MyHooks) as Box<dyn Hooks>]))`
  after `mod_api::name_mods(&["rustic"])` and with the `horizon_mods::api_level::MARK` static
  (see `crates/cargo-horizon-mod/template/native/lib.rs`, `crates/horizon-client/examples/wind_burst_hook_probe.rs`).
  The generated trait has **532 methods** (434 catalog + 98 pending fragments merged on main under
  `tools/native-hook-fragments/`; 338 server, 15 both) and `Host` **441 methods**. Exact
  signatures: `docs/horizon/hooks.md`, `docs/horizon/host.md`, `docs/horizon/hooks.json`
  (regenerate with `scripts/horizon-extract-mod-api.py`). Many hooks are cancellable (return
  `Flow`). **44 server hooks get no `host`** (`player_chat`, `player_move`, `player_teleport`,
  `living_incoming_damage_v3`...): Rustic serves those from a per-tick snapshot and queues writes.
  A panic in a hook on a dedicated server is caught by the tick loop and **crash-stops the server**
  (`game/dedicated.rs:1378`): Rustic must `catch_unwind` everything itself. Host calls can fire
  hooks again (re-entrancy). The dedicated server runs on the process main thread; on Windows the
  executable needs an 8 MB stack (`/STACK:8388608`, `horizon/packaging/windows/exe.rs`).
- Mod-API work moved to the fork arismoko/horizon on 2026-11-09 (announced "clean break": every
  hook gets a host, ID-based helpers removed): the main drift risk, see docs/ARCHITECTURE.md.
- Missing upstream (Rustic patches U1-U10, docs/ARCHITECTURE.md §9): server-list ping, join/quit
  lines, death message/keep-inventory/keep-level, closing a vanilla container, writes and spawn on
  non-ticking levels, `hand` on place/use, richer command argument kinds, typed scoreboard helpers
  (vanilla commands work meanwhile), the player inventory getter on `Host`.
- Build/run recipes: `docs/horizon/build.md`, `docs/horizon/run.md` (engine: `--profile fast
  --no-default-features`, 12 min, 103 MB exe; bots need `allow-flight=true`; JDK 25 in `tools/jdk25`).
- Server commands from mods: `register_commands` (usages like `"pay <player:player> <amount:integer>"`,
  kinds word/text/integer/number/boolean/player/item/block_pos), `perform_command`,
  `host.refresh_commands()` (explicitly meant for "a plugin host that reloads a plugin").
- Horizon's own WASM mod runtime is gone from main (moved to the unofficial fork
  arismoko/horizon); code mods are native and unsandboxed. Rustic therefore runs **its own**
  wasmtime runtime (ADR 0002).
- Velocity modern forwarding is already supported (`docs/velocity-forwarding.md` in Horizon).

## Core contracts (rusticmc-core), summarised

- `model`: Rust mirrors of every WIT record/enum/variant, same names in UpperCamelCase, plus
  `Event` (one variant per `event-types` record) with `name()`, `is_cancellable()`,
  `cancelled()`, `apply_mutable_from(&mut self, returned: &Event)`.
- `Backend` trait: one method per game-facing import (`server`, `players`, `inventories`,
  `menus`, `worlds`, `scoreboards`, `bossbars`, `tablist`, `messaging`), `&mut self`, returning
  `Result<T, HostError>`. Default bodies return `HostError::Unsupported(name)` so adapters grow
  incrementally. Implemented by the Horizon adapter and the test harness.
- `PluginInstance` trait: `on_load/on_enable/on_disable/run_task/execute_command/complete_command/
  on_message/handle_event` returning `Result<_, PluginFault>`; a fault disables the plugin.
- `Rustic` (the runtime, single-threaded, lives on the server thread): `load_dir`, `enable_all`,
  `disable`, `reload(id)`, `fire(event, backend) -> Event`, `tick(backend)`,
  `dispatch_command(sender, line, backend)`, `complete(...)`, `timings()`.
- Plugin import calls reach core state and the current `Backend` through a per-call context
  (`rusticmc_core::context`), set by the runtime around each call into a plugin.
