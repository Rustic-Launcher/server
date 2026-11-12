# Rustic Bridge: Bukkit/Paper plugins on Rustic

Rustic Bridge runs existing Bukkit/Paper plugin jars, unmodified, on `rustic-server`. Each jar of
`plugins/bukkit/` becomes an ordinary Rustic plugin (`bukkit-<name>` in `rustic plugins`) with its
plugin.yml commands and permissions, its event listeners at their priorities, its scheduler tasks
and its config. Admin guide: [guides/bukkit-bridge.md](guides/bukkit-bridge.md). What the API
covers: [BUKKIT_MAPPING.md](BUKKIT_MAPPING.md#running-unchanged-plugins-rustic-bridge). Decisions:
ADRs [0009](DECISIONS/0009-bridge-licensing-and-isolation.md) (licensing),
[0040](DECISIONS/0040-bridge-jvm-sidecar.md) (execution model),
[0041](DECISIONS/0041-bridge-ipc-protocol.md) (IPC),
[0042](DECISIONS/0042-bridge-classloading-and-threads.md) (class loading, threads, `System.exit`).
Which popular plugins could run at all: [BRIDGE_ANALYSIS.md](BRIDGE_ANALYSIS.md).

## Architecture

```
rustic-server (MIT/Apache, feature `bridge`, default)         JVM sidecar (GPL-3.0-only, bridge/java)
  rusticmc-core: plugins, events, commands, permissions         rustic.bridge.Main (the "main thread")
  rusticmc-bridge (bridge/rusticmc-bridge)                        Bukkit.getServer() = ServerProxy
    BridgeLoader -> bukkit-bridge  (BridgeInstance) ---tick-->      Scheduler (BukkitScheduler, Folia schedulers)
                 -> bukkit-<name>  (JavaPluginInstance) -event->    PluginManagerProxy (HandlerList, @EventHandler)
                                                       -command->   PluginCommand / CommandExecutor
    host.rs  <-- player/world/server/permission calls ----------    PlayerProxy, World/Block, Console...
    sidecar.rs: spawn, token handshake, log forwarding              PluginClassLoader per jar (+ System.exit guard)
    link.rs: u32 length | u8 kind | u32 id | JSON, nested calls     Paper API jar + libs (fetched, not committed)
                         loopback TCP
```

- **The core sees one plugin per jar** and one more, `bukkit-bridge`, that all Bukkit plugins
  depend on. Its instance starts the JVM on load (`hello`: server facts, online players), sends one
  `tick` pulse per server tick (the Java scheduler runs then), keeps the JVM's online-player mirror
  (subscribed to `player-join` at LOWEST and `player-quit` at MONITOR), and stops the JVM on
  disable. Because of the dependency, losing the JVM disables every Bukkit plugin through the
  core's normal cascade; `rustic reload bukkit-bridge` starts a new JVM and re-enables them.
- **Events**: when a Bukkit plugin registers a listener for a bridged event class, it is
  subscribed in the core to the matching Rustic event at the same priority. The core then calls the
  plugin's instance once per (plugin, priority); the bridge builds the Bukkit event(s) from the
  record, runs that plugin's listeners at that priority (via the API's own `HandlerList` /
  `RegisteredListener`, so `ignoreCancelled` behaves as on Bukkit), and returns the record with
  the mutable fields and `cancelled`. Bukkit and WebAssembly plugins therefore interleave by
  priority on the same event.
- **Commands**: plugin.yml commands are registered in the core (name, aliases, description,
  permission; a bare usage and a greedy `args` usage with dynamic completion) and run through the
  plugin's `PluginCommand` (executor, usage message on `false`, tab completer).
- **Permissions**: plugin.yml `permissions:` are declared in the core with their defaults and
  children; `hasPermission` asks the core (the Rustic permission engine or a provider plugin).
- **Threads**: the thread serving the host is the Bukkit main thread; API calls from other threads
  are marshalled to it (ADR 0042).

## Spike: embedded JVM (JNI) vs sidecar, measured

Both models run the *same* Java code (the bridge's dispatch, the Probe test plugin) with the same
requests. Machine: Windows 11, x86-64, JDK 25.0.5 (Temurin), Rust 1.99, release builds,
20 000 events and 2 000 ticks per row, after warm-up. Commands:
`cargo run --release -p rusticmc-bridge --example measure` (sidecar) and
`CARGO_TARGET_DIR=target cargo run --release --manifest-path bridge/spikes/jni/Cargo.toml` (JNI).

| Measure | A: embedded JVM (JNI, `jni` 0.22) | B: sidecar (loopback TCP) |
|---|---:|---:|
| `player-chat` (cancellable, 1 Bukkit listener; every other message cancelled), p50 / p99 | 8.7 / 87.7 µs | 70.1 / 196.4 µs |
| `player-move` (1 listener), p50 / p99 | 10.5 / 75.9 µs | 80.8 / 189.9 µs |
| `player-chat`, 10 Bukkit plugins listening, p50 / p99 | (not run) | 662.8 / 1 281 µs |
| `player-move`, 10 plugins listening, p50 / p99 | (not run) | 819.9 / 1 500 µs |
| raw round trip (`ping`), p50 / p99 | 2.2 / 6.6 µs | 38.3 / 105.3 µs |
| per tick, 0 Bukkit plugins (no JVM), p50 / p99 | 0.1 / 0.2 µs | 0.1 / 0.2 µs |
| per tick, 1 plugin with a 1-tick repeating task, p50 / p99 | 2.4 / 34.9 µs | 50.3 / 186.9 µs |
| per tick, 10 plugins each with a 1-tick task, p50 / p99 | (not run) | 51.0 / 189.0 µs |
| startup: JVM ready | 68 ms (JVM created) | 169 ms (spawn to handshake; 3.1 s on a cold disk cache) |
| startup: plugins enabled | 0.51 s (1 plugin) | 0.52 s (1 plugin), 0.79 s (10 plugins) |
| memory (RSS) | 97.6 MiB whole process (server process grows by ~92 MiB) | JVM 104.9 MiB (1 plugin), 109.3 MiB (10); Rust side +2 MiB |
| plugin calls the API from its own thread (`getHealth` from a `new Thread`) | marshalled the same way | answered at the next tick pulse: 2-6 ms in tests (pulse every 5 ms), 51 ms worst case on the server (pulse every 50 ms) |
| plugin uses SQLite through a shaded driver (sqlite-jdbc 3.50.3.0 in its jar) | same JVM, works | works (`sqlite: 1 row(s) in probe.db (driver 3.50.3.0)`), needs `--enable-native-access=ALL-UNNAMED` to stay quiet |
| two plugins shade a class with the same name | isolated (one loader per jar) | isolated: `welcome-copy` / `commands-copy` |
| plugin throws in a listener/command | logged, plugin stays enabled (Bukkit behaviour) | same |
| plugin calls `System.exit(3)` | refused by the bytecode guard; via reflection: **the server dies** | refused by the guard (`SecurityException`); via reflection: the JVM dies, the bridge is disabled, the server goes on |
| plugin loops forever on the main thread | **the server thread hangs, no recovery** (`Thread.stop` is gone) | after `call_timeout_ms` (5 s in the test) the JVM is killed, Bukkit plugins disabled, the server goes on; `rustic reload bukkit-bridge` brings them back |
| JVM crash / OOM | takes the server down | `ExitOnOutOfMemoryError`: the child exits; same recovery as above |

Notes: 10-plugin rows exist for the sidecar only (JNI was measured with one plugin; its per-call
cost scales the same way, one call per listening plugin). The sidecar's floor is the loopback round
trip, which is slow on Windows; Linux loopback (and an AF_UNIX fast path, ADR 0041) is typically
several times cheaper. Both models spend the same ~6-8 µs in Java per event (JSON both ways,
building the Bukkit event, running the listener).

### Recommendation

**Sidecar (model B), as implemented.** The embedded JVM is about 8x cheaper per call, but it puts
plugin code on the server thread inside the server process: an infinite loop, a reflective
`System.exit`, a native crash in a shaded library or an OOM ends or freezes the server, which
AGENTS.md rules out. The data say the sidecar costs about 60-70 µs per event per listening Bukkit
plugin and ~50 µs per tick: invisible for chat, joins, commands and blocks (a 50 ms tick budget),
noticeable for `player-move` with many Java listeners (10 plugins x 20 moves/s x 100 players would
be 1.6 s of work per second). Server owners are told to keep hot paths in native plugins
(guide), and the follow-ups (binary encoding for hot events, AF_UNIX on Linux, one call per event
for all Java plugins) are listed in ADR 0040.

## The demo: two plugins, unmodified

Test plugins (original code, GPL-3.0-only, `bridge/java/test-plugins/`), compiled against the
Paper API as normal plugin jars by `bridge/java/build.sh`:

- **Welcome** (`Welcome.jar`): `config.yml` (join/quit/private/delayed messages, blocked word),
  `saveDefaultConfig`, `PlayerJoinEvent` (rewrites the join line, greets privately, schedules a
  `runTaskLater` greeting 40 ticks later), `PlayerQuitEvent` at MONITOR, `AsyncPlayerChatEvent`
  with `ignoreCancelled` (cancels the blocked word), `/welcome [reload]` with permissions
  `welcome.use` (default true) and `welcome.reload` (op), visitor count saved in `onDisable`.
- **RusticCommands** (`RusticCommands.jar`): `/heal`, `/fly`, `/broadcast` (alias `/bc`; builds
  the line on `runTaskAsynchronously`, then a `BukkitRunnable` timer counts down 3..1 on the main
  thread and broadcasts), op-default permissions.
- **Probe** (`Probe.jar`, the spike's probe): `/probe throw|exit|halt|loop|async|sqlite`, a chat
  listener (cancels "cancelme"), a move listener, a 1-tick timer; shades sqlite-jdbc.

### (a) Integration test with a recording backend

`cargo test -p rusticmc-bridge --test bukkit_plugins` runs the three jars through a real JVM and
the real core over a recording `Backend` (`bridge/rusticmc-bridge/tests/bukkit_plugins.rs`): ids
`bukkit-bridge`, `bukkit-welcome`, `bukkit-rusticcommands`, `bukkit-probe` enabled; join line
`Welcome, Steve!`; private greeting; delayed greeting after 40 ticks; chat cancelled;
`/welcome reload`, `/heal` (health 20, food 20), `/fly`, `/bc hello world` (broadcast
`[CONSOLE] hello world` after the countdown); SQLite; async marshalling; `System.exit` and an
exception contained; a stuck plugin kills the JVM in 4 s, the core disables the Bukkit plugins,
`reload bukkit-bridge` + `enable bukkit-welcome` bring Welcome back; shutdown runs `onDisable`.
Excerpt:

```
INFO  rustic: [bukkit-bridge] JVM started (pid 30460) in 158 ms
INFO  rustic: [bukkit-bridge] Java 25.0.5 ready (bridge 0.1.0)
INFO  bukkit: [Probe] guarded System.exit/Runtime.exit/halt calls in rustictest.probe.ProbePlugin
INFO  bukkit: [RusticCommands] RusticCommands enabled, bundled lib commands-copy
INFO  bukkit: [Welcome] Welcome enabled: join message '&6Welcome, &e{player}&6!', bundled lib welcome-copy
INFO  bukkit: [Welcome] delayed greeting sent to Steve
INFO  bukkit: [Welcome] blocked a message from Steve
INFO  bukkit: [RusticCommands] broadcast in 1...
INFO  bukkit: [Probe] sqlite rows 1
ERROR bukkit: [Rustic Bridge] a plugin called System.exit(3); refused
WARN  bukkit: [Rustic Bridge Watchdog] the main thread has been busy for 1470 ms in `command (Probe)`; a plugin may be stuck.
ERROR rustic: [bukkit-bridge] the JVM did not answer within 4000 ms; the JVM (pid 51896) is stopped and the Bukkit plugins are disabled (`rustic reload bukkit-bridge` starts it again)
WARN  rustic: [bukkit-welcome] disabling: required dependency `bukkit-bridge` was disabled (...)
INFO  rustic: [bukkit-bridge] JVM started (pid 52596) in 182 ms
INFO  rustic: reloaded bukkit-bridge
test bukkit_plugins_run_unmodified ... ok
```

The Java layer has its own unit tests without Rust (`bridge/java/src/test`, run by the build):
JSON (u64 UUID halves), component conversion (legacy codes, the server's modern hover events),
and the three plugins driven through `Host.handle` with a fake host.

### (b) On rustic-server with a bot

`bridge/java/e2e.sh` (recipe below) starts `rustic-server` (feature `bridge`, on by default) in
`.local/rustic-bridge-e2e/`, a `horizon-bots` bot joins, chats and runs the commands (raw
`ChatCommand`/`Chat` packets), the console ops it halfway (so the first `/heal` is refused by its
permission node), then checks recovery. Server log (`logs/latest.log`, 2026-11-11 07:21, kept as
`.local/rustic-bridge-e2e-evidence.log`):

```
[07:21:02] [Server thread/INFO]: [rustic] [bukkit-bridge] JVM started (pid 52956) in 178 ms
[07:21:02] [Server thread/INFO]: [rustic] Rustic ready: 4 plugin(s) enabled (4 loaded, 0 failed)
[07:21:04] [Server thread/INFO]: System chat: Plugins (4): bukkit-bridge, bukkit-probe, bukkit-rusticcommands, bukkit-welcome
[07:21:05] [Server thread/INFO]: System chat: Welcome, BridgeBot0!
[07:21:06] [bridge-jvm-stdout/INFO]: [bukkit] [Welcome] delayed greeting sent to BridgeBot0
[07:21:08] [Server thread/INFO]: [Not Secure] <BridgeBot0> hello from a bot
[07:21:09] [bridge-jvm-stdout/INFO]: [bukkit] [Welcome] blocked a message from BridgeBot0
[07:21:15] [bridge-jvm-stdout/INFO]: [bukkit] [Welcome] configuration reloaded by BridgeBot0
[07:21:17] [bridge-jvm-stdout/INFO]: [bukkit] [RusticCommands] broadcast in 1...
[07:21:19] [bridge-jvm-stdout/INFO]: [bukkit] [Probe] sqlite rows 1
[07:21:21] [bridge-jvm-stdout/ERROR]: [bukkit] [Rustic Bridge] a plugin called System.exit(3); refused
[07:21:23] [Server thread/INFO]: [Not Secure] <BridgeBot0> still alive
[07:21:27] [bridge-jvm-stdout/WARN]: [bukkit] [Rustic Bridge Watchdog] the main thread has been busy for 3026 ms in `command (Probe)`; a plugin may be stuck. Stack:
[07:21:29] [Server thread/ERROR]: [rustic] [bukkit-bridge] the JVM did not answer within 5000 ms; the JVM (pid 52956) is stopped and the Bukkit plugins are disabled (`rustic reload bukkit-bridge` starts it again)
[07:21:29] [Server thread/WARN]: [rustic] [bukkit-welcome] disabling: required dependency `bukkit-bridge` was disabled (timeout in task: ...)
[07:21:33] [Server thread/INFO]: [Not Secure] <BridgeBot0> the server survived a stuck plugin
[07:21:38] [Server thread/INFO]: [rustic] [bukkit-bridge] JVM started (pid 34884) in 193 ms
[07:21:38] [Server thread/INFO]: System chat: Reloaded: bukkit-bridge
[07:21:41] [Server thread/INFO]: System chat: Enabled: bukkit-welcome
[07:21:43] [Server thread/INFO]: [rustic] [bukkit-bridge] JVM stopped
```

What the bot received (system chat packets decoded from its `--capture`, seconds after joining):

```
   1.98s SystemChat  aqua | Enjoy your stay, BridgeBot0!                         <- runTaskLater, 40 ticks
   4.78s SystemChat  red | Please do not say 'badword'.                          <- AsyncPlayerChatEvent cancelled
   6.28s SystemChat  gold | Join message:  | gold | Welcome,  | yellow | {player} <- /welcome
   7.28s SystemChat  red | command.unknown.command ... /heal                     <- not op yet: rusticcmd.heal hides it
   8.42s SystemChat  gray | chat.type.admin | Server | commands.op.success | BridgeBot0
   9.78s SystemChat  green | You have been healed.                               <- /heal
  10.28s SystemChat  aqua | Flight enabled.                                      <- /fly
  10.79s SystemChat  green | Welcome configuration reloaded.                     <- /welcome reload (op-only node)
  11.78s SystemChat  gray | Broadcast scheduled.                                 <- /bc hello everyone
  13.28s SystemChat  light_purple | [BridgeBot0]  | white | hello everyone       <- async task, then the 3..1 timer
  14.47s SystemChat  sqlite: 1 row(s) in probe.db (driver 3.50.3.0)              <- shaded JDBC driver
  15.33s SystemChat  async thread: 1 online, health 20.0, round trip through the main thread 2 ms
  16.29s SystemChat  red | An internal error occurred while attempting to perform this command   <- System.exit refused
  17.28s SystemChat  red | An internal error occurred while attempting to perform this command   <- exception
  24.33s SystemChat  red | An internal error occurred in plugin `bukkit-probe` while running this command.  <- stuck: JVM killed
```

### `rustic bridge analyze` on the test plugins

```
| Plugin | Version | Verdict | API members (covered / used) | Uncovered members |
| Welcome | 1.2.0 | 🟢 | 36 / 36 | 0 |
| RusticCommands | 0.3.1 | 🟡 | 24 / 25 | 1 | o.b.entity.Player#getAllowFlight
API coverage over the panel: 51 of 52 distinct API members used (98%), 98.9% of the 90 call sites
```

The analyzer measures coverage by `rustic:api` (the WIT); `Player#getAllowFlight` has no WIT getter
but the bridge answers it from the flag it set, so RusticCommands runs fully.

### Real plugins from the panel (`bridge/panel/jars`, local only)

`cargo run -p rusticmc-bridge --example try_jars -- <jars>` (loads, enables, fires a join and a
chat, runs every plugin.yml command as the console, ticks 2 s):

| Plugin (verdict) | Result |
|---|---|
| ChestSort 1.1 (🟡) | enabled; its `InventoryClickEvent` listener is not called (event not bridged, warned at registration) |
| SetHome 6.3 (🟡) | enabled; all 6 commands run (`/admindelhome` prints its usage, player commands refuse the console) |
| VaultUnlocked 2.20.3 (🟡) | enabled (after adding `Server#getPluginsFolder`); `PluginEnableEvent` fired Java-side |
| InstantRestock 2.6.7 (🟡) | enabled (after adding Paper's region/global/async schedulers); 2 villager events not bridged; `instantrestock.*` refused as a declared node (Rustic forbids declaring wildcards) |
| MiniMOTD 2.2.5 (🟡) | disables itself: it checks for "Paper 1.21.8+" and does not recognise the bridge |
| BlueMap 5.29 (🟡) | fails in `onEnable` (spigot build on a "Paper-like" server; needs world folders) |

## Building, testing, running

```sh
bridge/java/build.sh            # or: powershell -File bridge\java\build.ps1 ; JDK 25 (tools/jdk25)
cargo test -p rusticmc-bridge   # Rust unit tests + the integration test (needs the Java build)
CARGO_TARGET_DIR=$PWD/horizon/target cargo build --manifest-path crates/rusticmc-server/Cargo.toml --profile fast
bridge/java/e2e.sh              # the bot test above (needs horizon-bots and the prepared resources)
```

The build fetches the Paper API and its libraries from `repo.papermc.io` with SHA-256 checks
(`bridge/java/deps.txt`), compiles with `javac --release 25`, runs the Java unit tests, and leaves
`build/rustic-bridge.jar`, `build/libs/` (the runtime classpath) and `build/plugins/` (the test
plugins). No Gradle or Maven is needed (Maven was used once, in a scratch folder, to resolve the
dependency list); the scripts are the reproducible build. Deploy `rustic-bridge.jar` + `libs/` as
`<server>/bridge/`, or point `home` in `rustic-bridge.toml` at them.

## Limitations (0.1)

- **Events bridged**: join, quit, chat (`AsyncPlayerChatEvent`, Paper `AsyncChatEvent`), move,
  teleport (not cancellable), command preprocess, server command, block break, game-mode change,
  respawn, pre-login, changed world; plugin enable/disable fire Java-side; plugin-defined events
  work through `callEvent` among Java plugins. Not yet: block place, interact, damage, death,
  inventory, entity events (listeners register and are warned about).
- **API**: players, console, server, plugin manager, schedulers (Bukkit and Paper's
  global/region/async), services manager, worlds (name, key, time, spawn, players, highest block),
  blocks (type, set type) are implemented; inventories, items beyond the API's own classes,
  entities other than players, scoreboards, boss bars, Brigadier lifecycle commands, plugin
  messaging, metadata/PDC and the rest throw `UnsupportedOperationException("Rustic Bridge: <Type>.<method> not supported yet")`.
- Ids are `bukkit-<name>` (the core's plugin ids allow `[a-z0-9_-]` only, no `:`).
- A message sent to a player inside `PlayerJoinEvent` did not reach the bot on the server (the
  delayed one did): handed to the adapter owner (`docs/HANDOFF.md`).
- `onDisable` does not run when the JVM is killed (stuck plugin, crash).
- Per-task timings of Bukkit plugins are inside the `bukkit-bridge` plugin's tick.
- No `libraries:` download (plugin.yml), no `paper-plugin.yml`-only plugins, no NMS/CraftBukkit.
