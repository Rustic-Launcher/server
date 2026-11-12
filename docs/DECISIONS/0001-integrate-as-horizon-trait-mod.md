# ADR 0001: Rustic integrates with Horizon as one trait mod in its own engine executable

- Status: accepted (Phase 0 map confirms it, see docs/ARCHITECTURE.md §8)
- Date: 2026-11-10
- Horizon: `a84f637dd` (`NATIVE_HOOKS_API = 415`)

## Context

Rustic needs to observe and change the game the way Bukkit/Paper plugins do: logins, chat, commands,
movement, block break/place, interactions, damage, deaths, inventories, plus a toolbox of server actions
(messages, teleports, inventories, blocks, scoreboards, boss bars...). Horizon offers four ways in:

1. **Trait mods**: a crate implements `horizon_client::mod_api::Hooks` (generated at build time from
   `native-hooks.json`, the pending fragments in `tools/native-hook-fragments/` and the game's signatures) and an
   executable passes it to `horizon_client::entry::main_with(Arc::from([...]))`. At `a84f637dd` the trait has 532
   methods (453 not client-only, 131 cancellable) and `Host` 441 helpers (docs/horizon/hooks.md, host.md).
2. **Mixin bundles** (horizon-mixins): source patches applied before compiling the game.
3. **WASM mods**: not on Horizon's main; they live on the unofficial fork arismoko/horizon, which since
   2026-11-09 also receives all of upstream's mod-API PRs and is in a declared API "clean break".
4. **Outside the process**: the supervisor's control API / RCON / management protocol, or a proxy.

The dedicated server is the game executable run with `--dedicated`; `horizon-server` (supervisor) runs whatever
`engine.path` names, so an engine with mods compiled in is a supported setup (docs/native-mods.md, "Standalone
server export").

## Decision

Build a **`rustic-server` executable** (`crates/rusticmc-server`, own workspace, Horizon as a path dependency,
shared `horizon/target`) whose `main`:

- calls `horizon_client::mod_api::name_mods(&["rustic"])`, then
  `horizon_client::entry::main_with(Arc::from([Box::new(RusticHooks) as Box<dyn Hooks>]))`;
- keeps what Horizon's own `main.rs` keeps (the `horizon_mods::api_level::MARK` static, the global allocator) and,
  on Windows, the 8 MB main-thread stack link argument (`packaging/windows/exe.rs`).

`RusticHooks` is the **only** trait mod. It implements the hooks Rustic maps (docs/ARCHITECTURE.md §5), wraps every
body in `catch_unwind`, converts Horizon types to `rusticmc-core`'s model, and hands events to Rustic's own wasmtime
runtime (ADR 0002), which lives on the server thread. Plugins are loaded, reloaded and unloaded inside Rustic;
`Host::refresh_commands()` republishes their commands. Rustic also answers `permission_check` for every mod.

Missing hook points (server-list ping, join/quit broadcast lines, death message/keep-inventory, vanilla container
close, a few `LevelRef` writes/reads, inventory getter) are proposed upstream (docs/UPSTREAM_DRAFT.md) and, until
they land, carried as minimal patches in `patches/` with the WIT fields documented as ignored.

The supervisor (`horizon-server`) is used unchanged: `engine.path = <rustic-server>`.

## Consequences

- One process, no IPC; plugin calls happen inline on the server thread with the game state at hand.
- Rustic is tied to the `mod_api` of a pinned Horizon commit; every submodule bump regenerates
  `docs/horizon/hooks.json` and the diff is reviewed. Only `crates/rusticmc-server` touches `mod_api`.
- Some hooks have no `host` (chat, move, teleport, damage v3...): Rustic serves plugin imports there from a
  per-tick snapshot and queues writes.
- A panic that escapes `RusticHooks` crash-stops the server (Horizon catches it in the tick loop and stops cleanly):
  the `catch_unwind` wrapper is mandatory.
- Building the engine costs ~12 minutes cold on Windows (docs/horizon/build.md); Rustic ships per Horizon commit.
- Trait mods cannot be added at run time (`MODS` is a `OnceLock`); irrelevant since Rustic manages its plugins.

## Alternatives

- **Mixin bundle**: a full game rebuild for every Rustic change, patches against private functions that move every
  week, nothing gained over the trait (whose hooks are the stable names). Rejected; mixins only for unavoidable patches.
- **Build on the fork's WASM mods**: unofficial, not on Horizon's main, mid API break, and its WIT is not Rustic's
  Bukkit-shaped contract. Rejected for now; it can coexist later as another trait mod.
- **Separate process** (plugin host talking to the engine through the control API, RCON or a proxy): sees commands
  and packets, not gameplay (no cancellable block break, damage, loot), adds latency. Rejected.
- **Fork Horizon**: maximal control, maximal drift. Rejected; minimal patches only.
