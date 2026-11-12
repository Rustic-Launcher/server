# Plugin format reference

This page is the complete reference for packaging a Rustic plugin: the manifest
(`rustic.plugin.toml`), the `.rplugin` archive, dev-mode plugin directories, how the server
finds, orders, loads and unloads plugins, and where a plugin's data lives. The API a plugin
calls is the WIT contract `rustic:api@0.1.0` (`crates/rusticmc-api-wit/wit/`); the Rust SDK
(`rusticmc-api`) wraps it. The implementation is `crates/rusticmc-core` (`manifest`,
`archive`, `discovery`, `storage`, `config` modules).

## 1. The manifest: `rustic.plugin.toml`

```toml
[plugin]
id = "homes"                    # required, [a-z0-9_-]{1,64}, unique on the server
name = "Homes"                  # required, 1-64 characters, shown to admins
version = "1.2.0"               # required, semantic version (pre-release/build allowed)
api = "0.1"                     # required, plugin API "MAJOR.MINOR" the plugin targets
authors = ["Alice", "Bob"]      # required, at least one non-empty name
license = "MIT"                 # required, an SPDX expression
entry = "homes.wasm"            # required, the component, relative path inside the package
description = "Set and teleport to homes"   # optional, at most 1024 characters

[dependencies]                  # optional table
required = { economy = "^1.0" } # must be installed, enabled first, version must match
optional = { placeholders = "*" }   # enabled first when installed (and matching)
load_before = ["chat"]          # these plugins are loaded after this one when installed

[capabilities]                  # optional table; everything is denied by default
network = false                 # or a list of host names: ["api.example.com", "*.example.org"]
filesystem = "data-folder"      # "none" (default) or "data-folder"

[limits]                        # optional overrides, capped by the server's rustic.toml
fuel_per_call = 50000000        # fuel (WebAssembly instructions, roughly) per call
memory_mb = 64                  # linear memory limit
```

Validation is strict; every problem is reported at once, and a plugin with any problem is not
loaded:

| Key | Rule |
|---|---|
| unknown keys | refused anywhere (a typo is an error, never a silently ignored setting) |
| `id` | `[a-z0-9_-]{1,64}`; `rustic`, `minecraft` and `horizon` are reserved |
| `version` | [semver](https://semver.org) |
| `api` | exactly `MAJOR.MINOR`, compatible with the server's plugin API (see below) |
| `entry` | relative, forward slashes, no `.`/`..`/empty segments, ends in `.wasm`, the file must exist in the package |
| `dependencies.*` keys | valid plugin ids, never the plugin itself; a plugin cannot be both `required` and `optional`, nor listed in `load_before` while being a dependency |
| version requirements | Cargo syntax (`^1.0`, `>=1.2, <2`, `~1.4`, `=1.0.3`, `*`). `*` matches every version, pre-releases included; other requirements follow semver (`^1.0` does not match `1.1.0-beta`) |
| `capabilities.network` | `false` or a list of host names (lowercase letters, digits, `-`, at least one dot; a leading `*.` matches any subdomain, not the domain itself). `true` is refused: list the domains. No scheme, port or path |
| `capabilities.filesystem` | `"none"` or `"data-folder"` |
| `limits.fuel_per_call` | positive |
| `limits.memory_mb` | 1 to 4096 (then capped by the server, see section 6) |

### API compatibility

The server implements plugin API `0.1`. Compatibility follows semver rules for the two-number
`api` value: while the major version is `0`, the minor must match exactly (`0.1` plugins run on
an `0.1` server only); from `1.0` on, the major must match and the plugin's minor must not be
newer than the server's.

## 2. Packages

### `.rplugin` archive

A `.rplugin` is a zip file:

```text
rustic.plugin.toml      the manifest, at the root (required)
homes.wasm              the component named by plugin.entry (required)
config.toml             default configuration (optional, must be valid TOML)
resources/...           read-only files (optional): lang files, templates, schematics...
```

Anything else is ignored. Readers refuse unsafe entry names (absolute paths, `..`,
backslashes, drive letters) and cap sizes while decompressing: manifest 64 KiB,
`config.toml` 1 MiB, component 256 MiB, each resource 64 MiB, at most 10 000 entries.

`rustic build` writes archives with `RpluginBuilder` (deterministic: sorted entries, fixed
timestamps, deflate). `rusticmc_core::archive::pack_directory` packs a dev directory.

### Dev-mode directories

During development a plugin can be an unpacked directory in `plugins/` with the same layout
(manifest at its root). Any directory of `plugins/` containing `rustic.plugin.toml` is a
plugin; other directories (plugin data folders) are ignored. `/rustic reload <id>` re-reads the
directory, so rebuilding the `.wasm` and reloading is the edit loop.

Name dev directories differently from the plugin id (`plugins/homes-dev/`): `plugins/<id>/` is
the plugin's data folder. If a dev directory is named exactly `<id>`, its data folder becomes
`plugins/<id>/.data/` so data and sources never mix.

## 3. Discovery, ordering, refusal

At start-up the server scans `<server>/plugins/` (sorted by name):

1. Every `*.rplugin` and every directory with a manifest is opened and validated. Errors are
   logged with the path and listed by `/rustic plugins`.
2. **Duplicate ids**: if two packages declare the same id, *none* of them is loaded (which one
   was meant is ambiguous); the error names every path.
3. **Resolution** computes a load order:
   * a plugin whose `required` dependency is missing, has a version outside the requirement,
     or is itself refused, is refused (transitively), with a message naming the dependency;
   * plugins whose `required` dependencies form a **cycle** are all refused; the error names
     every member (`required dependency cycle between a, b, c`);
   * `optional` dependencies and `load_before` only order plugins that are present (an
     optional dependency with a non-matching version is ignored with a warning). A cycle made
     only of these soft links is broken with a warning instead of refusing anyone;
   * remaining ties are broken by id, so the order is deterministic.
4. Each plugin is **loaded** in order: instantiated by the host, then its `on-load` export
   runs (declare permissions and services there). An error from `on-load`, a trap or a missing
   dependency marks it failed.
5. Every loaded plugin is **enabled** in order: its `config.toml` is installed (section 5),
   then `on-enable` runs (register listeners, commands, tasks). An error disables it.

States: `discovered → validated → loaded → enabled → disabling → disabled`, or `failed` from
any state. `/rustic plugins` colours them; `/rustic info <id>` shows the reason of the last
failure or disable.

## 4. Disable, faults, reload

Disabling a plugin, whatever the cause, removes **everything** it registered:

* its event listeners, commands (bare names are handed to the next plugin that asked for them),
  sync and async tasks and pending `post-to-main` messages, declared permissions, provided
  services (consumers are re-linked to the next provider);
* what it owns in the game: open menus are closed, boss bars removed, plugin-message channels
  unregistered (when no other plugin uses them), chunk tickets released;
* its storage is flushed to disk.

Plugins that **require** it are disabled first, with the reason
`required dependency 'x' was disabled (...)`. `on-disable` runs before the cleanup, except after
a fault (the instance is not trusted any more).

A **fault** (a trap, running out of fuel, the epoch deadline, the memory limit, a host-side
panic) disables only that plugin (and its dependents), logs the reason with the guest
backtrace when available, and the server continues. A plugin that disables itself from its
own call (e.g. through a command) is disabled at once; its `on-disable` runs when the call
returns.

`/rustic reload <id>`: disable the plugin (dependents first), re-read the package from disk,
load and enable it, then enable the dependents again (re-instantiated). If the package cannot
be read any more the plugin stays failed with the reason.

`/rustic enable <id>` re-instantiates a disabled or failed plugin from its known package if its
required dependencies are enabled.

## 5. Data folder, configuration and storage

```text
<server>/plugins/<id>/                    data folder; preopened as "/" when
                                          capabilities.filesystem = "data-folder"
<server>/plugins/<id>/config.toml         the plugin's configuration
<server>/plugins/<id>/.rustic/kv.rkv      KV store (storage interface), managed by the server
<server>/plugins/<id>/.rustic/players/<uuid>.rkv   per-player data (players.get-data...)
```

Never write into `.rustic/` from the plugin: the server owns those files.

**Configuration.** On the first enable, the package's `config.toml` is copied to the data
folder if there is none; an existing file is never overwritten, so admins' edits survive
updates. Put a `config_version = <integer>` key at the top of your default config and bump it
when its structure changes; the SDK's config helper reads the admin's file, compares
`config_version` with the default's and calls your migration hook
(`fn migrate(from: u32, table: &mut toml::Table)`) before writing the upgraded file back. The
server itself does not interpret the config.

**KV store** (`storage` interface): keys 1 to 1024 bytes (no NUL), values up to 16 MiB, at
most `storage.max_kv_bytes` (64 MiB by default) of keys + values per plugin; a `set` over quota
is refused and logged. `keys(prefix)` returns sorted keys. Data lives in memory and is written
atomically (temporary file, fsync, rename) at most every `storage.flush_interval_ticks` ticks,
when the plugin is disabled and at shutdown. The file has a checksum; a corrupt file is refused
(the plugin's storage calls fail) rather than silently emptied.

**Per-player data** (`players.get-data/set-data/delete-data`): the same store, one file per
player and per plugin (namespaced: plugins never see each other's data), up to 1 MiB per player
per plugin. It is loaded on first use and written back and evicted from memory when the player
quits.

The async instance uses the same store (calls are thread-safe).

## 6. Server limits: `<server>/rustic.toml`

Created with defaults on first start. Every key is optional; unknown keys are errors.

```toml
[limits]
fuel_per_call = 50_000_000      # default fuel per call into a plugin
max_fuel_per_call = 500_000_000 # cap for a plugin's [limits].fuel_per_call
epoch_deadline_ms = 50          # wall-clock deadline of one call
memory_mb = 64                  # default memory limit
max_memory_mb = 512             # cap for a plugin's [limits].memory_mb

[scheduler]
max_task_ms_per_tick = 25       # sync task budget per tick; the rest rolls to the next tick
async_workers = 2               # threads of the async instance pool

[events]
move_threshold = 0.0625         # blocks a player must move before player-move fires

[watchdog]
seconds = 10                    # report calls running longer than this (0 = off)

[storage]
flush_interval_ticks = 20
max_kv_bytes = 67108864

[timings]
enabled = false                 # also /rustic timings on|off at runtime
```

A plugin's effective limits are its `[limits]` values (or the server defaults), capped by the
`max_*` values.

## 7. Runtime behaviour plugins can rely on

* **Events**: handlers run by priority (`lowest` → `monitor`), then in subscription order.
  Only the fields documented `mut` in `event-types` and `cancelled` are taken from what a
  handler returns; `monitor` handlers' changes are ignored; `ignore-cancelled` handlers are
  skipped for cancelled events. Invalid values (a non-finite or negative damage amount, a
  non-finite respawn location) are ignored. Subscribing or unsubscribing inside a handler
  takes effect from the next event. Custom events (`events.call-custom`, subscribe with
  `custom:<plugin-id>:<name>`) may be fired from handlers, nested up to 8 levels. An event is
  never delivered to a plugin that is currently executing (it would re-enter it): that
  handler is skipped with a one-time warning. `plugin-message` events only reach plugins that
  registered the channel. Clicks in a plugin's menu start cancelled.
* **Commands**: a command's usages form a tree; literals win over arguments when both match
  (`/home fail` runs the `fail` usage, not `<name>`), then the first declared usage. The first
  plugin to register a name gets `/name`; every command is also `/<plugin-id>:<name>`. A
  usage's permission replaces the command's for that usage. Argument kinds: `word`, `string`
  (quoted with `"` or `'`, escapes `\" \' \\ \n \t`), `greedy`, `integer`/`number` (bounds
  inclusive), `boolean` (`true`/`false`), `player` (name, UUID, `@p` `@r` `@s`, or `@a` when it
  matches exactly one), `players` (also `@a`), `position` and `block-pos` (`~` relative to a
  player sender), `item` (`diamond`, `mod:thing`, optional count `diamond 5`), `world`
  (`overworld` → `minecraft:overworld`, checked against loaded worlds), `duration` (`20t`,
  `10s`, `5m`, `2h`, `1d`, combinations `1h30m`, a bare number is ticks).
* **Scheduler**: `run-later(0)` and `run-later(1)` run next tick; timers run every `period`
  ticks (min 1) after the tick they last ran; due tasks run in task-id order within the tick
  budget, overflow rolls to the next tick. Async tasks and jobs run in the async instance;
  `post-to-main` messages are delivered at the start of the next tick.
* **Permissions**: the console has every permission. Otherwise: the permission provider plugin
  if one is installed (else the default `permissions.toml` engine), then declared parents
  (`children` of a node the player holds), then the node's declared default, then `op`.
  Wildcards `plugin.*` and `*` are understood by both engines.
