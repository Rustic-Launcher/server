# ADR 0040: Rustic Bridge runs Bukkit plugins in a supervised JVM sidecar

- Status: accepted
- Date: 2026-11-11

## Context

Bukkit plugins are JVM bytecode. Two ways to run them next to `rustic-server` were spiked and
measured with the same Java code and the same requests (`docs/BRIDGE.md` has every number and
how they were taken):

- **A, embedded JVM**: `JNI_CreateJavaVM` in the server process (crate `jni` 0.22, MIT/Apache,
  `invocation` feature, `-Xrs`), the bridge's dispatch called from the server thread
  (`bridge/spikes/jni`).
- **B, sidecar**: a child `java` process, length-prefixed JSON frames over loopback TCP
  (`bridge/rusticmc-bridge/examples/measure.rs`).

Results on the dev machine (Windows 11, JDK 25, release builds): a chat event with one Bukkit
listener costs 8.7 µs (p50) embedded and 70 µs in the sidecar; a move event 10.5 vs 81 µs; the
per-tick pulse 2.4 vs 50 µs; both start in about 0.5 s and use about 100 MiB for the JVM. The
sidecar is about 8x slower per call, with a floor of one loopback round trip (38 µs here).

What the numbers do not show decides: AGENTS.md requires that **a plugin can never crash or hang
the server**. Embedded, a Bukkit plugin runs on the server thread inside the server process:

- an infinite loop in a listener hangs the server thread, and Java cannot stop a thread
  (`Thread.stop` is gone since JDK 20), so there is no recovery short of killing the server;
- `System.exit`/`Runtime.halt` through reflection or JNI, a native crash in a shaded library
  (sqlite-jdbc, netty natives), `OutOfMemoryError` or a JVM bug kill the whole server;
- the JVM's signal handlers, GC threads and heap share the server's address space.

In the sidecar each of these is a dead or silent child process: the host's read times out or
fails, the JVM is killed, the bridge and its plugins are disabled, the server goes on, and
`rustic reload bukkit-bridge` starts a fresh JVM. The spike proved it end to end (a plugin
looping forever, `System.exit`, exceptions; `docs/BRIDGE.md`).

## Decision

- Rustic Bridge uses **model B**: one JVM sidecar per server, started by the `bukkit-bridge`
  plugin when it loads, stopped when it is disabled, killed when it does not answer within
  `call_timeout_ms` (default 30 s; the JVM logs the stack of a call running longer than
  `watchdog_ms`, default 3 s).
- The JVM runs `-Xrs -XX:+ExitOnOutOfMemoryError --enable-native-access=ALL-UNNAMED` and the
  configured `jvm_args` (default `-Xmx1G`), in its own process group (Ctrl+C on the console goes
  to the server, which stops the bridge cleanly).
- The cost is accepted and bounded: Bukkit listeners are only called for events they subscribed
  to (the core never calls a plugin that does not listen), so the price is paid per event *a Java
  plugin wants*. Known follow-ups if it matters: binary encoding of hot events, AF_UNIX on Linux
  (lower loopback cost than Windows TCP), and one call per event for all Java plugins instead of
  one per plugin and priority (needs a core hook to dispatch a group of instances at once).

## Consequences

- A high-frequency event with many Java listeners is expensive: 10 Bukkit plugins on `player-move`
  cost about 0.8 ms per move. Server owners should prefer native (WebAssembly) plugins for hot
  paths; `docs/guides/bukkit-bridge.md` says so.
- Memory: about 100 MiB of JVM regardless of plugin count (10 small plugins: +4 MiB).
- Off-thread API calls from plugins are marshalled to the next tick pulse (ADR 0042), which a
  sidecar needs anyway (only the main thread talks to the host).
- `System.exit` in plugin bytecode is refused (ADR 0042) rather than relied on the process boundary.

## Alternatives considered

- **A, embedded JVM**: 8x lower latency, rejected for isolation (above) and licensing (ADR 0009).
- **One JVM per plugin**: stronger isolation, but ~100 MiB per plugin, and Bukkit plugins call
  each other's classes and services in the same JVM (Vault, PlaceholderAPI); rejected.
- **GraalVM native image / Espresso**: no dynamic class loading of arbitrary plugin jars; no.
