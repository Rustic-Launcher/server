# Rustic's patches on Horizon

Rustic links Horizon's game as a trait mod (ADR 0001). Where Horizon's mod API has no hook or helper
for something the WIT promises (docs/ARCHITECTURE.md §9, U1-U11), Rustic carries a **small patch**
written the way Horizon adds hooks itself, so each one can be proposed upstream unchanged:

- a hook is a `native_*` function (a `Game` method in `game/native_hooks.rs`, or a free function in
  the client library for connection-thread hooks) with a doc comment that is the contract, a call
  site, and a **pending API fragment** in `tools/native-hook-fragments/<name>.json` (catalog entry:
  name, target, side, contract, `@API:<name>@` marker). Horizon's build script
  (`crates/horizon-mod-api-gen`) generates the `Hooks` method, the event struct and the dispatch;
- a helper is a `native_*` `Game` method listed in a fragment's `helpers`, which the generator puts
  on `Host`;
- nothing is numbered: the maintainers stamp fragments (`tools/stamp_native_hooks.py`);
- zero cost with no mod: a hook with no listener is a loop over an empty slice, and the patches that
  change what the server sends keep the old path when no mod is loaded (or when the mods leave the
  value unchanged);
- each patch has tests in Horizon's style (`tests/mod_api_hooks.rs` for a mod's view of a hook,
  unit tests next to the code, `#[ignore]`d gameplay tests that need prepared resources).

Upstream status: all **to propose** to arismoko/horizon (where mod-API PRs go since 2026-11-09,
docs/ARCHITECTURE.md §6) or horizon-rust/horizon for the non-mod-API one (U11).

## The patches (`patches/*.patch`, on Horizon `a84f637dd`)

| # | Patch | What | Files | Rationale / upstream notes |
|---|---|---|---|---|
| U1 | `0001-rustic-server-list-ping-hook-status_response.patch` | hook `status_response(address, protocol, motd, description, max_players, online, sample)` on the connection thread, once per status request | `lan/conn.rs` (hook + call in `serve_status`), `lan/mod.rs` (`Shared::status_json_edited`, `StatusFields`), `native_client_hooks.rs`, `entry.rs`, fragment `status_response.json` | Bukkit's `ServerListPingEvent`. No `Game` there (like `handshake`); unchanged fields answer byte-for-byte as before (tested). |
| U2 | `0002-rustic-join-and-leave-lines-players-are-sent-go-thro.patch` | `player_join_message` / `player_leave_message` (pending hooks) now decide the line **players** are sent, on dedicated servers and LAN hosts, not only the host's own chat | `lan/hub.rs` (`join_lines`, `default_joined_line`, `send_system_line`, leave line held back while mods are loaded), `lan/guest.rs` (`leave_line`), `game/lan.rs` (asked once per join/leave), `native_hooks.rs` docs, both fragments | Bukkit's `PlayerJoinEvent.joinMessage` / `PlayerQuitEvent.quitMessage`. Their old doc said "a dedicated server sends no such line", which was not true (the hub sends it). With no mod, the hub sends its lines exactly as before. |
| U3 | `0003-rustic-player_died_v3-the-death-message-and-what-a-d.patch` | hook `player_died_v3(player, name, message, keep_inventory, keep_level)` | `game/lan_actions.rs` (`guest_die`, respawn XP), `game/item_entities.rs` (`drop_inventory` no longer reads the rule itself), `lan/guest.rs` (`keep_level` until the respawn), `native_hooks.rs`, fragment | Bukkit's `PlayerDeathEvent`. Asked only when a mod is loaded. |
| U5 | `0004-rustic-close_container-closes-any-menu-a-player-has-.patch` | helper `close_container(uuid) -> bool` | `game/native_menus.rs`, `game/lan_actions.rs` (`guest_open_windows` visible to the module), fragment | Bukkit's `closeInventory`; `close_menu` closed only a mod's menu. Same close as a player's own (`guest_close_all`) plus the client packet. |
| U6 | `0005-rustic-world_spawn-and-set_world_spawn-helpers.patch` | helpers `world_spawn()`, `set_world_spawn(pos, angle)` | `game/native_level_world.rs`, `game/commands.rs` (`set_world_spawn` factored out of `/setworldspawn`, `wrap_degrees` made `pub(super)`), fragment | Bukkit's `World.getSpawnLocation`/`setSpawnLocation`. Block writes on any loaded level were already upstream (`edit_blocks`, fragment `edit_blocks_any_level`): no patch needed for that half of U6. |
| U7 | `0006-rustic-acting_hand-helper.patch` | helper `acting_hand() -> Option<Hand>` | `game/native_server.rs`, fragment | The hand of `block_place` / `use_item` / `use_item_on` without new `_v2` hooks: one getter over `Game::hand`, valid in every hook of a player's turn. |
| U8 | `0007-rustic-richer-command-argument-kinds-and-bounds.patch` | usage kinds `string`, `string?`, `players`, `entity`, `entities`, `vec3`, `dimension`, `uuid`, `duration`, and bounds `integer(min,max)` / `number(min,max)` | `game/native_server.rs` (`parser`, test), `mod_api.rs` + `docs/native-mods.md` (the kinds' list) | Client-side parsers only (colouring, red errors, bounds); mods still get the raw line, so Rustic keeps parsing arguments itself. |
| U9 | `0008-rustic-per-player-sidebar-and-the-player-inventory-w.patch` | `sidebar(uuid, Option<&NativeSidebar>)` on `Host`; `NativeSidebar` public with `new` | `game/native_server.rs`, `game.rs`, `mod_api.rs`, fragment `sidebar_host.json` | The existing mixin helper, exposed. Objectives, scores and teams stay the vanilla commands (`run_command`), which Rustic's backend uses. |
| U10 | `0008-rustic-per-player-sidebar-and-the-player-inventory-w.patch` | helpers `inventory(uuid, kind)` / `inventory_typed`, `NativeInventory.selected` (the held hotbar slot) | `game/native_inventories.rs`, `game.rs`, `mod_api.rs`, fragment `inventory_read.json` | The getter mixin mods had; `NativeInventory` becomes public and `#[non_exhaustive]`. |
| U11 | `0009-rustic-opt-in-PROXY-protocol-v1-v2-for-servers-behin.patch` | opt-in HAProxy PROXY protocol v1 and v2 in front of the handshake: `proxy-protocol=true`, `proxy-protocol-trusted=<ip>,...` in `server.properties` | `lan/proxy_protocol.rs` (new), `lan/conn.rs` (`serve`), `lan/mod.rs` (`NetConfig.proxy_protocol`), `game/dedicated.rs` | For servers behind a TCP relay (Rustic Relay, HAProxy, nginx `stream`): bans, logs, `NativeServerPlayer.address` and the ping see the player. See below. |
| U12 | `0010-rustic-block_state_name-helper-names-a-block-state-i.patch` | helper `block_state_name(BlockStateId) -> Option<String>` (`minecraft:oak_log[axis=y]`) | `game/native_level_refs.rs`, a gameplay test in `game/lan.rs`, fragment `block_state_name.json` | Hooks hand over `BlockStateId`s (`block_place`, `destroy_block`, loot contexts) but `Host` could only name a position's state. Rustic's `block-place.block` is the exact state now. |
| U14 | `0012-rustic-name_picture-hook-a-picture-in-the-place-of-the-mark.patch` | platform hook `horizon_hooks::Hooks::name_picture(uuid) -> Option<u64>` (first answer wins): a crate's sprite (`players::Sprites` id) drawn whole in the place of the mark before that player's name, in the player list (8x8 in the mark's column, with the text's shadow) and on the name tag over their head; while it answers, that player's mark (`player_mark`, `NameDecoration::mark`) is not drawn | `horizon-hooks/src/lib.rs` (hook, `engine::name_picture`, recorder test), `horizon-client-render/src/tab_list.rs` (`TabInputs::picture`, `hook_sprite`, the column for marks or pictures, test), `horizon-client/src/game/{scoreboard,player_hooks,names}.rs` (asked per listed player and per tag, `Marks::picture`), `examples/probe/tab_list_fixture.rs` | Not a mod-API hook: `horizon_hooks` is the platform layer an executable registers into (Horizon's cosmetics, Play with Friends), and a `Mark` is `&'static` rectangles, which cannot hold a smooth picture. Used by the Rustic edition client's badge (launcher ADR 0072, Rustic ADR 0012). With no hook answering, nothing changes. |
| U13 | `0011-rustic-title_widgets_hidden-and-title_open-helpers.patch` | client helpers `title_widgets_hidden(hidden)` (the main title draws only its panorama; its buttons and bottom notice take no click, Tab or Enter) and `title_open(button) -> bool` (presses a main title button for the player as a click does: `singleplayer`, `multiplayer`, `realms`, `friends`, `options`, `language`, `accessibility`, `credits`, `quit`) | `title.rs` (`MainButton`, `set_widgets_hidden`, `press_main`, tests), `game/native_title_screen.rs` (new), `game.rs`, `game/window_handler.rs` (`idle` runs the queued press), fragment `title_screen.json` | Used by the Rustic edition client's main menu (launcher branch, ADR 0148; ADR 0011 here). Both `&self` (a `Cell` on `Game`, as `apply_screen_blur`) so the read-only screen hooks can call them; the press runs between frames, where `title_action` has the event loop. Not for the startup menu (no trait mod runs there). |

U4 (a host in every no-host hook) is not a patch: the fork's "clean break" does it; Rustic serves
those hooks from a per-tick snapshot meanwhile (crates/rusticmc-server/src/backend.rs).

### U11: PROXY protocol rules

- Off by default; on, `proxy-protocol-trusted` must name at least one IP (IPv4-mapped IPv6 matches
  its IPv4), else the server refuses to start with the reason.
- A connection from a trusted address **must** start with a v1 (`PROXY TCP4 ...\r\n`) or v2 header;
  without one it is closed. A connection from any other address **must not** carry one (closed:
  anyone could claim any address); other direct connections are served as before.
- v2 `LOCAL` (health checks), `UNSPEC`/`UNIX` and v1 `UNKNOWN` keep the proxy's address.
- The header is read before anything else (the legacy `0xFE` ping and the handshake follow
  untouched), with a 5 s limit.
- **Precedence with Velocity modern forwarding**: Velocity's signed forwarding, read during login,
  replaces the address (and the profile) the PROXY header gave; the header's address is what the
  handshake, the status ping and the login gate see before that. Use one or the other per listener
  in practice (a relay in front of Velocity speaks PROXY to Velocity, not to Horizon).
- The per-address connection limit still counts the proxy's address (it is taken at `accept`,
  before the header): a relay is one address with many players, and local addresses already get a
  place each (`lan/mod.rs`, `one_address_takes_at_most_its_share`).

## Tests (run 2026-11-10/11, Windows 11, shared target dir)

| Command | Result |
|---|---|
| `cargo test -p horizon-client-lib --no-default-features --lib -- lan::proxy_protocol lan::arrivals lan::tests` | 115 passed (new: 5 `proxy_protocol`, `a_status_edit_changes_the_answer_and_no_edit_changes_nothing`, `a_join_line_a_mod_chose_replaces_or_hides_the_vanilla_one`, `while_mods_are_loaded_the_leave_line_waits_for_the_game`) |
| `cargo test -p horizon-client --no-default-features --test mod_api_hooks` | 18 passed (new: `a_status_response_is_edited_in_turn_and_unchanged_fields_stay`, `a_death_hook_decides_the_message_and_what_is_kept`) |
| `cargo test -p horizon-client --no-default-features --lib -- native_server::tests world_spawn_sync_tests --include-ignored` (with `HORIZON_TEST_RESOURCES`) | 21 passed, run from a scratch folder with scratch homes (new: `richer_argument_kinds_and_bounds_map_to_vanilla_parsers`, `mod_helpers_read_the_inventory_close_any_menu_name_the_hand_and_move_the_spawn`; the existing `setworldspawn_*` tests pass over the factored-out `set_world_spawn`, `native_level_setters_tests` too) |
| `cargo check -p horizon-client --no-default-features --tests --bins`, and the `rustic-server` fast build over the patched crates | clean (the engine with every patch) |
| `python scripts/patches.py check` | `OK: 10 patch(es) apply on a84f637dd6` (U9 and U10 are one patch: both expose existing mixin helpers on `Host`) |
| U13 (2026-11-12): `cargo test -p horizon-client-lib --lib -- title::` / `cargo test -p horizon-client --lib -- native_title_screen` / `cargo clippy -p horizon-client --lib --tests -- -D warnings` | 97 passed (new: `hidden_main_widgets_are_neither_drawn_nor_pressed`, `pressing_a_main_button_for_the_player_does_what_its_click_does`, `main_button_names`) / 1 passed (`a_button_is_pressable_only_on_the_main_title_and_when_it_is_on`) / clean; `patches.py check`: `OK: 11 patch(es) apply on a84f637dd6`; exercised end to end by the Rustic edition client (launcher ADR 0148) |
| U14 (2026-11-12): `cargo test -p horizon-hooks --lib` / `cargo test -p horizon-client-render --lib -- tab_list` / `cargo clippy -p horizon-client -p horizon-hooks -p horizon-client-render --lib --tests -- -D warnings` | 4 passed / 4 passed (new: `a_picture_takes_the_place_of_the_mark_and_the_names_stay_aligned`; the `name_picture` dispatch is asserted in the hooks' recorder test) / clean; `patches.py check`: `OK: 12 patch(es) apply on a84f637dd6`; exercised end to end by the Rustic edition client's badge (launcher ADR 0072) |

## Workflow

```sh
python scripts/patches.py apply     # horizon/ -> branch rustic-patched = pinned commit + patches/*.patch
# edit in horizon/, then one commit per concern:
git -C horizon commit -am "rustic: <what>"        # or --amend / fixup the patch's own commit
python scripts/patches.py rebuild   # rewrite patches/*.patch from the branch
python scripts/patches.py check     # every patch applies on the pinned commit (CI)
python scripts/patches.py status
```

`horizon/` must be clean for `apply`/`rebuild`. The submodule pointer the superproject records
stays on the upstream commit: the patched branch exists only locally.

### Rebasing on a new Horizon

1. `git -C horizon fetch && git -C horizon checkout <new upstream commit>`; record it in the
   superproject (`git add horizon`) so `patches.py` pins it.
2. `python scripts/patches.py apply`. A patch that no longer applies stops `git am --3way`: fix the
   conflict in `horizon/`, `git -C horizon am --continue`, repeat.
3. Patches upstream took: drop them (`git -C horizon rebase -i` is not available to agents: reset the
   branch to the pinned commit and `git am` the remaining files by hand), then `rebuild`.
4. `python scripts/sync_horizon.py` (the server workspace's `[patch]` block, lock seed),
   `python -I scripts/horizon-extract-mod-api.py` and diff `docs/horizon/hooks.json` for API drift,
   rebuild `rustic-server`, rerun the tests above.
