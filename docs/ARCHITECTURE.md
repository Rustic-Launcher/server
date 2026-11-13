# Rustic on Horizon: architecture and hook map

Status: Phase 0 map, written against Horizon **`a84f637dd`** (submodule `horizon/`, `NATIVE_HOOKS_API = 415`,
`horizon/crates/horizon-client/src/game/native_hooks.rs:1086`), 2026-11-10. Every `file:line` below is relative
to `horizon/crates/horizon-client/src/` unless another crate is named. Companion files:

- [horizon/hooks.md](horizon/hooks.md), [horizon/hooks.json](horizon/hooks.json): every non-client `Hooks`
  method, exact signature, event fields, mutable fields, side, cancellable (generated, `scripts/horizon-extract-mod-api.py`).
- [horizon/host.md](horizon/host.md): the 441 `Host` methods, exact signatures.
- [horizon/build.md](horizon/build.md), [horizon/run.md](horizon/run.md): build times, sizes, a running server and bots.
- ADR [0001](DECISIONS/0001-integrate-as-horizon-trait-mod.md); draft to upstream: [UPSTREAM_DRAFT.md](UPSTREAM_DRAFT.md).

Legend used below: **✓** served by an existing hook/helper; **≈** served with glue in Rustic (several hooks,
a command fallback, or a degraded form); **✗ MISSING** needs a patch in Horizon (or upstream).

---

## 1. Process and integration shape

```
horizon-server (supervisor, unchanged)            rustic-server (= Horizon engine + Rustic trait mod)
  server.toml -> server.properties                  main(): name_mods(["rustic"]);
  console / RCON / query / panel / mgmt API  --->    entry::main_with(Arc::from([Box::new(RusticHooks)]))
  spawns  engine.path --dedicated --nogui            |-- Horizon game (Game, hub, levels)  -- one process
                                                     `-- RusticHooks: impl Hooks  -> rusticmc-core + rusticmc-host (wasmtime)
```

- The dedicated server is the game executable with `--dedicated` (`entry.rs:43`, `game/loading_window.rs:360`
  calls `Game::run_dedicated`, `game/dedicated.rs:904`). It runs **on the process's main thread**, which the log
  labels "Server thread" (`entry.rs:50`).
- `horizon-server` (`crates/horizon-server`) is a supervisor: it writes `server.properties` from `server.toml`,
  serves RCON, GS4 query, the web panel/control API and vanilla's management protocol, and spawns
  `engine.path` as `<engine> --resources <dir> --control-socket <dir>/.horizon/... --dedicated --nogui` in the
  server folder with `HORIZON_NO_AUDIO=1` (`crates/horizon-server/src/engine.rs:247-271`). Any executable that
  calls `entry::main_with` is a valid engine: `rustic-server` slots in through `engine.path`, exactly like the
  launcher's native-mod export (`docs/native-mods.md` "Standalone server export").
- Trait mods are installed once: `entry::main_with(mods)` (`entry.rs:10`) does
  `mod_api::MODS.set(mods)` (`entry.rs:12`), a process-wide `OnceLock<Arc<[Box<dyn Hooks>]>>`
  (`mod_api.rs:1014-1016`). A second list is ignored with a warning. `mod_api::name_mods(&["rustic"])`
  (`mod_api.rs:1028`) names entries for crash reports and for `set_mod_on` (`mod_api.rs:1047`, an
  operator's off switch skipping a mod's hooks from the next dispatch).
- Generated dispatch (`OUT_DIR/mod_api_generated.rs`, `mod_api.rs:1251-1253`): `dispatch_<hook>(host?, mods, args...)`
  builds the event struct, loops over `entered(mods, "<hook>")` (`mod_api.rs:1074`), which marks the running
  mod for crash naming, and for a cancellable hook stops at the first `Flow::Cancel`. Scalars a mod may change are
  copied back after the loop (`*amount = event.amount`).
- The executable must keep the 8 MB main-thread stack the game needs on Windows: Horizon's own build script
  links `/STACK:8388608` (MSVC) or `-Wl,--stack,8388608` (`horizon/packaging/windows/exe.rs:18-28`,
  `crates/horizon-client/build.rs:24`). `rustic-server`'s `build.rs` must do the same (or run `main_with` on a
  thread with a big stack, as `examples/wind_burst_hook_probe.rs:31-38` does for debug builds).
- `main.rs:13` keeps `horizon_mods::api_level::MARK` in the binary for the launcher/supervisor to read; `rustic-server`
  should embed it too.

## 2. Execution model

### 2.1 Threads (dedicated server)

| Thread | Created at | Runs | Hooks that fire there |
|---|---|---|---|
| main = "Server thread" | process main; `game/dedicated.rs:918` `run_dedicated_inner` | the whole tick loop, all gameplay, console/RCON lines, the hub (network state machine for play) | every hook that has a `host`, plus the no-host chat/move/teleport/damage hooks (see 2.4) |
| "Server console handler" | `game/dedicated.rs:941` | reads stdin lines into an mpsc channel; the tick drains it (`dedicated.rs:1331-1336`) | none |
| "Server Watchdog" | `game/dedicated.rs:65` | `max-tick-time` watchdog | none |
| "horizon-lan-accept" | `lan/mod.rs:762` | `accept()` loop | none |
| "horizon-lan-conn" (one per connection until play) | `lan/mod.rs:1016` | handshake, status, login (encryption, Velocity forwarding), configuration (`lan/conn.rs:605-1660`) | `handshake` (`conn.rs:687`), `player_pre_login` (`conn.rs:1102`), `player_profile` (`conn.rs:1118`): **no `Game`, no host** |
| "horizon-net-N" (pool, one per two cores) | `lan/io_pool.rs:284` | play-state socket I/O, framing, compression, keep-alives; decodes serverbound play packets and sends them to the server thread as `NetEvent::Packet` (`lan/conn.rs:1761-1797`) | none |
| "horizon-chunk-encode-i" | `lan/chunks.rs:505` | chunk packet encoding | none |
| job pool (`horizon_threads::jobs`, "15 worker threads" at start) | `horizon-client-render/src/streaming.rs` | world generation, light, chunk loading | worldgen hooks (`chunk_generate`, `lod_column`, installed in `entry.rs`) run on workers |
| "horizon-region-writer", "horizon-player-writer", "Far chunk reader #n" | `save.rs:3046`, `lan/playerdata.rs:404`, `lan/far.rs:103` | disk I/O | none |
| mod worker pool | `Host::spawn_task` / `spawn_background_job` | a mod's own off-thread work; the callback returns to the server thread "during `native_tick`" | (Rustic's async instances can use their own threads instead) |

Consequence for Rustic: `rusticmc-core`'s single-threaded runtime on the server thread matches Horizon. Only three
login-time hooks run elsewhere (connection threads), and they have no host anyway.

### 2.2 The tick loop and its phases

`game/dedicated.rs:1318-1450` (one iteration = one server tick, 50 ms at rate 20):

1. `catch_unwind` around console lines + control socket (`dedicated.rs:1331`) -> `console_line` (dedicated.rs:1664-1667),
   then vanilla/mod console commands.
2. pause-when-empty bookkeeping, autosave schedule.
3. `catch_unwind` around `tick_server_timed` (`dedicated.rs:1378`) -> `Game::tick_server` (`game/levels.rs:591`):
   1. `native_configs_poll` (levels.rs:603).
   2. **`server_tick` phase `ServerTickStart`** (`levels.rs:609` -> `game/native_lifecycle.rs:235-241`), only when
      the world runs (not paused / held).
   3. For each level to tick (`levels_to_tick`, levels.rs:613; overworld first): `enter_level` swaps that dimension's
      whole state into `Game` (`docs/server-levels.md`), then
      - **`level_tick_pre(dimension)`** (`levels.rs:636`),
      - streaming/loading (`update_dedicated_streaming`, `update_world`),
      - first level: `Game::tick` (`game.rs:5507`) = shared parts (functions, weather, `native_tick` hook at
        `game.rs:5595`, then `tick_world`); other levels: `tick_world` (`game.rs:5601`).
        Inside `tick_world`: **network in** = `tick_lan_packets` (`game/lan.rs:2418`): on the first level
        `hub.poll` drains every `NetEvent` (`lan/hub.rs:2339`), then each level drains the packets of its players;
        events are applied as the guest (`lan_guest_packet`, `game/lan_actions.rs:2169`); then block ticks,
        entities, mobs, block entities (`game.rs:5627-5770`); then **`tick_lan`** (`game/lan.rs:2627`):
        mods' command tree refresh (`register_native_commands`), **player handling** `tick_guests`
        (`game/lan_actions.rs:4226`: mining, item use, `player_tick` hook at `lan_actions.rs:4409`), entity
        **tracking** and the per-guest **send** of the tick's changes (queued to the outboxes; the net threads write).
      - **`level_tick_post(dimension)`** (`levels.rs:660`).
   4. idle levels close (`close_idle_levels`).
   5. **`server_tick` phase `ServerTickEnd`** (`levels.rs:702`).
   6. `server_tick_timed(wall, cpu, spans)` when spans are on (`levels.rs:758`).
4. idle kick, clock record, far-ring brake, wait for the next tick (`wait_for_next_server_tick`).

While a `server_tick` hook runs, *which level `self.world` holds is not defined* (hook doc, generated
`Hooks::server_tick`). Block/world helpers that act on "the authoritative level in place" (`set_block`,
`edit_blocks`, `surface_heights`, `summon`) are only reliable inside that level's `level_tick_pre/post` or a
player's hook (their level is in place). `LevelRef` helpers (`Host::level(dim)`, `level_block_state`,
`level_set_weather`...) work on any open level from any server hook.

### 2.3 Guest-state swapping (`as_guest`)

A server player's state lives in a `Guest` record of the hub. To run shared gameplay code for them,
`Game::as_guest` (`game/lan_actions.rs:411`) / `enter_guest` (`lan_actions.rs:472`) swaps their position,
inventory, game mode, mining and item use into `Game`'s single-player fields for one call, then
`leave_guest` swaps back. Hooks heard inside a guest's turn (`destroy_block`, `right_click_block`, `use_item_on`,
`block_place`, `perform_command`...) see that player through `Host::acting_player()` and the "acting player"
helpers (`count_items`, `take_items`, `held`, `tell` reaching them). Levels swap the same way (`enter_level`).

**Where a Rustic callback is safe:**

- Any hook that has `host: &mut dyn Host` on the server thread: safe to call plugins and to use helpers.
  Writes to "the level in place" are safe for the level that is in place (the acting player's, or the
  `level_tick_*` dimension).
- `server_tick` `ServerTickEnd` is the natural place for Rustic's scheduler (`scheduler.run-later/run-timer`), the
  async-to-main mailbox and deferred writes; world writes from tasks go through `LevelRef` helpers or are queued to
  the target dimension's `level_tick_post`.
- No-host hooks (2.4): plugins may run (same thread) but imports that need the game must read a snapshot or be
  queued.
- Connection-thread hooks: never call into a plugin instance there; hand the event to the server thread and wait
  (bounded), or answer from a cache.
- Reentrancy: a helper called from inside a hook can fire other hooks synchronously (`teleport_player` ->
  `player_teleport`; `set_game_mode` -> `player_game_mode_changed`; `run_command*` -> many). Rustic's dispatch must
  be reentrant (no lock held across a `Host` call; nested events queued or dispatched with a depth limit).

### 2.4 Hooks with no host

44 of the non-client hooks get only their event (list in [hooks.md](horizon/hooks.md), marked *no host*). For
Rustic the relevant ones: `player_chat`/`player_chat_v2`/`chat_decorate` (run inside `hub.poll`, the `LanHost`
is taken out of `Game`), `player_move`, `player_teleport` (mid-teleport), `living_incoming_damage`/`_v3`,
`living_damage_pre/post`, `player_hurt_by_player`, `server_packet`, `handshake`, `player_pre_login`,
`player_profile`. Upstream's fork plans "every hook gets a host" in its API clean break (#7386); until then Rustic
serves these with a per-tick snapshot (online players, positions, names) and queued effects.

## 3. Packet paths and the hooks on them

All on the server thread unless noted; "C" = cancellable.

**Login** (connection thread "horizon-lan-conn"):
`lan/conn.rs:662 serve_connection` reads the handshake -> **`handshake`** (C, no host; may rewrite the host string,
`conn.rs:687`) -> intent: status -> `serve_status` (`conn.rs:757`, **no hook**) | login ->
`serve_login` (`conn.rs:983`): hello, name check, online-mode authentication or Velocity forwarding
(`lan/forwarding.rs`), the login gate (whitelist, bans, full; `game/dedicated.rs:1101-1110`), then
**`player_pre_login`** (no host; each mod may push a `Receiver` the login waits on, 20 s max, `conn.rs:937-972`,
called at `conn.rs:1102`), **`player_profile`** (mutable profile properties, `conn.rs:1118`) -> `LoginFinished` ->
configuration (`login_and_configure`, `conn.rs:1212`: registries, packs, code of conduct, resource pack) -> `play`
(`conn.rs:1659`): the connection moves to the io pool and the hub gets a `Joined` event.
Server thread, `game/lan.rs:2117`: **`player_login`** (C, host, `reason`; `lan.rs:2127`) -> `lan_guest_joined`
(placement; the hub broadcasts the join line itself, `lan/hub.rs:1505-1511`) -> **`player_logged_in`**
(`lan.rs:2227`).

**Chat** (`ServerboundChat`): `hub.poll` -> `lan/hub/chat.rs`: signature chain, chat filter, then (only when a mod
is loaded) **`player_chat`** and **`player_chat_v2`** (C, no host, `replies`; `chat.rs:279-281`) -> **`chat_decorate`**
(returns the displayed content, `chat.rs:299`) -> broadcast -> `HostEvent::PlayerChat` (log/events, `lan.rs:2103`).

**Command** (`ServerboundChatCommand[Signed]`): `lan_guest_packet` -> `guest_command` (`game/lan_actions.rs:3661`):
**`player_command`** (C, host, replies; `lan_actions.rs:3672`) -> message commands, `/list`, mute, tempban ->
**`player_command_level`** (may change the level for this line, `lan_actions.rs:3732`) -> a mod-registered command
is queued (`GuestCommand`) and run by `run_guest_commands` (`lan_actions.rs:3775`) as the guest:
`line_allowed` (level or permission node via `permission_check`) -> **`perform_command`** (C=`ran`,
`game/native_server.rs:1746`); otherwise vanilla's dispatcher (`game/commands.rs`, crate `horizon-command`).
Console: `dedicated.rs:1664 console_command` -> **`console_line`** (C, `source` "console"/"rcon") -> vanilla or
**`perform_console_command`** (`native_server.rs:1771`). Tab completion: **`tab_complete`** /
**`tab_complete_v2`** (`lan_actions.rs:3973-3980`).

**Movement** (`MovePlayerPos/Rot/PosRot`): `hub.poll` -> the hub's movement checks (speed, collision, anticheat)
-> **`player_move`** (C, no host, `to` mutable; `lan/hub.rs:4494`, via `PLAYER_MOVE` set in `game.rs:12440`) for
every accepted packet that changed position or look. Teleports: **`player_teleport`** (C, no host, `to` and the
relative flags mutable; `server_travel.rs:972`) then **`player_teleported`** (`server_travel.rs:949`);
dimension change **`player_changed_dimension`** (`server_travel.rs:1068`).

**Block break** (`PlayerAction` start/stop/abort digging): `lan_guest_packet` -> **`left_click_block`** (C,
`lan_actions.rs:2253`) -> mining progress in `tick_guests` -> `tick_mining` (`game.rs:5236`) ->
`break_block_as_player` (`game.rs:4987`) -> **`destroy_block`** (C, host, acting player; `game.rs:5016`) -> drops
through **`loot_drops`** (mutable list; `NativeLootDrops.block = Some(pos, state)`) and **`block_exp`**.

**Block place / use** (`UseItemOn`): `use_hand.rs` -> **`right_click_block`** / **`right_click_block_v2`** (C,
`use_hand.rs:22-24`) -> **`use_item_on`** (C, item's use, `use_hand.rs:247`) -> `BlockItem.place`:
**`block_place`** (C, guests only, `item_actions.rs:166`) -> after placement **`place_block`** (listen,
`use_hand.rs:525`). `UseItem` (air): **`use_item`** (C, `use_hand.rs:250`).

**Entity interaction** (`Interact`): **`interact_entity`** / **`interact_entity_v2`** (C, `item_actions.rs:133-134`),
**`entity_interact_specific`** (interact-at); attack: **`attack_entity`** (C, `lan_actions.rs:3548`) -> damage
pipeline **`living_incoming_damage`** -> **`_v2`** (host) -> **`_v3`** (no host, attacker/victim UUIDs) ->
`living_damage_pre/post`, **`living_death`** (C).

**Inventory** (`ContainerClick`): **`inventory_click`** (C, vanilla menus, `lan_actions.rs:2020`),
**`inventory_drag`**; a mod's own menu: **`menu_clicked`** / **`menu_closed`** (`native_menus.rs:459-462`);
vanilla close: **`player_container_close`** (`native_menus.rs:416`).

**Custom payload** (`ServerboundCustomPayload`): next tick, host available: **`custom_payload_from_client`**
(any channel but Horizon's own, `native_net.rs:115`), **`player_register_channels`** (`minecraft:register`).

**Disconnect**: the io pool notices -> `HostEvent::Left` (`lan.rs:2234`) -> **`player_logging_out`** while the
player is still in the world (`tell_mods_logging_out`, `lan_actions.rs:4449-4458`) -> save -> the hub broadcasts
the "left the game" line itself (`lan/hub.rs:2617`) -> **`player_logged_out`** (`lan.rs:2314`). Kicks:
**`player_kicked`** (`lan.rs:2345`).

## 4. Commands, configuration, world format

- **Command engine**: `crates/horizon-command` is a port of Brigadier (string reader, literal/argument nodes with
  redirects, best-parse, suggestions, vanilla's errors; `src/lib.rs:1-5`), with the vanilla tree in `vanilla.rs`
  and `tree_*.rs`, argument types in `args.rs`, and the tree's network form in `network.rs`. Execution lives in
  `game/commands.rs` + `game/commands/*.rs`.
- **Mod commands**: `register_commands` (`Hooks`) is asked when the hub starts and after `Host::refresh_commands()`
  (`game/native_server.rs:1539 register_native_commands`). Each `NativeCommand` (`mod_api.rs:767-818`) is one usage
  string: words then `<name:kind>` / `[name:kind]` (optional, last). Kinds and their Brigadier parser
  (`native_server.rs:38-53`): `word`, `word?` (asks the server: `tab_complete_v2`), `player_name` -> `brigadier:string word`;
  `text`, `text?` -> greedy; `integer`, `number`, `boolean`; `player` -> `minecraft:entity` single player;
  `item` -> `item_stack`; `block_pos`. **No min/max bounds, no quoted string, no multi-player selector, no vec3 or
  dimension argument types.** Usages merge into one tree per command, sent after vanilla's, filtered by the
  player's level (`NativeCommand::op(usage, level)`) or by a permission node (`.permission(node)`, decided by
  `permission_check`); `.replace_vanilla(true)` takes over a vanilla name; otherwise a vanilla name is refused
  with a warning. Execution passes the **raw line** (`perform_command`, `perform_console_command`): the mod parses
  arguments itself. `player_commands_sent(_v2)` lets a mod filter each player's tree; `Host::resend_commands(uuid)`
  re-sends one tree.
- **Permission levels**: vanilla's 0-4 from `ops.json` (`player_permission`), `op-permission-level`,
  `function-permission-level`; console and RCON run at 4. Permission nodes: `Host::has_permission(uuid, node, default_level)`
  asks the mods' `permission_check` (first `Some` wins), `permission_groups`, `player_permission_value`
  (tri-state), `primary_group`: a permissions engine (Rustic's `rusticmc-permissions`) plugs in as the answerer for
  every mod.
- **Config**: `server.toml` (supervisor; `[server] [world] [properties] [engine] [backups] [panel] [api]
  [transfer] [anticheat] [chat_filter] [combat] [prune_chunks]`, `docs/hosting.md` §4) is written into vanilla's
  `server.properties` before each start; the engine reads `server.properties` (crate `horizon-server-files`) plus
  Horizon keys in it (`horizon-accept-older-clients`, `max_view_distance` -> far ring, `horizon-clients-only`...),
  `eula.txt`, `ops.json`, `whitelist.json`, `banned-*.json`, `usercache.json`, `whitelist-bypass.txt`,
  `forwarding.secret`. Mods have their own config hooks (`register_configs`, `config_loading/reloading`) and
  `Host::data_dir(mod_id)`, `config_dir`, `server_dir`, `world_dir`.
- **World format**: vanilla's (Anvil regions, `level.dat`, `playerdata`/`players/data`, `entities/`, `poi/`),
  round-tripping with vanilla; 1.8-26.2 worlds are upgraded by Horizon's datafixers (`horizon-world/src/datafix/`).
  Mod data on players is saved under `NeoForgeData` (`Host::player_persistent_data`).

## 5. WIT -> Horizon mapping

Conversions used throughout: WIT `uuid {hi, lo}` <-> `u128`; `text` (JSON component) -> `horizon_client::text::Text`
via `horizon_client_core::component::to_text(&json, ...)` or `Text::literal(plain).with_component(json)` (sends the
exact JSON to clients), and back with `Text::component()`; `location` <-> `(dimension, [f64;3], yaw, pitch)`;
`item-stack` <-> `ItemStack` via `Host::item_from_snbt` and the stack's NBT; `block-state` <-> the
`"id[k=v,...]"` string that `level_block_state` returns and `NativeBlockEdit.palette` takes, or `BlockStateId`
via `default_state` + properties; `entity-ref.id` = Horizon's entity id (`Host::entity(id)` -> `EntityRef`).

### 5.1 `game.wit` imports

| WIT function | Horizon | Notes / cost |
|---|---|---|
| `server.broadcast` | ✓ `Host::broadcast(Text)` | chat of every player + log |
| `server.dispatch-command` | ✓ `Host::run_mod_command(line)` (reaches mods' commands incl. Rustic's) or `run_command_captured` | level 4, from the overworld spawn; teleports to another level apply next tick |
| `server.get-tick-stats` | ✓ `tick_stats()` (rate, avg ms) + `tps()[0]` (1-minute) ; `tick_times()` for percentiles | |
| `server.max-players` | ✓ `max_players()` | |
| `server.motd` | ✓ `motd()` -> String (server.properties form) -> text | |
| `server.version` | ≈ Rustic-side string + Horizon commit (compile-time) | |
| `server.plugins`, `self-meta` | ≈ Rustic-side | |
| `server.offline-player` | ✓ `profile_by_name(name)` (online, else `usercache.json`) | |
| `server.operators` | ≈ read `ops.json` under `Host::server_dir()`; online ones: `player_permission(uuid) > 0` | no helper lists ops |
| `players.online` / `name` / `by-name` / `is-online` | ✓ `server_players()` (`NativeServerPlayer`: uuid, name, permission, dimension, position, yaw/pitch, game_mode, health, address, latency, xp...) ; `profile_by_name` | one Vec per call: cache per tick |
| `players.get-location` | ✓ `server_players()` dimension/position/yaw/pitch | |
| `players.address` / `ping` | ✓ `NativeServerPlayer.address` / `.latency` | |
| `players.locale` | ✓ `player_client(uuid).language` | |
| `players.send-message` / `send-action-bar` | ✓ `tell` / `action_bar` | |
| `players.send-title` | ✓ `title(uuid, title, subtitle, fade_in, stay, fade_out)` | |
| `players.play-sound` | ✓ `play_sound_to(uuid, sound, category, None, vol, pitch)` | |
| `players.teleport` | ✓ `teleport_player(uuid, dim, pos, yaw, pitch)` | fires `player_teleport` re-entrantly |
| `players.get/set-game-mode` | ✓ `NativeServerPlayer.game_mode` / `set_game_mode` | fires `player_game_mode_changed` |
| `players.health/set-health`, `food-level`, `saturation`, `experience-level` (+ setters) | ✓ `server_players()`, `player_food`, `set_player_status(uuid, health, food, xp)` | |
| `players.max-health` | ✓ `attribute(uuid, "minecraft:max_health").1` | |
| `players.set-flying` | ✓ `set_flight(uuid, allow, flying)` | |
| `players.kick` | ✓ `kick(uuid, &Text)` | |
| `players.is-op` / `set-op` | ✓ `player_permission(uuid) >= 1` / ≈ `run_command("op <name>")` / `deop` | |
| `players.get/set/delete-data` | ≈ `player_persistent_data` / `set_player_persistent_data` (a compound saved with the player; Rustic stores `ByteArray`s under `rustic:<plugin>`) | **online players only** ("None for a player not reached"); offline: Rustic KV fallback |
| `inventories.get-slot` / `contents` | ≈ `player_stacks(uuid)` (slots 0-35) + `player_equipment(uuid, EquipmentSlot::{Feet,Legs,Chest,Head,OffHand})` | Horizon's `NativeInventoryKind::Player` layout = WIT's (0-8 hotbar, 9-35, 36-39 feet..head, 40 off hand). The full getter `native_inventory(uuid, kind)` (`game/native_inventories.rs:276`, 688) exists but is **not on `Host`**: proposable (U10) |
| `inventories.set-slot` | ✓ `set_inventory_slot(uuid, Player, slot, expected, new)` | compare-and-set on `expected` |
| `inventories.give` | ✓ `give_stack(uuid, stack)` -> remainder | |
| `inventories.take` | ✓ `take_stacks(uuid, &[(slot, stack)])` after reading the inventory | |
| `inventories.clear` | ✓ `set_inventory(uuid, Player, vec![None; 41], None)` | |
| `inventories.held-slot` | ≈ `player_compound(uuid)` -> `SelectedItemSlot` (serialises the whole player: ~tens of µs) | the selected slot is not on `NativeServerPlayer`; proposable (U10) |
| `inventories.close-container` | **✗ MISSING** for vanilla containers (`close_menu` closes only a mod's menu) ; ≈ open then close a 1-row native menu | patch: `native_close_container(uuid)` in `game/native_menus.rs`, O(1) |
| `menus.open` | ✓ `open_menu(uuid, rows, title, items)` -> window id (`open_menu_with_input` for editable slots) | |
| `menus.set-item` | ✓ `set_menu_items(uuid, all)` (Rustic keeps the model) | sent with the tick's container sync |
| `menus.set-title` | ≈ re-open with `open_menu` (vanilla has no title-update packet) | |
| `menus.close` / `open-menu` | ✓ `close_menu(uuid)` / Rustic-side tracking + `menu_contents` | |
| `worlds.all` | ≈ `level_stats`/`level(dim)` over the three vanilla dimensions + datapack dims (`dimension_of_world`) | |
| `worlds.get-block` | ✓ `level_block_state(level(dim), pos)` -> `"id[props]"` | any open level |
| `worlds.set-block` | ≈ `set_block(dim, pos, BlockStateId, flags)` **only for the level in place**; else `run_command("execute in <dim> run setblock ...")` (enters the level, `levels.rs:570`) | **✗ MISSING (proposable)**: `level_set_block(LevelRef, pos, state, flags)` (#7387 handles) |
| `worlds.set-blocks` / `fill` | ≈ `edit_blocks(dim, &NativeBlockEdit{palette, runs, neighbour_updates})` in the level's `level_tick_*`; command `fill` otherwise | `physics=false` -> `neighbour_updates=false` |
| `worlds.highest-block-y` | ≈ `surface_heights(dim, cols, heightmap)` (level in place) ; else scan `level_block_state` | proposable: `level_height_at(LevelRef, x, z, heightmap)` |
| `worlds.is-chunk-loaded` | ✓ `chunk_is_loaded(dim, [cx, cz])` | |
| `worlds.keep-chunk-loaded` | ✓ `force_chunk_in("rustic:<plugin>", dim, chunk, hold)` (or `chunk_load_request/release`) | release all on disable |
| `worlds.time` / `set-time` | ✓ `level_day_time(level)` / `level_set_day_time(level, t)` | |
| `worlds.get-weather` / `set-weather` | ✓ `level_is_raining`/`level_is_thundering` / `level_set_weather(level, clear, time, rain, thunder)` | |
| `worlds.spawn` | **✗ MISSING** getter | patch: `native_world_spawn(LevelRef) -> (pos, angle)`; O(1) |
| `worlds.set-spawn` | ≈ `run_command("setworldspawn x y z yaw")` (overworld; check 26.3's dimension handling) | proposable `level_set_spawn` |
| `worlds.spawn-entity` | ✓ `summon(dim, kind, pos, nbt)` (level in place) or command `execute in <dim> run summon` | returns id; uuid via `entity_uuid` |
| `worlds.remove-entity` | ✓ `remove_entity(dim, id)` / `remove_mob` / `kill_mob` | |
| `worlds.entities-near` | ✓ `entities_in(dim, min, max)` + `entity_kind`/`entity_uuid` | |
| `worlds.entity-location` | ✓ `entity_position(EntityRef)` + `entity_rotation` | |
| `worlds.spawn-particle` | ≈ `level_send_particles(level, &ParticleOptions, ...)` for simple particles (`ParticleOptions::simple`); with options -> `run_command("execute in .. run particle ...")` | |
| `worlds.play-sound` | ✓ `level_play_sound(level, sound, at, source, vol, pitch)` | |
| `worlds.players` | ✓ `server_players()` filtered by dimension | |
| `scoreboards.*` (objectives, display, scores, teams) | ≈ vanilla commands through `run_command` / `run_command_captured` (`scoreboard objectives add/remove/setdisplay/modify`, `scoreboard players set/reset/get`, `team add/remove/modify/join/leave`) | ~10-50 µs per command; no typed scoreboard helper (`native_sidebar` exists but is mixin-only, not on `Host`). Proposable: typed scoreboard helpers |
| `bossbars.*` | ✓ `boss_bar(uuid, id, Some(&NativeBossBar{name, progress, color, overlay, ...}))` per viewer | Rustic keeps the bar and re-sends to viewers on change |
| `tablist.set-header-footer` | ✓ `tab_list(uuid, header, footer)` | |
| `tablist.set-display-name` | ✓ `set_list_name(uuid, Option<&Text>)` | (`set_display_name` is chat) |
| `messaging.register-incoming` / `unregister-incoming` | ≈ Rustic-side filter over `custom_payload_from_client` | to advertise to clients, send `minecraft:register` with `send_custom_payload_to_player` |
| `messaging.send` | ✓ `send_custom_payload_to_player(uuid, channel, data)` (any client, 1 MiB) | |
| `messaging.behind-proxy` | ≈ `forwarding.secret` present in `server_dir()` and `online-mode=false` | no helper |

### 5.2 `events.wit` events

| WIT event | Horizon hook(s) | Thread / host | Mutable fields and gaps |
|---|---|---|---|
| `player-pre-login` | ✓ `player_login` (C, `reason`) on the server thread; address captured earlier from `player_pre_login` (conn thread, no host) or `handshake` | server, host | cancel+`kick-message` ✓. Earlier refusal on the conn thread possible via `player_pre_login`'s deferred `Receiver` (20 s max) |
| `player-join` | ✓ `player_logged_in` | server, host | **`join-message`: ✗ MISSING on a dedicated server**: `player_join_message` only edits the LAN host's own chat line (`lan.rs:2214`); the line players get is built by the hub (`lan/hub.rs:571 joined_line`, sent at `hub.rs:1505-1511` and `:1729`) |
| `player-quit` | ✓ `player_logging_out` (still in the world) / `player_logged_out` | server, host | **`quit-message`: ✗ MISSING** (hub line at `lan/hub.rs:2617`; `player_leave_message` is host-chat only) |
| `player-chat` | ✓ `player_chat_v2` (C, `replies`, sender level) + `chat_decorate` (displayed content) | server, **no host** | `display`: ≈ `chat_decorate` replaces the *content* shown (signed body kept, the `<name>` part comes from the chat type and `set_display_name`); a full custom line = cancel + `broadcast_quiet` (unsigned) |
| `player-command` | ✓ `player_command` (C, replies) | server, host | |
| `server-command` | ✓ `console_line` (C, source console/rcon, replies) | server, host | |
| `player-move` | ✓ `player_move` (C, `to` mutable) | server, **no host** | fires per accepted move/look packet (up to ~20/s/player), not "once per tick": Rustic applies the threshold before entering WASM. See section 9 for the WIT wording |
| `player-teleport` | ✓ `player_teleported` (after; from/to/dimension) ; `player_teleport` (C, before, no host) available | server | |
| `player-change-world` | ✓ `player_changed_dimension` | server, host | |
| `block-break` | ✓ `destroy_block` (C) + `acting_player()` for player/dimension + `Host::block(pos)` for the state | server, host | `drop-items=false` ≈ clear `loot_drops` for that block position (same tick; `NativeLootDrops.block`) and `block_exp` |
| `block-place` | ✓ `block_place` (C, stack, state; guests only) | server, host | `hand` not in the event: ≈ compare with `held_stack`/off hand. Proposable `block_place_v2` with `hand` |
| `player-interact` | ✓ `right_click_block_v2` (C, hand), `left_click_block` (C), `use_item` (C, right-click air), `player_arm_swing` (left-click air, listen) | server, host | `use_item` lacks the hand (proposable) |
| `player-interact-entity` | ✓ `interact_entity_v2` (C, hand) / `entity_interact_specific` | server, host | entity uuid via `entity_uuid(entity(id))` |
| `entity-damage` | ✓ `living_incoming_damage_v3` (C, `amount` mutable, victim/attacker ids and player UUIDs) | server, **no host** | non-player victim/attacker UUIDs need `entity_uuid` (host): ≈ Rustic-side id->uuid cache, else zero. `living_incoming_damage_v2` has a host but no attacker UUIDs |
| `player-death` | ✓ `player_died_v2` (rendered message, read-only) / `player_died` (cause key) | server, host | **`death-message`, `keep-inventory`, `keep-level`: ✗ MISSING** (patch in `guest_die`, `game/lan_actions.rs:4471-4570`). Player death drops do pass through `loot_drops` (`item_entities.rs:610-622`) but clearing them destroys the items |
| `entity-death` | ✓ `living_death` (C) + `mob_killed` (killer player) + `loot_drops` (mutable drops) + `living_experience_drop` (mutable xp) | server, host | four hooks for one death: Rustic correlates by entity id within the tick |
| `player-respawn` | ✓ `player_respawn_position` (dimension, pos, yaw, pitch all mutable) ; `player_respawned` after | server, host | |
| `inventory-click` | ✓ `inventory_click` (C; window, slot, button, mode) for vanilla menus; `menu_clicked` for Rustic menus (never moves items) | server, host | `item`/`cursor` via `menu_contents`/`menu_cursor`/`inventory` |
| `inventory-close` | ✓ `player_container_close` (vanilla) / `menu_closed` (Rustic menus) | server, host | |
| `player-game-mode-change` | ✓ `player_game_mode_changed` (C) | server, host | |
| `server-list-ping` | **✗ MISSING**: `serve_status` (`lan/conn.rs:757`) builds the status JSON with no hook | conn thread | patch: `native_status_response(address, &mut motd, &mut max, &mut players_sample_hidden)` beside `native_handshake`; one call per ping; Rustic answers from a snapshot or a bounded hand-off to the server thread |
| `plugin-message` | ✓ `custom_payload_from_client` (next tick, in order) | server, host | |
| `chunk-load` / `chunk-unload` | ✓ `chunk_load` (`new` flag) / `chunk_unload` | server, host | per chunk: Rustic skips WASM when nobody subscribed |
| `custom-event` | ≈ Rustic-side | | |

`core.wit` services map as: `commands.register` -> `register_commands` + `refresh_commands` on any change
(argument kinds: `word`/`string` -> `word`, `greedy` -> `text`, `integer`/`number`/`boolean`/`player`/`item`/`block-pos`
direct, `players`/`position`/`world`/`duration` -> `word`/`text` parsed by Rustic, `dynamic-suggestions` -> `word?`/`text?`
+ `tab_complete_v2`, `min`/`max` checked by Rustic); dispatch via `perform_command`/`perform_console_command`;
`commands.reply` -> `feedback` (inside `perform_command`) / `tell` / console log; `permissions.has` ->
Rustic's engine, which also answers Horizon's `permission_check` for everyone; `scheduler` -> `server_tick`
(`ServerTickEnd`), `current-tick` -> `ServerTickEvent.context.tick`; `logging` -> the `log` crate (Horizon's logger);
`storage` -> files under `Host::data_dir("rustic")`; `http` -> Rustic's async instances.

## 6. Upstream status (checked 2026-11-10 with `gh`)

- **Native hooks API**: `NATIVE_HOOKS_API = 415` (`game/native_hooks.rs:1086`); "hook points are only ever added:
  each addition raises" it (`native_hooks.rs:986`). The catalog `horizon/native-hooks.json` lists 434 hooks
  (270 server, 10 both) and 177 helpers; **98 more hooks and 264 helpers are merged on main as pending fragments**
  (`horizon/tools/native-hook-fragments/*.json`, API number assigned when stamped: `@API:name@` in the docs).
  The generated trait has 532 methods, `Host` 441. Many hooks Rustic needs are pending ones (`block_place`,
  `player_move`, `player_pre_login`, `player_chat_v2`, `inventory_click`, `permission_check`, `console_line`,
  `custom_payload_from_client`, `living_incoming_damage_v3`...).
- **#3706** (world open/close hooks) and **#3878** (server tick hooks): merged 2026-10-30.
- **#7386** (native mod API without mixins or rebuilds) and **#7387** (Host services: handles for entities, players,
  levels): open; "this work continues on a fork, arismoko/horizon", which builds the mod API, prebuilt game, slots,
  internals and **wasm mods**. On **2026-11-09** the maintainer posted on #7386: *"mod-related PRs now go to Ari's
  fork, arismoko/horizon, which carries the mod API (native hooks, native, content and wasm mods) and reviews and
  merges that work."* The fork announces a **"one clean break"** (one hook per moment, handles, `Host` split by side,
  ID-based helpers removed, every hook gets a host), and answered on #7386 that a scripting host as an ordinary mod
  is something they plan to support.
- **#13914** (Java bridge as a separate optional download) open, nothing on main; **#10216** design note closed
  unmerged. Optional for Rustic (`bridge/java` is separate anyway).
- **#6304** (native mods on dedicated servers: who builds the engine) closed; today: `horizon-launcher native-build
  --server-output` exports an engine, `engine.path` points the supervisor at it, no compatibility handshake.
- **Admin API**: the supervisor's control API (`/api/v1`: status, logs, command, players, kick/ban/op, whitelist,
  backups, config, SSE events, Prometheus; `crates/horizon-server/src/api.rs`, `admin.rs`), MCP (`mcp.rs`), RCON
  (`rcon.rs`), GS4 query (`query.rs`), vanilla's JSON-RPC management protocol over WebSocket/TLS
  (`management*.rs`, `websocket.rs`); the engine side answers on its control endpoint
  (`game/dedicated/management.rs`, `events.rs`). All reusable unchanged with `rustic-server` as the engine.
- **Proxy**: Velocity modern forwarding v1 (`lan/forwarding.rs`, `docs/velocity-forwarding.md`; `forwarding.secret`,
  online-mode off); `accepts-transfers` and the play-state `Transfer` packet (`lan/hub.rs:1183-1189`); Horizon's own
  hand-over protocol between servers (`[transfer]`, `game/dedicated/transfer.rs`, `lan/arrivals.rs`). **Cookies**:
  `CookieResponse` is read and ignored during login (`lan/conn.rs:1244`); no cookie API. BungeeCord legacy
  forwarding: not found.
- **Online mode**: implemented (`OnlineMode::generate`, session server check, `enforce-secure-profile`, chat signing
  chains in `lan/hub/chat.rs`).

## 7. What Horizon provides / will provide / what Rustic adds

| Horizon provides today | Horizon (fork) is building | Rustic adds |
|---|---|---|
| The whole game server, vanilla-exact; supervisor with RCON, query, panel, management protocol, backups, updates | handles (`EntityRef`, `PlayerRef`, `LevelRef`), "parts", `Host` split by side, every hook with a host | a **sandboxed** plugin runtime (wasmtime components, fuel/epoch/memory limits, trap isolation) |
| ~450 server hooks, 441 helpers, typed and generated | prebuilt game + slots: mods without rebuilding the game | a stable, versioned plugin ABI (`rustic:api@0.1.0`) independent of Horizon's churn |
| command registration with permission nodes, permission-check delegation | wasm mods (fork) | Bukkit-like services: event priorities/`ignore-cancelled`/monitor, scheduler, commands with typed args, permissions engine, KV storage, services, menus, timings |
| mod config files, persistent player data, attachments | | hot load/unload/reload of plugins at runtime, per-plugin fault isolation, `/rustic timings` |

### 7.1 Hook points by origin (file, function, event, estimated cost)

Costs: Horizon's dispatch is a virtual call per mod per hook (~ns). Rustic's own cost per *subscribed* event is one
component call: ~0.5-2 µs with small records, ~2-10 µs with strings/lists (to be benchmarked, docs/PERFORMANCE.md);
unsubscribed events cost a branch.

**Already upstream (main, `a84f637dd`)**: every ✓ row of section 5 (catalog or pending fragment). The hottest:
`player_move` (`lan/hub.rs:4494`, per move packet: up to 20/s/player), `living_incoming_damage_v3`
(`game/native_hooks.rs:7935`, per hit), `chunk_load/unload` (`native_hooks.rs:11877-11880`, per chunk), `server_tick`
(2/tick), `level_tick_pre/post` (2/level/tick), `entity_tick_pre/post` (per entity per tick: Rustic must not
subscribe by default).

**Proposable upstream** (general, NeoForge/Bukkit-shaped, small):

| # | Where (Horizon file, function) | Hook / helper | Per-call cost |
|---|---|---|---|
| U1 | `lan/conn.rs:757 serve_status` (conn thread) | `status_response(address, protocol, &mut motd: Text, &mut max_players, &mut hide_sample)` (Bukkit `ServerListPingEvent`) | 1 call per ping |
| U2 | `lan/hub.rs:1505-1511` & `:1729` (join line), `:2617` (left line) | make `player_join_message`/`player_leave_message` cover the hub broadcast on servers (or `_v2`) (Bukkit `PlayerJoinEvent.joinMessage`) | 1 per join/leave |
| U3 | `game/lan_actions.rs:4471 guest_die` | `player_death_v3(player, &mut Option<Text> message, &mut keep_inventory, &mut keep_level, &mut drops?)` (Bukkit `PlayerDeathEvent`) | 1 per death |
| U4 | `game/native_hooks.rs` `no host` hooks (`player_chat(_v2)`, `player_move`, `player_teleport`, `living_incoming_damage_v3`) | a host on each (already planned by the fork's clean break) | 0 extra |
| U5 | `game/native_menus.rs` | `close_container(uuid)` for vanilla containers | O(1) |
| U6 | `game/native_level*.rs` (LevelRef family) | `level_set_block(LevelRef, pos, state, flags)`, `level_height_at`, `level_spawn` / `level_set_spawn` | O(1) |
| U7 | `game/item_actions.rs:166`, `use_hand.rs:250` | `hand` in `block_place` / `use_item` (v2) | 0 |
| U8 | `game/native_server.rs:38 parser` | argument kinds `number(min,max)`, `string` (quoted), `players` (multi selector), `vec3`, `dimension` | 0 at run time |
| U9 | `game/native_server.rs` | typed scoreboard/team helpers (expose `native_sidebar` on `Host` too) | O(1) |
| U10 | `game/native_inventories.rs:276` | expose `native_inventory(uuid, kind)` (with `selected`) on `Host` (it exists for mixin mods) | O(slots) |

**Rustic patch only** (if upstream declines or until it lands; each as a small file in `patches/`): the same
U1-U3 and U5-U6 at the same places, nothing else. No Rustic-only behaviour needs a patch: everything else is glue
on the existing API.

## 8. Relation with Horizon's mods: recommendation

**Confirmed: link Rustic as one trait mod (`impl Hooks`) into a `rustic-server` executable that calls
`horizon_client::entry::main_with`, and run Rustic's own wasmtime runtime inside it.** Evidence:

1. The trait-mod surface is already what a Bukkit-class platform needs: 24 of the 25 WIT events can be delivered
   (only `server-list-ping` has no hook); 3 of them carry mutable fields Horizon cannot honour yet (join/quit
   message, death message/keep-inventory/keep-level) and 3 are partial (chat `display`, `block-place.hand`,
   non-player UUIDs in `entity-damage`). Of the game imports, all but `inventories.close-container` and
   `worlds.spawn` are reachable (some via vanilla commands or with a cost). See section 5.
2. It is the integration Horizon itself documents for server engines with mods (`docs/native-mods.md` "Standalone
   server export", `horizon-server config set engine.path=...`), so the supervisor, RCON, panel, management and
   backups are reused unchanged.
3. `Host::refresh_commands` is documented for "a plugin host that reloads a plugin"; `permission_check` lets one
   mod be the permission engine of all; `name_mods`/`set_mod_on` give an operator off switch.
4. Wrapping Horizon's own WASM mods is not possible: that runtime is not on main; it lives on the fork, which is in
   a declared API break. Depending on it would tie Rustic's ABI to an unstable, unofficial API. Rustic's WIT is
   its own contract; the fork's wasm mods can coexist later as another trait mod.
5. Mixin (`horizon-mixins`) integration means a full game rebuild per change and no isolation; a separate process
   (proxy-level) would see packets, not gameplay (no `destroy_block`, damage, loot...).

Costs accepted: Rustic rebuilds the engine for each Horizon commit it supports (12 min cold on Windows, the
`horizon-client` crate incrementally); Rustic pins a Horizon commit; trait mods cannot be added at run time
(irrelevant: Rustic is the one mod, plugins come and go inside it).

## 9. Corrections to AGENTS.md "Horizon facts" and to the WIT

AGENTS.md:

- "434 hooks, 270 server-side" counts only `native-hooks.json`. The generated `Hooks` trait has **532** methods
  (the catalog plus 98 pending fragments): 338 `server`, 15 `both`, 78 `client`, 100 without a side, 1 unlisted.
  `Host` has **441** methods, not 177 (177 catalog helpers + 264 pending fragment helpers).
- `player_chat` is cancellable but has **no `host`** (also `player_move`, `player_teleport`,
  `living_incoming_damage` v1/v3...: 44 server hooks). Rustic's backend must work without `Host` in them.
- "A panic in a hook propagates and kills the game": on a dedicated server the tick and console prelude are inside
  `catch_unwind` (`game/dedicated.rs:1331`, `:1378`); a panicking hook ends the tick loop, writes a crash report
  naming the mod and hook (`crash::NativeModGuard`), and `stop_after_crash` (`dedicated.rs:2369`) saves and stops.
  So the server still goes down (the conclusion holds: Rustic must `catch_unwind` every hook body), but it is a
  clean crash-stop, not an abort. Conn-thread hooks panicking end that connection's thread.
- Mod API work now goes to **arismoko/horizon** (decided 2026-11-09 on #7386), not to horizon-rust/horizon.
- The dedicated server runs on the process main thread; on Windows a `rustic-server` binary needs the 8 MB stack
  link argument (section 1).
- Command kinds also include `word?`, `text?`, `player_name`; `NativeCommand::permission(node)` and
  `replace_vanilla(true)` exist.

WIT (`crates/rusticmc-api-wit/wit/`):

- `events.wit` `player-move`: "reported at most once per tick per player" conflicts with cancellation: Horizon asks
  per packet and needs the answer synchronously. Suggest: "for each move the server accepts whose distance exceeds
  the threshold".
- `player-join.join-message`, `player-quit.quit-message`, `player-death.{death-message, keep-inventory, keep-level}`,
  `block-place.hand`, `server-list-ping` (all fields): need U1-U3/U7 (or patches) before they can be honoured;
  until then the host must document them as ignored.
- `entity-damage.target/attacker.uuid` for non-player entities cannot be filled inside the no-host hook without a
  Rustic-side cache (U4 fixes it).
- `inventories.close-container`, `worlds.spawn`: need U5/U6.
- `commands.arg-kind.string` (quoted), `players`, `position`, `world`, `duration`, and `min/max`: Horizon's client tree
  cannot express them (U8); Rustic sends `word`/`text` and validates itself, so the client shows no red/colour
  feedback for them.
- `players.get-data` on an offline player: not reachable through Horizon (persistent data is "None for a player not
  reached").

### 9.1 Status of U1-U11 (2026-11-11): Rustic patches

Every gap above that needs Horizon is now a patch in `patches/` (written as upstream hooks/helpers with
fragments and tests; list, rules and workflow in [PATCHES.md](PATCHES.md)), used by `crates/rusticmc-server`:

| Gap | Patch | Rustic side |
|---|---|---|
| U1 server-list ping | hook `status_response` (conn thread) | `server-list-ping`: handed to the server thread, fired at the tick's end, answered within 250 ms (`ping.rs`) |
| U2 join/quit lines | `player_join_message` / `player_leave_message` decide the hub's lines | `player-join.join-message`, `player-quit.quit-message` honoured |
| U3 death | hook `player_died_v3` (message, keep_inventory, keep_level) | `player-death` mutable fields honoured |
| U4 host everywhere | none (the fork's clean break) | snapshot + queued writes (`backend.rs`) |
| U5 close container | helper `close_container` | `inventories.close-container` |
| U6 spawn / writes | helpers `world_spawn`, `set_world_spawn` (writes on any level were already upstream: `edit_blocks`) | `worlds.spawn`, `worlds.set-spawn` |
| U7 hand | helper `acting_hand` | `block-place.hand` |
| U8 argument kinds | `string`, `players`, `entity(ies)`, `vec3`, `dimension`, `uuid`, `duration`, `integer(min,max)`, `number(min,max)` | the core may map its kinds onto them (it still parses the raw line) |
| U9 scoreboards | `sidebar` on `Host` | per-player sidebars; objectives/teams via vanilla commands |
| U10 inventory | helpers `inventory` / `inventory_typed` with `selected` | `inventories.contents`, `get-slot`, `held-slot` |
| U11 PROXY protocol | `proxy-protocol` / `proxy-protocol-trusted` | real player addresses behind Rustic Relay |
| U12 state names | helper `block_state_name` | `block-place.block` is the exact state about to be set |
| U13 title screen (client) | helpers `title_widgets_hidden`, `title_open` (ADR 0011) | none on the server: the Rustic edition client's main menu (launcher branch, ADR 0148) |
| U14 name picture (client) | platform hook `horizon_hooks::Hooks::name_picture` (ADR 0012): a crate's sprite in the place of the mark before a player's name, in the player list and on the name tag | none on the server: the Rustic edition client's Rustic badge (launcher branch, ADR 0072) |
| U15 widget skin (client) | `horizon_client_render::ui::skin` (ADR 0013): an executable installs one `Skin` at startup; every `ui::nine_slice`, `ui::blit_sprite` and `ui::tiled(_sized)` asks it first by GUI sprite name (buttons, sliders, fields, check boxes, tabs, scroll bars, tooltips, menu/list backgrounds, separators) and draws vanilla's sprite only when it declines; the centred labels (`ui::scrolling_text_around_at`) and lines (`ui::centered_text`) likewise; `Skin::begin(atlas)` once a frame from `Game::hud` | none on the server: the Rustic edition's look of every vanilla and Horizon screen (launcher branch, ADR 0160) |
| U16 menu helpers (client) | helpers `languages()`, `set_language(code)`, `server_list()`, `join_server(address)`, `add_server(name, address)` (ADR 0013, fragment `client_menus.json`) | none on the server: the edition's language screen and in-game server directory (Discover) (launcher branch, ADR 0160) |

Also honoured without a patch (crates/rusticmc-server): `block-break.drop-items = false` (that break's
`loot_drops` and `block_exp` are cleared), `entity-damage` UUIDs of mobs (an id -> UUID table kept from
`entity_join_level` while a plugin listens; damage comes from `living_incoming_damage_v3`, which covers every
hit). Still not honoured: `entity-death.dropped-exp` (reported 0), `player-interact` for clicks in the air.

## 10. Risks

| Risk | Detail | Mitigation |
|---|---|---|
| Upstream drift | The trait is generated from source + catalog; pending hooks have no API number yet; the fork announces a clean break (helpers removed, hooks merged, `Host` split). Event structs are `#[non_exhaustive]` (fields may be added), but names can change in the break | pin the submodule; `scripts/horizon-extract-mod-api.py` -> diff `docs/horizon/hooks.json` on every bump; keep the Horizon adapter (`crates/rusticmc-server`) as the only code touching `mod_api`; coordinate (UPSTREAM_DRAFT.md) |
| Where to upstream | horizon-rust redirects mod-API PRs to an unofficial fork | coordinate with both; keep patches minimal and local in `patches/` |
| WASM call cost on hot hooks | `player_move` (~20/s/player), damage, chunk load, entity ticks | subscribe-gated dispatch; Rust-side prefilters (move threshold); batch chunk events per tick; benchmarks (criterion) in `rusticmc-host` |
| Threading | three login hooks on conn threads; no-host hooks on the server thread with the hub taken; reentrant hooks from `Host` calls | never call a plugin off the server thread; snapshot + queues for no-host hooks; reentrant dispatcher with depth limit |
| OnceLock mod list | `MODS` set once in `main_with`; no runtime add of trait mods | Rustic is the single trait mod and manages plugins itself; `refresh_commands` after a reload |
| Panics | a panic in any hook crash-stops the server | `catch_unwind` around every hook body in `RusticHooks`; plugin faults are traps, contained by wasmtime |
| Build cost | 12 min cold engine build on Windows; ~7 GB target | shared `horizon/target`; incremental `fast` profile; CI caches |
| Licence/EULA | engine needs the user's prepared resources and `eula=true` | resources stay in `.local/`; docs say who accepts the EULA |
