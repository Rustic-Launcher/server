# Perms

Permission groups for Rustic servers: inheritance, weights, prefixes and suffixes, wildcard and
negated nodes, temporary nodes and groups, promotion tracks. Perms **provides the standard
services** `rustic:services/permission-provider@0.1.0` and `rustic:services/chat-meta@0.1.0`
(priority high): once installed, every permission check of the server goes through it, and chat
formatters (or `/msg` in essentials-lite) get prefixes and suffixes from it.

Concepts follow the usual permission-plugin model (groups, tracks, temporary nodes); the code and
texts are written from scratch for Rustic.

## How a check is resolved

For a player and a node, Perms looks at **sources** in order; the first source that holds a
matching entry decides:

1. the player's own nodes;
2. every group the player inherits (their groups, then those groups' parents, transitively),
   ordered by **weight** (highest first), then by inheritance depth (closest first), then by name.

Inside one source the most specific entry wins: `a.b.c`, then `a.b.*`, then `a.*`, then `*`.
A value is `true` (granted) or `false` (negated); typing a node with a leading `-` sets it to
`false`. Temporary entries count until they expire and shadow a permanent entry on the same node.
`group.<name>` is held by everyone who inherits `<name>`. When nothing matches, the answer is
"undefined" and the server falls back to the node's declared default (or op status).

Players without any group are in the **default group** (`default_group`, created automatically,
cannot be deleted); new players are put in it when they first join.

## Commands

Everything is under `/perms` (alias `/perm`), permission `perms.admin` (default: op). Arguments
complete with online and known players, groups, tracks and nodes. `<duration>` is `30s`, `10m`,
`2h`, `7d`, `1h30m`... `<player>` may be offline if they joined once (or a UUID).

| Command | What |
|---|---|
| `/perms` or `/perms help` | the command list (click to fill one in) |
| `/perms user <player> info` | groups (with expiry), primary group, prefix/suffix, own nodes |
| `/perms user <player> group add\|remove\|set <group>` | membership (`set` replaces every group) |
| `/perms user <player> group addtemp <group> <duration>` | temporary membership |
| `/perms user <player> permission set <node> [true\|false]` | own node (`-node` = false) |
| `/perms user <player> permission settemp <node> <true\|false> <duration>` | temporary node |
| `/perms user <player> permission unset <node>` | remove an own node |
| `/perms user <player> promote\|demote <track>` | move along a track |
| `/perms user <player> setprefix\|setsuffix [text]` | personal prefix/suffix (none: remove) |
| `/perms group <group> create [weight]` / `delete` / `info` | groups (`delete` also removes it from parents, players and tracks) |
| `/perms group <group> permission set\|settemp\|unset <node> ...` | group nodes |
| `/perms group <group> parent add\|remove <group>` | inheritance (cycles are refused) |
| `/perms group <group> setprefix\|setsuffix [text]` | `&` legacy codes or MiniMessage, stored as typed |
| `/perms group <group> setweight <weight>` | priority of the group |
| `/perms groups` | every group by weight (click for details) |
| `/perms track <track> create\|delete\|info` | tracks |
| `/perms track <track> append\|remove <group>` / `insert <group> <position>` | the ladder (lowest first) |
| `/perms tracks` | every track |
| `/perms check <player> <node>` | the verdict and where it comes from (source and matching key) |
| `/perms reload` | re-read `config.toml` and the stored data |
| `/perms export` / `/perms import` | write / read `perms.toml` in the data folder |

Tracks: `promote` puts a player who is on none of the track's groups in the first one, otherwise
moves them one step up; `demote` moves them one step down, and off the track from its first group.
A player in two groups of the same track is refused (fix it by hand).

## Permissions

| Node | Default | |
|---|---|---|
| `perms.admin` | op | every `/perms` command |

## Configuration (`plugins/perms/config.toml`)

```toml
config_version = 1
lang = "en"                    # "en" or "fr"
default_group = "default"      # created when missing, cannot be deleted
expiry_check_ticks = 20        # how often temporary entries are checked (20 = 1 s)
notify_expiry = true           # tell online players when a temporary group/node of theirs expires
export_file = "perms.toml"     # /perms export and /perms import, in the data folder
```

## Data

* Plugin storage (JSON): `groups`, `tracks`, and one `users/<uuid>` entry per player (name,
  groups, temporary groups, nodes, temporary nodes, prefix, suffix). Written on every change.
* `/perms export` writes the same data as TOML (`[groups.<name>]`, `[tracks.<name>]`,
  `[users.<uuid>]`); edit it and `/perms import` to load it back. Imports are validated (names,
  nodes, parents, tracks and memberships must name existing groups, no inheritance cycle) and a
  bad file changes nothing.
* Temporary entries store their expiry as a unix time in seconds, so they survive restarts.

## Services for other plugins

| Service | Answers |
|---|---|
| `permission-provider.check(player, node)` | the resolution above (`allow` / `deny` / `undefined`) |
| `groups` / `primary-group` | direct groups, highest weight first; the first is the primary group |
| `add-group` / `remove-group` / `set-node` | edit a player (unknown groups are refused) |
| `chat-meta.prefix` / `suffix` | the player's own, else the highest-weight inherited group's |
| `chat-meta.group-prefix` | a group's prefix |

Prefixes are returned as stored (`&c[Staff]` or `<red>[Staff]`); consumers render either form.

## Build, test, size

Perms is a **standalone Cargo workspace** (ADR 0032): the SDK feature `provide-permissions`
selects the component's world, and cargo unifies features across a workspace, so perms is
excluded from `plugins/Cargo.toml` and built on its own. It still uses `plugins/target`
(`plugins/.cargo/config.toml`, read when cargo runs from inside `plugins/`).

```sh
cd plugins/perms
cargo build --target wasm32-wasip2 --release   # ../target/wasm32-wasip2/release/perms.wasm
cargo test                                      # 21 unit + 1 manifest + 13 integration
rustic build --release && rustic check --deny-warnings   # ../target/rustic/perms.rplugin
# from the repository root instead: CARGO_TARGET_DIR=plugins/target cargo build --manifest-path plugins/perms/Cargo.toml ...
```

Component 939.9 KiB, `.rplugin` 293.2 KiB (release, `opt-level = "s"`, LTO, stripped).

The integration tests (`tests/plugin.rs`) load the real `perms.wasm` in `rusticmc-test` and cover
every command, the `perms.admin` denial, the join event, both services, expiry on the timer,
persistence across a plugin reload, export/import, completion and the French messages. The expiry
test sleeps 2 s because expiry uses the wall clock.

## Live check (rustic-server on Horizon, 2026-11-11)

`.local/batchA-live.log`, two bots (`Alice0`, `Bob0`), commands from the console and from the bots:

```
[05:34:46] [rustic] service rustic:services/permission-provider@0.1.0: provider none -> perms
[05:34:46] [rustic] service rustic:services/chat-meta@0.1.0: provider none -> perms
[05:34:47] [rustic] [perms] Perms enabled: 1 groups, 0 players, 0 tracks (default group `default`, lang En)
[05:35:08] System chat: [Perms] Created group staff (weight 10).
[05:35:08] System chat: [Perms] Prefix of group staff set to: [Staff]
[05:35:08] System chat: [Perms] Appended staff to track ladder.
[05:35:17] System chat: [Perms] Added Alice0 to group staff.
[05:35:17] System chat: [Perms] Promoted Bob0 from default to staff on ladder.
[05:35:17] System chat: [Perms] Demoted Bob0 from staff to default on ladder.
[05:35:17] System chat: [Perms] Alice0 → perms.admin = true (from group staff, key perms.admin)
[05:35:17] System chat: [Perms] Bob0 → essentials.warmup.bypass = false (from group default, key essentials.warmup.bypass)
[05:35:17] System chat: [Perms] Exported to perms.toml (2 groups, 2 players, 1 tracks).
[05:38:12] [rustic] reloaded perms
[05:38:12] [rustic] [perms] Perms enabled: 2 groups, 2 players, 1 tracks (default group `default`, lang En)
[05:38:12] System chat: Primary group: staff
[05:38:12] System chat: [Perms] Alice0 → fly.test is undefined here: ...   <- a 20 s temporary node, expired
```

What the bots received (system chat from their packet captures): Bob0 (not admin) gets Horizon's
"unknown command" for `/perms` (Horizon hides commands the player has no permission for); Alice0
(in `staff`) runs `/perms group staff info` (`Prefix: [Staff] (&c[Staff])`, `perms.admin = true`),
`/perms user Alice0 permission settemp fly.test true 20s`, `/perms check` (`from user, key
fly.test`), `/perms tracks` (`ladder: default → staff`), `/perms user Bob0 info`; Bob0 gets
"Your rank is now staff." / "default." on promote/demote. The essentials-lite permissions of both
bots came from Perms (`essentials.*` on `default`, `-essentials.warmup.bypass`).
