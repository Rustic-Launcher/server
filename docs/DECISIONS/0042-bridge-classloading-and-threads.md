# ADR 0042: Rustic Bridge class loading, plugin threads and System.exit

- Status: accepted
- Date: 2026-11-11

## Context

Bukkit plugins expect: one class loader per plugin (shaded libraries do not clash), access to
their dependencies' classes, `getResource`/`saveDefaultConfig` from their jar, the API's
`JavaPlugin` constructor contract, and a main thread where the game is touched, with
`BukkitScheduler` to get there from async code. Modern Java (17+, removed in 24) has no
SecurityManager, so nothing stops a plugin from calling `System.exit`.

## Decision

- **Class loaders.** Each jar gets a `PluginClassLoader` (a `URLClassLoader` over the jar that
  implements the API's `ConfiguredPluginClassLoader`, which `JavaPlugin`'s constructor requires,
  and calls the API's own `JavaPlugin.init`). Delegation: parent first (JDK, the bridge, the Paper
  API and its libraries: Adventure, Gson, Guava, SnakeYAML...), then the jar, then the classes of
  the other loaded plugins (as Bukkit does for dependencies). Two plugins shading the same class
  each see their own copy (tested: `rustictest.shared.Lib` in Welcome and RusticCommands).
- **API services.** The bridge provides the `ServiceLoader` services the API looks up:
  `io.papermc.paper.InternalAPIBridge` (lifecycle manager for `JavaPlugin`; Brigadier lifecycle
  registrations are accepted and logged as unsupported), `io.papermc.paper.ServerBuildInfo`,
  `org.bukkit.plugin.PluginLoader`.
- **System.exit guard.** Plugin classes are defined from bytes rewritten with the JDK 25
  class-file API (`java.lang.classfile`, no agent, no dependency): `invokestatic
  System.exit(I)V`, `invokevirtual Runtime.exit(I)V` and `Runtime.halt(I)V` become calls to
  `rustic.bridge.Guard`, which throws `SecurityException` (what the SecurityManager used to do).
  Only classes whose constant pool names these methods are rewritten. Reflection and JNI can still
  reach `exit`; then the JVM dies and the sidecar's isolation takes over (ADR 0040).
- **Threads.** The thread serving the host is the Bukkit main thread (`isPrimaryThread`). A
  plugin's async task or own thread calling the API is marshalled: calls without a result
  (`sendMessage`, `broadcast`, setters) are queued; calls with a result wait (up to 10 s) for the
  main thread, which drains the queue at the next tick pulse and after each host request. So
  `Bukkit.getOnlinePlayers()` (served from the JVM's player mirror) is free from any thread and
  `player.getHealth()` from an async thread costs at most one tick (measured 5-6 ms in the
  integration test, which pulses every 5 ms; up to 50 ms on a server).
- **Scheduler.** `BukkitScheduler` runs in the JVM: sync tasks in the tick pulse in scheduling
  order, async tasks on a 4-thread pool (ticks are 50 ms), all tasks of a plugin cancelled when it
  is disabled. Per-plugin task timings are therefore inside the `bukkit-bridge` plugin's tick in
  `/rustic timings`.
- **Exceptions** in listeners, commands and tasks are logged with the plugin's name and do not
  disable the plugin (Bukkit's behaviour); an exception in `onEnable` fails the enable.

## Consequences

- JDBC with a shaded driver works when the plugin loads the driver class
  (`Class.forName("org.sqlite.JDBC")`), as on Paper; the bridge does not scan plugin jars for
  `java.sql.Driver` services.
- Plugins see a Bukkit-like environment but not a Paper server: Brigadier lifecycle commands,
  NMS/CraftBukkit, and packet libraries are absent (`docs/BUKKIT_MAPPING.md`).

## Alternatives considered

- **A Java agent** with `Instrumentation` for the exit guard: needs `-javaagent` and an extra jar;
  the class loader already owns every plugin class's bytes.
- **Child-first class loading**: breaks plugins that expect the server's Adventure/Gson; Paper is
  parent-first too.
- **Rust-side scheduling of each Bukkit task** (`scheduler.run-later` per task): per-plugin
  timings, but one IPC round trip per scheduled task and per run; rejected for the pulse.
