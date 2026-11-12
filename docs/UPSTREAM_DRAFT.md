# Draft: coordinating hook points with Horizon (not posted)

Where it would go: an issue on **arismoko/horizon** (since 2026-11-09 mod-API PRs go there, per
horizon-rust/horizon#7386), linked from horizon-rust/horizon#4953 (the hooks tracker). Per Horizon's
AGENTS.md, an issue opened by an agent starts with the "Filed by" line naming the account owner. The login
below is the GitHub account the `gh` CLI is authenticated as on this machine (`BouBouw`); the user must confirm the
account and the wording before anything is posted. **Do not post automatically.**

---

> 🤖 Filed by BouBouw's AI agent (Claude Code, Claude Opus 5.5), not by BouBouw personally.

**Title:** Server hooks for a sandboxed plugin host (trait mod): 7 small additions

Hello, and thank you for the trait-mod API: it covers almost everything a Bukkit-style plugin platform needs.

We are building **Rustic**, an MIT/Apache-2.0 plugin host for Horizon servers: one trait mod (`impl Hooks`) linked
into an engine executable that calls `entry::main_with`, running sandboxed WebAssembly plugins (wasmtime component
model) on the server thread. It will use `register_commands` + `refresh_commands` (as documented, "a plugin host that
reloads a plugin"), `permission_check`, and about sixty of the server hooks. We mapped our API against `a84f637dd`
(NATIVE_HOOKS_API 415) and found a few gaps. We would rather add them upstream than carry patches, and we are happy
to write the PRs (one hook each, small, with tests) if you agree with the shape:

1. **Server list ping**: a hook in `lan/conn.rs` `serve_status`, beside `native_handshake`, to change the MOTD, the
   max players and whether the player sample is shown (Bukkit's `ServerListPingEvent`). Connection thread, no
   `Game`, one call per ping.
2. **Join and leave lines on servers**: `player_join_message` / `player_leave_message` only edit the LAN host's own
   chat line; the line other players receive is built in `lan/hub.rs` (`joined_line`, the "left the game" line).
   Either let those hooks decide the hub's broadcast too, or a `_v2` that does (Bukkit's `PlayerJoinEvent` /
   `PlayerQuitEvent` messages).
3. **Player death outcome**: in `guest_die`, a hook (or `player_died_v3`) with a mutable death message
   (`Option<Text>`), `keep_inventory` and `keep_level` (Bukkit's `PlayerDeathEvent`).
4. **Close a vanilla container** for a player (`close_menu` closes only a mod's own menu).
5. **`LevelRef` world access**: `level_set_block(level, pos, state, flags)`, a surface height read and the world
   spawn (get/set) on any open level, so a scheduled task does not need the level to be "in place".
6. **Inventory getter on `Host`**: `native_inventory(uuid, kind)` exists for mixin mods but is not on `Host`; with the
   selected hotbar slot.
7. **`hand`** in `block_place` and `use_item` (v2), and a `host` in the hooks that have none today
   (`player_chat(_v2)`, `player_move`, `player_teleport`, `living_incoming_damage_v3`): we saw the clean-break plan
   ("every hook gets a host") and would align with it.

Also useful but optional: argument kinds with bounds, quoted strings, multi-player selectors, coordinates and
dimensions in `NativeCommand` usages, so clients colour and complete them.

Questions:

- Is arismoko/horizon the right place for these, and should they wait for the clean break or go in as pending
  fragments now?
- Is there a recommended way for a trait mod to follow API changes during the break (a changelog of renamed or
  merged hooks we could track)?

Our full map, with file and line references, is public in our repository (docs/ARCHITECTURE.md); we can paste the
relevant parts here. Thanks for your time.
