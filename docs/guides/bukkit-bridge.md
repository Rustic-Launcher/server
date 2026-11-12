# Running Bukkit/Paper plugins on Rustic (Rustic Bridge)

This guide is for server admins. Rustic Bridge runs ordinary Bukkit/Paper plugin jars next to
Rustic's own (WebAssembly) plugins. It is optional, and it is a compatibility layer: it covers the
everyday API (players, commands, chat and join events, the scheduler, config files, permissions)
but not server internals. Check a plugin first with `rustic bridge analyze`.

## 1. Before you start: will the plugin work?

```sh
rustic bridge analyze path/to/SomePlugin.jar
```

- 🟢 the plugin only uses what Rustic covers;
- 🟡 it uses some API Rustic does not cover yet: it may still load, and fail only where it
  touches the missing part (look at the "uncovered members");
- 🔴 it touches server internals (NMS, CraftBukkit, ProtocolLib, Netty...): it will not work.

Of 55 popular plugins analysed (docs/BRIDGE_ANALYSIS.md) none is green and 12 are yellow; plugins
built for Paper's newest "paper-plugin.yml" format, packet libraries, world editors and map
renderers are out of reach. Simple gameplay, chat, command and utility plugins are the target.

## 2. What you need

- `rustic-server` built with the bridge (cargo feature `bridge`, on by default; see
  `crates/rusticmc-server/README.md`).
- Java 25 (any JDK or JRE 25). The bridge starts it as a separate process.
- The bridge itself: `rustic-bridge.jar` and its `libs/` folder (`bridge/java/build.sh` makes them
  in `bridge/java/build/`). Copy both into `<server>/bridge/`.

## 3. Install plugins

```
server/
  rustic-bridge.toml          (optional, see below)
  bridge/rustic-bridge.jar
  bridge/libs/*.jar
  plugins/bukkit/Welcome.jar  <- Bukkit plugins go here, not in plugins/
```

Start the server. The log shows the JVM starting and each plugin loading:

```
[rustic] [bukkit-bridge] JVM started (pid 8224) in 257 ms
[rustic] [bukkit-welcome] enabled Welcome v1.2.0
[rustic] Rustic ready: 4 plugin(s) enabled (4 loaded, 0 failed)
```

The plugins' own log lines (`getLogger()`, `System.out`) are in `logs/latest.log` as
`[bukkit] [PluginName] ...`.

Each jar is a Rustic plugin named `bukkit-<name in lower case>`: `rustic plugins`,
`rustic info bukkit-welcome`, `rustic disable|enable|reload bukkit-welcome` work as for any plugin.
Its data folder is `plugins/bukkit-<name>/` (its `config.yml` lands there on first start).
`bukkit-bridge` is the bridge itself: every Bukkit plugin depends on it.

## 4. Configuration (`rustic-bridge.toml`, next to `rustic.toml`)

```toml
java = "/opt/jdk-25/bin/java"   # default: $RUSTIC_BRIDGE_JAVA, $JAVA_HOME/bin/java, then `java`
home = "bridge"                 # where rustic-bridge.jar and libs/ are
plugins = "plugins/bukkit"
jvm_args = ["-Xmx1G"]           # the JVM's heap: ~100 MiB is used with a few small plugins
call_timeout_ms = 30000         # a plugin call longer than this is considered stuck
watchdog_ms = 3000              # log the stack of a call running longer than this
online_mode = true              # what plugins see from Server#getOnlineMode
enabled = true
```

## 5. Permissions

`plugin.yml` permissions are declared with their defaults (`true`, `false`, `op`, `not op`).
Grant or deny them in `permissions.toml` like any Rustic permission, or with a Rustic permission
plugin. Commands whose permission a player lacks are hidden from that player, like on Paper.

## 6. When a plugin misbehaves

- **It throws an exception**: logged with the plugin's name (`Could not pass event ...`,
  `Unhandled exception executing command ...`); the plugin stays enabled, as on Paper; the player
  sees "An internal error occurred".
- **It calls `System.exit`**: refused (`a plugin called System.exit(3); refused`).
- **It hangs** (an endless loop): after 3 s the log shows the plugin's stack trace
  (`[Rustic Bridge Watchdog] the main thread has been busy for ...`); after `call_timeout_ms`
  the JVM is stopped and **all Bukkit plugins are disabled**; the server and the WebAssembly
  plugins go on. Fix or remove the plugin, then `rustic reload bukkit-bridge` (and
  `rustic enable bukkit-<name>` for the others).
- **The JVM crashes or runs out of memory**: same as a hang.

A Bukkit plugin can never take the server down, but one stuck plugin stops all Bukkit plugins
until you reload the bridge.

## 7. Limits to know

- Performance: each event a Bukkit plugin listens to costs one round trip to the JVM (about
  70 µs on Windows, less on Linux) per listening plugin. Fine for chat, commands, joins, blocks;
  avoid many Bukkit plugins listening to `PlayerMoveEvent` on a busy server: prefer a native
  Rustic plugin for hot paths.
- Events not bridged yet (inventory clicks, damage, deaths, block place, interact...) are
  announced at startup: `listens to InventoryClickEvent, which the Rustic Bridge does not fire
  yet; that handler will not be called`.
- Methods not supported yet throw `UnsupportedOperationException("Rustic Bridge: <Type>.<method>
  not supported yet")`; the plugin sees that exception where it calls them.
- `onDisable` is not called when the JVM had to be killed.
- Plugin messaging, scoreboards, boss bars, inventories and item metadata are not available to
  Bukkit plugins yet (they are to Rustic plugins).
- The full list: `docs/BUKKIT_MAPPING.md`, section "Running unchanged plugins".

## 8. Licence

The bridge's Java part is GPL-3.0-only (it implements the GPL Bukkit API) and is a separate
program from the MIT/Apache Rustic server; distributing `rustic-bridge.jar` means honouring the GPL
for it (its source is `bridge/java/`). See ADR 0009.
