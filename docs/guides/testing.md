# Testing plugins with `rusticmc-test`

`rusticmc-test` runs your plugin's **real `.wasm`** inside an in-memory server, from ordinary
`cargo test`s. It is the Rustic counterpart of MockBukkit, written from scratch: the server is
the real Rustic runtime (`rusticmc-core`: events, commands, permissions, scheduler, storage)
over a simulated game (worlds, players, inventories, menus, scoreboards, boss bars, tab list,
plugin channels), and plugins load through the real WebAssembly host (`rusticmc-host`). If a test
passes here, the same component does the same thing on a server.

Projects made with `rustic new` already have it set up (`tests/plugin.rs`); run them with
`rustic test`.

## Setup

```toml
# Cargo.toml of the plugin
[dev-dependencies]
rusticmc-test = { path = "<rustic checkout>/crates/rusticmc-test" }   # or a registry version
```

```rust
// tests/plugin.rs
use rusticmc_test::prelude::*;

fn server() -> MockServer {
    // Builds this crate for wasm32-wasip2 (cached; cargo is incremental) and loads it.
    MockServer::with_built(&build_plugin(env!("CARGO_MANIFEST_DIR")))
}

#[test]
fn greets_joining_players() {
    let server = server();
    let steve = server.add_player("Steve");           // fires player-pre-login, player-join
    steve.assert_message_contains("Welcome");
}
```

`build_plugin(path)` runs `cargo build --target wasm32-wasip2 --release` on the crate once per
test binary and returns the component with its manifest and default `config.toml`. Relative
paths are also looked up from every parent directory, so `build_plugin("plugins/hello")` works
from any crate of the repository. `RUSTIC_TEST_PROFILE=dev` builds unoptimised components
(faster to compile, slower to run). A crate without `rustic.plugin.toml` (a test fixture) gets a
manifest made from its package name.

Other ways to load plugins:

| Call | Loads |
|---|---|
| `MockServer::with_built(&built)` | a `BuiltPlugin` |
| `MockServer::with_plugin_wasm(path)` | a plugin crate (built first), a dev directory, a `.rplugin`, or a `.wasm` next to its manifest |
| `MockServer::with_native_plugin(id, \|\| NativePlugin::new()...)` | a native closure plugin (the harness's own tests, quick experiments) |
| `MockServer::builder()...build()` | several plugins, a custom `rustic.toml`, a `permissions.toml` engine, admin `config.toml`s, a fixed server dir |
| `server.load_plugin(path)` / `load_package(pkg)` / `load_native(..)` | into a running server |

```rust
let server = MockServer::builder()
    .plugin_config("greeter", "config_version = 1\ngreeting = \"Hi {name}\"\n") // admin edit
    .built(&build_plugin(env!("CARGO_MANIFEST_DIR")))
    .built(&build_plugin("../economy"))                                          // a dependency
    .build();      // panics with the reason (and the console) if a plugin does not start
```

`build_unstarted()` instead of `build()` lets you assert a plugin that fails to load.

The default `rustic.toml` of a mock server is the server's, except a 2 s wall-clock deadline
per plugin call (`harness_config()`; the server uses 50 ms) so debug builds and busy CI
machines do not time out; fuel still stops runaway loops. Pass `.config(..)` to test with the
production limits.

## Players: actions fire events and apply the outcome

Every action fires the event the real server fires and **applies the result like the server
does**: what a handler cancels does not happen, what it changes is used.

| `MockPlayer` method | Event(s) | Outcome applied |
|---|---|---|
| `server.add_player(name)` / `add_op` / `try_add_player` | `player-pre-login`, `player-join` | refused with the kick message; join line broadcast (or hidden) |
| `chat(msg) -> bool` | `player-chat` | cancelled: nobody sees it; `display` replaces the `<name> msg` line |
| `command("/x ...") -> CommandRun` | `player-command`, then the command | cancelled: nothing runs; unknown: "Unknown or incomplete command" |
| `complete("home ")` / `server.complete_as(&p, ..)` | completion | the suggestions |
| `move_to(loc)` / `move_by(dx, dy, dz) -> bool` | `player-move` (beyond the move threshold) | cancelled: the player is put back |
| `teleport(loc)` | `player-teleport`, `player-change-world` | |
| `set_game_mode(mode) -> bool` | `player-game-mode-change` | cancelled: unchanged; flight follows the mode |
| `break_block(pos) -> bool` | `block-break` | cancelled: the block stays; `drop_items` decides the drop |
| `place_block(pos, "oak_log[axis=x]") -> bool` | `block-place` | survival needs the item in a hand and uses one |
| `right_click_block`, `left_click_block`, `right_click_air`, `left_click_air` | `player-interact` | |
| `interact_entity(&entity)` | `player-interact-entity` | |
| `damage(amount)`, `damage_by(amount, cause, attacker)`, `attack(&other, amount)` | `entity-damage` | the final amount; death at 0 health; creative/spectator are immune |
| `kill()` | `player-death` | keep-inventory / keep-level, drops, death line |
| `respawn()` | `player-respawn` | where the handler says |
| `click_menu_slot(slot, ClickKind::Left) -> bool` | `inventory-click` (cancelled by default in plugin menus) | allowed clicks move items (cursor, shift-click into the inventory, drop) |
| `close_menu()` | `inventory-close` | |
| `send_plugin_message(channel, data) -> bool` | `plugin-message` (registered channels only) | |
| `disconnect()` | `inventory-close` (open menu), `player-quit` | quit line |

Events caused by a plugin's own calls are fired too: a plugin teleporting a player or changing
their game mode fires the events for the other plugins (the calling plugin is skipped, as the
runtime never re-enters a plugin); a plugin killing (`set_health(0)`), kicking or closing a menu
is processed right after its call returns.

Test setup without events: `set_location`, `set_health`, `set_food`, `set_level`, `give(id, n)`,
`give_stack`, `set_item(slot, stack)`, `set_held_slot`, `set_op`, `set_permission(node, bool)`,
`unset_permission`, `set_locale`, `set_ping`.

## Assertions

```rust
steve.assert_message_contains("Welcome");        // any chat line received
steve.assert_no_message_contains("heck");
steve.assert_next_message("Hello, Steve!");       // reads in order, MockBukkit-style
steve.assert_no_more_messages();
steve.next_message();                             // Option<String>
steve.messages(); steve.messages_json();          // plain / JSON components
steve.last_action_bar(); steve.titles(); steve.sounds(); steve.assert_heard("entity.experience_orb.pickup");
steve.assert_kicked(); steve.kick_reason();
steve.inventory(); steve.item(0); steve.count("diamond"); steve.held_item(); steve.cursor();
steve.open_menu();                                // MenuView: rows, title, items
steve.health(); steve.food(); steve.game_mode(); steve.location(); steve.is_dead();
steve.has_permission("x.y");                      // resolved like on the server
steve.data_json::<u64>("my-plugin", "visits");    // per-player data a plugin stored
steve.boss_bars(); steve.tab_header_footer(); steve.tab_display_name(); steve.plugin_messages();

server.assert_console_contains("enabled");        // console replies, broadcasts, log lines
server.broadcasts(); server.console_commands();
server.storage_json::<T>("my-plugin", "key");     // the plugin's key-value storage
server.score("kills", "Steve"); server.objective("kills"); server.team("red");
server.displayed_objective("sidebar"); server.boss_bars(); server.incoming_channels();
server.plugin_state("my-plugin"); server.plugin("my-plugin"); server.assert_enabled("my-plugin");
server.has_command("spawn");
```

Worlds: `server.overworld()`, `nether()`, `the_end()`, `world(id)` give a `MockWorld`:
`block(pos)`, `block_id(pos)`, `set_block(pos, "stone")`, `highest_block_y`, `time`/`set_time`,
`weather`/`set_weather`, `spawn`/`set_spawn`, `location(x, y, z)`, `spawn_entity(kind, at)`,
`entities()`, `entities_of("zombie")`, `dropped_items()`, `damage_entity`, `kill_entity`
(fires `entity-death`), `particles()`, `sounds()`, `load_chunk`/`unload_chunk` (fire chunk
events), `is_chunk_forced`. The terrain is flat: bedrock at the bottom, stone, grass at y = 64 in
the overworld; spawn at (0.5, 65, 0.5). Positions can be `(x, y, z)` tuples.

The server side: `console_command("say hi") -> CommandRun`, `dispatch_as(&p, line)`,
`ping(address)` (fires `server-list-ping`), `fire(event)` (any event record),
`set_max_players`, `set_motd`, `set_keep_inventory`, `set_behind_proxy`, `disable`/`enable`/
`reload(id)`, `shutdown()`, `with_state(|state| ..)` for anything else.

`CommandRun` tells what happened to a command line: `Plugin(CommandOutcome::Executed |
NoPermission | Usage(..) | PluginError(..) | ...)`, `Vanilla(bool)`, `Cancelled`, `Unknown`;
`.executed()` is true when it ran. The mock knows a few vanilla commands (`say`, `msg`, `give`,
`clear`, `gamemode`, `tp`, `time`, `weather`, `kill`, `kick`, `op`, `deop`, `list`,
`setworldspawn`), for plugins that `Server::dispatch_command` them.

## Time and async work

The clock is fake. `server.tick(n)` runs `n` ticks of 50 ms: `post-to-main` messages, due
scheduler tasks, world time; then (default `AsyncMode::Inline`) it waits until the async
instance has run every async task that became due, so a `run_async` job's `post_to_main` result
is handled at the next tick, deterministically. `run_for(Duration)`, `advance_time(d)` (move the
clock without ticking, for millisecond async delays) and `run_async()` complete the set.
`AsyncMode::Manual` runs native plugins' async tasks only on `run_async()`.

## Logs

Plugin logs and runtime warnings are captured per server (`server.log_lines()`, included in
`console()`). Warnings and errors are also printed to stderr, which `cargo test` shows for
failing tests; `RUSTIC_TEST_LOG=1` (or `debug`) echoes everything.

## Faults

A plugin that traps, panics, loops or runs out of memory is disabled exactly as on the server:
`server.plugin_state(id) == Some(PluginState::Failed)` and `server.plugin(id).unwrap().failure`
says why. The other plugins keep running (see `crates/rusticmc-test/tests/fixtures.rs`).

## What is simulated and what is not

Simulated faithfully: every `Backend` call (players, inventories with stacking rules, menus,
worlds with block storage, entities, scoreboards, teams, boss bars, tab list, plugin channels,
tick stats), permissions (`permissions.toml` engine, declared defaults, op), storage and
per-player data on disk in a temporary server directory, the scheduler.
Not simulated: physics, mob AI, redstone, real terrain generation, item use effects, the
network. Block `physics = true` writes are counted (`physics_updates()`), not simulated.

## Examples in the repository

* `crates/rusticmc-test/tests/hello.rs`: the suite of `plugins/hello` (join, chat filter,
  commands, completion, permissions, action bar timer, config, reload, `.rplugin` loading).
* `crates/rusticmc-test/tests/fixtures.rs`: fault isolation with `plugins/fixtures/buggy`.
* `crates/rusticmc-test/tests/harness.rs`: every simulated rule, with native plugins.
* The `tests/plugin.rs` of each `rustic new` template.
