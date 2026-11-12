# rustic-server

Horizon's dedicated engine (the Rust 26.3 game server, `horizon/` with Rustic's patches) with the
Rustic plugin platform linked in as its one trait mod (ADR 0001, docs/ARCHITECTURE.md).

```
rustic-server  =  horizon_client::entry::main_with([RusticHooks])
                    RusticHooks (hooks.rs) -- every Horizon hook -> rusticmc_core::Rustic (fire, tick, commands)
                    HorizonBackend (backend.rs) -- rusticmc_core::Backend over Horizon's Host
                    WasmHost (rusticmc-host) -- the plugins' WebAssembly components
```

## Build

Its own Cargo workspace (it must use Horizon's `[patch]` and Cargo.lock), built in Horizon's target
dir so Horizon is compiled once:

```sh
python scripts/patches.py apply          # horizon/ on rustic-patched (the patches the adapter uses)
python scripts/sync_horizon.py --check   # the [patch] block and Cargo.lock are in step
CARGO_TARGET_DIR=$PWD/horizon/target cargo build --manifest-path crates/rusticmc-server/Cargo.toml --profile fast
# -> horizon/target/fast/rustic-server(.exe)
```

Cargo feature `bridge` (on by default) links Rustic Bridge: Bukkit/Paper jars dropped in
`plugins/bukkit/` run in a supervised JVM sidecar (docs/BRIDGE.md, docs/guides/bukkit-bridge.md).
Without jars no JVM is started; `--no-default-features` builds the server without it.

`--profile fast` is Horizon's own (`release` without LTO, 16 codegen units, incremental), the profile
the engine is released with by the launcher (docs/horizon/build.md). Gates:

```sh
cargo fmt --manifest-path crates/rusticmc-server/Cargo.toml --check
CARGO_TARGET_DIR=$PWD/horizon/target cargo clippy --manifest-path crates/rusticmc-server/Cargo.toml --all-targets -- -D warnings
CARGO_TARGET_DIR=$PWD/horizon/target cargo test --manifest-path crates/rusticmc-server/Cargo.toml
```

## Run

`rustic-server` is always a dedicated server.

- **Directly**: `rustic-server [--resources <prepared>]` in the server folder. Without `--dedicated`
  it starts itself again with `--dedicated --nogui` appended (the game reads its arguments from the
  process). Without `--resources` it uses `RUSTIC_RESOURCES`, else Horizon's own defaults
  (`resources/` beside the executable, `.local/horizon-client/26.3`), else the checkout's
  `horizon/.local/horizon-client/26.3` (docs/horizon/run.md prepares it there).
- **Under Horizon's supervisor** (`horizon-server`): the supervisor runs `engine.path` as
  `<engine> --resources R --control-socket <dir>/.horizon/engine.json [--transfers T] --dedicated --nogui`
  in the server folder (`horizon/crates/horizon-server/src/engine.rs`); `rustic-server` passes that
  command line through untouched, so RCON, the panel, the management protocol and backups work as with
  Horizon's own engine.
- **`rustic run`** (crates/rusticmc-cli) starts it in `<plugin>/target/rustic/dev-server` with the
  plugin as a dev folder `plugins/<id>-dev/` and sends `rustic reload <id>` on stdin when the plugin is
  rebuilt (`--watch`). The server needs `eula.txt` (`eula=true`) and, for bots and offline clients,
  `server.properties` with `online-mode=false` in that folder.

Files next to `server.properties`:

| Path | What |
|---|---|
| `rustic.toml` | Rustic's config (limits, scheduler, events, watchdog, storage, timings), created with defaults |
| `permissions.toml` | the default permission engine's groups and users |
| `plugins/` | `*.rplugin` packages and dev folders (`<id>-dev/` with `rustic.plugin.toml` + the `.wasm`) |
| `plugins/<id>/` | each plugin's data folder (config, storage) |
| `cache/rustic/` | compiled WebAssembly modules |

Console (stdin, RCON, the panel): `rustic` (help), `rustic plugins`, `rustic info <id>`,
`rustic enable|disable|reload <id>`, `rustic timings [on|off|reset|report|export]`, `rustic version`, and
every plugin command.

Environment: `RUSTIC_DEMO_PLUGINS=1` loads three built-in native test plugins (`rustic-demo`,
`rustic-buggy`, `rustic-ui`, src/demo.rs); `RUSTIC_TEST_PANIC=<hook>` makes that adapter hook panic, to check that
a panic never reaches Horizon.

## How the adapter works

- **Threads.** The runtime (`Rc`, `!Send`) lives in a thread local of Horizon's server thread, where
  every hook with a host runs. It starts at the first `server_tick` or `register_commands`.
  `status_response` runs on a connection thread: the ping is handed to the server thread, fired at the
  end of the tick and answered within 250 ms, only when a plugin listens (src/ping.rs).
- **Panics.** Every hook body runs in `catch_unwind` (`hooks::guard`); a panic is logged and the hook
  answers as if Rustic were absent. Plugin traps and panics are contained by the host and the core
  (that plugin is disabled).
- **No-host hooks** (`player_chat_v2`, ...): reads come from the snapshot taken at each tick
  (`server_players`, motd, limit), writes are queued and run in order at the end of the tick.
- **Re-entrancy.** A plugin's call can make Horizon fire a hook again; a nested entry gets its own
  write queue (merged into the outer one) and the same snapshot; the core refuses to re-enter a
  plugin that is executing.
- **Cost.** An event nobody listens to costs `has_listeners_for` (under 1 ns) and is never built.
- **Commands.** The core's usages become `NativeCommand`s (with their permission nodes, answered by
  Rustic's permission engine through `permission_check`); `refresh_commands()` after any change;
  `perform_command` / `perform_console_command` dispatch the raw line; tab completion through
  `tab_complete_v2`.
- **Scheduler and timings.** `server_tick` `ServerTickEnd` runs `Rustic::tick` (tasks, async
  results) and records the tick's duration for `/rustic timings`.

## Backend coverage (`rusticmc_core::Backend` over Horizon's `Host`)

Every `Backend` method is implemented (src/backend.rs; none is left `Unsupported`). "Live" means it
needs the game: inside a no-host hook (`player-chat`, `player-move`, `entity-damage`) it answers
`unsupported` (run it from a task); "queued" writes made there run at the end of the tick.

| Interface | Methods | Horizon (`Host`) | Notes |
|---|---|---|---|
| server | broadcast, dispatch-command, get-tick-stats, max-players, motd, version, offline-player, operators | `broadcast`, `run_mod_command`/`run_command`, `tick_stats`+`tps`, `max_players`, `motd`, `profile_by_name`, `ops.json` in `server_dir` | operators read `ops.json` |
| players | online, name, by-name, is-online, get-location, address, ping, locale | `server_players` (snapshot without a host), `player_client` | |
| players | send-message, action-bar, title, play-sound, teleport, game mode get/set, kick, set-flying | `tell`, `action_bar`, `title`, `play_sound_to`, `teleport_player`, `set_game_mode`, `kick`, `set_flight` | queued without a host |
| players | health/set, max-health, food/set, saturation/set, experience-level/set, is-op/set-op | `server_players`, `set_player_status`, `attribute(max_health)`, `player_food`, `/experience set`, `/op` `/deop` | |
| inventories | contents, get-slot, held-slot, set-slot, give, take, clear, close-container | `inventory` (U10), `set_inventory_slot` (compare-and-set), `give_stack`, `set_inventory`, `close_container` (U5) | components as SNBT both ways (`item_from_snbt`, `encode_components`) |
| menus | open, set-item, set-title, close, open-menu; events `inventory-click` (cancelled by default for a plugin's menu), `inventory-close` | `open_menu`, `set_menu_items`, `close_menu`; hooks `menu_clicked`, `menu_closed`, `inventory_click`, `player_container_close` | set-title re-opens the window (no rename packet); a close Rustic makes runs at the end of the tick, so the plugin hears it |
| worlds | all, get-block, set-block, set-blocks, fill, highest-block-y, is-chunk-loaded, keep-chunk-loaded, time/set, weather get/set, spawn/set-spawn, spawn-entity, remove-entity, entities-near, entity-location, spawn-particle, play-sound, players | `level`, `level_block_state`, `/setblock` and `/fill` through `execute in`, `chunk_is_loaded`, `force_chunk_in`, `level_day_time`/`level_set_day_time`, `level_is_raining`/`thundering`/`level_set_weather`, `world_spawn`/`set_world_spawn` (U6), `summon`, `remove_entity`, `entities_in`+`entity_*`, `/particle`, `level_play_sound` | highest-block-y scans the loaded column; `physics = false` is not honoured by `/setblock` |
| scoreboards | objectives, display slot, display name, scores (set/reset/get), teams (create/remove/prefix/suffix/color/join/leave) | vanilla's `/scoreboard` and `/team` (`run_command`, `run_command_captured` for get) | Horizon's per-player `sidebar` (U9) is on `Host` for a later per-player API |
| bossbars | create, set-title, set-progress, set-color, show, hide, remove | `boss_bar(uuid, id, bar)` per viewer | Rustic keeps the bar and its viewers, resends on change |
| tablist | set-header-footer, set-display-name | `tab_list`, `set_list_name` | |
| messaging | send, register/unregister-incoming, behind-proxy; event `plugin-message` | `send_custom_payload_to_player`, hook `custom_payload_from_client`, `forwarding.secret` | the core routes incoming payloads by registered channel |

Events wired (src/hooks.rs): player-pre-login (via join), player-join, player-quit, player-chat (+display), player-command,
server-command, player-move (threshold `[events] move_threshold`), player-teleport, player-change-world, block-break
(`drop-items = false` clears that break's loot and experience), block-place (exact state, U12; hand, U7), player-interact
(left/right click on a block), player-interact-entity, entity-damage (every hit; mob UUIDs from an id table kept while a
plugin listens), player-death (U3), entity-death (with its drops, mutable), player-respawn (mutable location),
inventory-click, inventory-close, player-game-mode-change, server-list-ping (U1), plugin-message, chunk-load,
chunk-unload. Not wired: `player-interact` for clicks in the air (`use_item` does not say which hand nor whether a block
was aimed at), `entity-death.dropped-exp` (reported 0).

## Recipes used for the evidence below

```sh
# A scratch dedicated server with plugins (hello, the buggy fixture, the built-in test plugins):
#   .local/rustic-server-test/server/plugins/{hello-dev,buggy-dev}/ (rustic.plugin.toml + .wasm)
: > .local/rustic-server-test/console.in
tail -f .local/rustic-server-test/console.in | RUSTIC_DEMO_PLUGINS=1 scripts/rustic-server-test.sh > .local/rustic-run.log 2>&1 &
echo "rustic plugins" >> .local/rustic-server-test/console.in          # any console command
horizon/target/fast/horizon-bots.exe --server 127.0.0.1:25601 --bots 2 --behaviour walk --seconds 20 \
  --name RusticBot --raw "<tick>:<id>:<hex>,..."      # raw ServerboundChat (9) / ChatCommand (7) / PlayerAction (41)
echo stop >> .local/rustic-server-test/console.in

# rustic run (crates/rusticmc-cli), the plugin as plugins/hello-dev, hot reload on save:
mkdir -p .local/rustic-dev-server   # with eula.txt (eula=true) and server.properties (online-mode=false)
target/debug/rustic run plugins/hello --server .local/rustic-dev-server \
  --server-bin horizon/target/fast/rustic-server.exe --watch

# Horizon's supervisor with rustic-server as its engine:
H=horizon/target/fast/horizon-server.exe; D=$PWD/.local/rustic-supervised
$H --dir $D init --eula --offline --port 25603 --world world --no-whitelist --yes
$H --dir $D config set "engine.path=$(cygpath -w $PWD/horizon/target/fast/rustic-server.exe)"
$H --dir $D start          # -> "The server is running (pid ...)"
$H --dir $D status         # -> Running, TPS 20.0, MSPT 0.4 ms average, 0.6 ms P95
$H --dir $D cmd rustic plugins     # -> Plugins (1): hello
$H --dir $D restart        # -> The game is running again.
$H --dir $D stop           # -> Stopped. (plugins disabled: "[rustic] Rustic stopping")
```

(Every Horizon run used scratch homes: `HORIZON_HOME`, `HORIZON_LAUNCHER_HOME`, `XDG_DATA_HOME`,
`XDG_CONFIG_HOME` under `.local/rustic-server-test/home`, `HORIZON_NO_AUDIO=1`,
`HORIZON_LAUNCHER_NO_SELF_UPDATE=1`.)

## Evidence (2026-11-11, Windows 11, `--profile fast`, Horizon a84f637dd + patches/)

Boot with plugins (`.local/rustic-run2.log`):

```
[03:50:09] [Server thread/INFO]: Done (2.318s)! For help, type "help"
[03:50:09] [Server thread/INFO]: [rustic] Rustic 0.1.0 (rustic:api 0.1) on Horizon (Minecraft 26.3) starting in ...\rustic-server-test\server
[03:50:09] [Server thread/INFO]: [rustic] [hello] Hello enabled: 2 banned words, 0 joins so far
[03:50:09] [Server thread/INFO]: [rustic] Rustic ready: 4 plugin(s) enabled (4 loaded, 0 failed)
[03:50:09] [Server thread/INFO]: [rustic] registering 25 command usage(s) with Horizon
```

Server-list ping (patch U1, the demo plugin sets the motd):
`{"description":{"text":"A Rustic server (demo plugin)","color":"green"},"players":{"max":20,"online":0},...}`

Bots join, chat, run commands; a plugin panic and a WASM trap disable only their plugin:

```
[03:50:32] [Server thread/ERROR]: [rustic] [rustic-buggy] panic in event: rustic-buggy panics on purpose — the plugin is disabled
[03:50:32] [Server thread/INFO]: System chat: RusticBot0 joined a Rustic server          <- join line from a plugin (U2)
[03:50:35] [Server thread/INFO]: [rustic] [rustic-demo] event player-chat: <RusticBot0> hi from a bot
[03:50:35] [Server thread/INFO]: [Not Secure] <RusticBot0> hi from a bot
[03:50:37] [Server thread/INFO]: [rustic] [rustic-demo] /demo from Player(...): demo Alice
[03:50:38] [Server thread/INFO]: [rustic] [rustic-demo] event player-chat: <RusticBot0> please cancelme   <- cancelled: no chat line
[03:50:39] [Server thread/INFO]: [rustic] [rustic-demo] event player-chat: <RusticBot0> what the heck     <- hello's filter cancels it
[03:50:40] [Server thread/ERROR]: [rustic] [buggy] panic in event: player-chat: boom in the chat handler (at fixtures\buggy\src\lib.rs:26:17) — the plugin is disabled
[03:50:40] [Server thread/INFO]: [rustic] registering 19 command usage(s) with Horizon    <- buggy's commands withdrawn
[03:50:42] [Server thread/INFO]: [Not Secure] <RusticBot0> still alive
[03:50:47] [Server thread/INFO]: [rustic] [rustic-demo] event player-quit: RusticBot0
```

Console admin, hot reload, timings, block events (U7 hand), MSPT:

```
[03:51:00] [Server thread/INFO]: [rustic] Plugins (4): buggy, hello, rustic-buggy, rustic-demo
[03:51:01] [Server thread/INFO]: [rustic] [hello] disabled (reloading)
[03:51:01] [Server thread/INFO]: [rustic] reloaded hello
[03:51:05] [Server thread/INFO]: [rustic] Enabled: buggy
[03:51:07] [Server thread/INFO]: System chat: Builder0 joined (join #3)                   <- hello's join line after its reload
[03:51:14] [Server thread/INFO]: System chat: Target tick rate: 20.0 per second. Average time per tick: 3.5ms   (2 bots streaming chunks)
[03:51:15] [Server thread/INFO]: [rustic] Timings (on): window 16.2 s, 323 ticks, mean 1.84 mspt, max 20.82 ms
[03:52:16] [Server thread/INFO]: [rustic] [rustic-demo] event block-place: minecraft:stone at BlockPos { x: -2, y: -63, z: -2 } with MainHand
[03:56:32] [Server thread/INFO]: [rustic] [rustic-demo] event block-break: minecraft:grass_block at BlockPos { x: 3, y: -61, z: 3 } in minecraft:overworld
[03:56:13] [Server thread/INFO]: System chat: Percentiles: P50: 0.4ms P95: 0.6ms P99: 0.7ms   (idle, 4 plugins)
```

A deliberately panicking adapter hook (`RUSTIC_TEST_PANIC=destroy_block`), no crash report, the
server goes on:

```
[03:55:58] [Server thread/ERROR]: [rustic] hook destroy_block panicked, ignored (the server goes on): RUSTIC_TEST_PANIC=destroy_block: a deliberate panic in the adapter
[03:56:02] [Server thread/INFO]: [rustic] [rustic-demo] event player-quit: Digger0
```

`rustic run --watch` (`.local/rustic-cli-run.log`):

```
     Running horizon/target/fast/rustic-server.exe in .local/rustic-dev-server
[04:02:03] [Server thread/INFO]: [rustic] Rustic ready: 1 plugin(s) enabled (1 loaded, 0 failed)
    Building hello v0.1.0 (wasm32-wasip2, debug)
   Reloading hello
[04:02:24] [Server thread/INFO]: [rustic] reloaded hello
     Stopped server exited (exit code: 0)
```

### Second round (2026-11-11): menus, boss bar, sidebar, drops, damage UUIDs, exact block state

Bot packet log (`python -I scripts/bot-capture-summary.py .local/ui3-capture.txt`, a bot that joined with the
built-in `rustic-ui` plugin loaded):

```
   0.06s SetObjective         rustic_ui | color | aqua | text | Rustic sidebar
   0.06s SetDisplayObjective  rustic_ui
   0.06s SetScore             Ui30 | rustic_ui*
   0.06s BossEvent            Rustic@ | color | gold | text | Rustic boss bar
   2.00s OpenScreen           color | dark_purple | text | Rustic menu
   7.09s ContainerClose
```

Server log (`.local/rustic-run8.log`, `.local/rustic-run11.log`, `.local/rustic-run12.log`):

```
[rustic-ui] boss bar 1 shown to Ui40: Ok(())
[rustic-ui] menu opened for c530c920-...: Ok(2)
[rustic-demo] event block-break: minecraft:dirt at BlockPos { x: 2, y: -60, z: 2 } in minecraft:overworld
[rustic-ui] minecraft:dirt at BlockPos { x: 2, y: -60, z: 2 } breaks without drops
[rustic-ui] menu Some(2) click slot 4 Left on minecraft:diamond (cancelled by default: true)
[rustic-ui] menu None closed              <- the plugin closed it; the core hides a menu id it no longer owns
System chat: Test failed                  <- `execute if entity @e[type=minecraft:item]`: nothing dropped
...rustic disable rustic-ui, same dig...
System chat: Test passed. Count: 1        <- control: the dirt dropped
[rustic-demo] event entity-damage: minecraft:zombie 00000001-0000-0002-0000-000300000006 hit by minecraft:player fdfb28f5-8332-38d7-8f84-79bc213bafde for 1 (minecraft:player_attack)
[rustic-demo] event entity-death: minecraft:zombie 00000001-0000-0002-0000-000300000004 killed by Some(...), drops []
[rustic-demo] event block-place: minecraft:oak_log[("axis", "y")] at BlockPos { x: 3, y: -60, z: -3 } with MainHand
System chat: Percentiles: P50: 0.4ms P95: 0.6ms P99: 0.8ms   (idle, 5 plugins)
```

The first round's checks still pass (chat reaches plugins and `cancelme` is cancelled, `/demo Alice`, `buggy`
disabled by its "boom" panic while the server goes on, `rustic reload hello`, no crash report).
