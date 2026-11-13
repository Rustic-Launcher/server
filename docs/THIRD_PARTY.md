# Third parties: inspirations and dependencies

Rustic copies **no code** from Bukkit, Spigot, Paper, CraftBukkit, Glowstone, MockBukkit,
LuckPerms, Vault or EssentialsX. The *concepts* below inspired its design and were reimplemented
from scratch in Rust, with their own names and semantics where those differ.

## Concepts and where they come from

| Concept in Rustic | Inspired by | What we kept / changed |
|---|---|---|
| Event priorities LOWEST..MONITOR, `ignore_cancelled`, MONITOR read-only | Bukkit `EventPriority`, `@EventHandler` | Same ordering idea; events are WIT records, handlers return the changed record and the host applies only the mutable fields. |
| Plugin manifest with depend/softdepend/loadbefore | Bukkit `plugin.yml` | TOML, semver ranges on dependencies, capabilities (sandbox permissions). |
| Scheduler sync/async tasks | Bukkit `BukkitScheduler` | Async tasks run in a separate WASM instance with no world access (no shared memory). |
| Patch-based fork of an upstream server | Paper's paperweight over Spigot | `patches/` + `scripts/patches.py`; most hooks come from Horizon's own trait-mod API instead. |
| Services registry | Bukkit `ServicesManager`, Vault | Services are WIT interfaces exported by a provider component, linked by the host. |
| Permission nodes, wildcards, defaults (true/false/op/not-op) | Bukkit permissions, LuckPerms | Default engine is a small TOML file engine; a plugin can replace it as a service. |
| Timings report | Spigot/Paper timings, spark | Per plugin, per event, per task; text and JSON. |
| Test harness with a fake server | MockBukkit | In-memory `Backend` driving real WASM plugins. |
| Plugin messaging channels | Bukkit `Messenger` | WIT `messaging` interface over custom payload packets. |
| Command tree with typed arguments | Mojang's Brigadier (as described by its public docs), Paper's command API | Built from usages; sent to the client through Horizon's command tree. |
| Bukkit API compatibility layer | Glowstone (showed that reimplementing the API on another server is feasible) | Separate GPLv3 Java layer over IPC; no Glowstone code. |

## Runtime dependencies

The full list with licences is produced by `cargo deny list` and checked in CI (`deny.toml`).
Notable choices are recorded in ADRs: wasmtime (ADR 0002), core crates (ADR 0003), plugin format
and storage (ADR 0005), guest SDK (ADR 0007), bridge analyzer (ADR 0008), bridge runtime (ADR 0009).

### `rusticmc-host` (ADRs 0020-0023)

| crate | version | licence | why |
|---|---|---|---|
| wasmtime | 50.0.1 | Apache-2.0 WITH LLVM-exception | component runtime, Cranelift, fuel/epochs, cache, pooling |
| wasmtime-wasi | 50.0.1 | Apache-2.0 WITH LLVM-exception | WASI p2 for plugins (clocks, random, stdio, data folder) |
| ureq | 3.5.1 (`rustls`, `gzip`) | MIT OR Apache-2.0 | blocking HTTP client for `http.fetch` |
| rustls / ring / webpki-roots (via ureq) | 0.23 / 0.17 / 1.0 | Apache-2.0 OR ISC OR MIT / Apache-2.0 AND ISC / CDLA-Permissive-2.0 | TLS |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | content hash of components (in-memory compile cache) |
| bytes | 1.12 | MIT | guest stdout/stderr stream glue (wasmtime-wasi types) |
| tokio (no features) | 1.54 | MIT | `AsyncWrite` trait for the guest output stream (already a wasmtime-wasi dependency) |
| criterion (dev) | 0.8.2 | MIT OR Apache-2.0 | benches |
| tempfile (dev) | 3.27 | MIT OR Apache-2.0 | tests |

### `rusticmc-test` (ADR 0030)

| crate | version | licence | why |
|---|---|---|---|
| md-5 | 0.10 | MIT OR Apache-2.0 | offline-mode player UUIDs (MD5 of `OfflinePlayer:<name>`, as the vanilla server) |
| tempfile | 3.27 | MIT OR Apache-2.0 | throwaway server directories |
| toml | 1.2 | MIT OR Apache-2.0 | reading `Cargo.toml` of fixture crates |
| log (`std`) | 0.4.35 | MIT OR Apache-2.0 | capturing plugin and runtime logs per server |
| serde / serde_json | 1 | MIT OR Apache-2.0 | storage and player-data assertions |
| rusticmc-host | (workspace) | MIT OR Apache-2.0 | loading `.wasm` plugins (feature `wasm`, default) |

### `rusticmc-cli` (ADR 0031)

| crate | version | licence | why |
|---|---|---|---|
| clap (`derive`, `wrap_help`) | 4.6 | MIT OR Apache-2.0 | command line |
| anstream / anstyle | 0.6 / 1.0 | MIT OR Apache-2.0 | colored output honouring `NO_COLOR`/`CLICOLOR` (clap's own stack) |
| anyhow | 1 | MIT OR Apache-2.0 | error reports |
| wasmparser | 0.262 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | component validation |
| wit-parser (`decoding`) | 0.262 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | reading a component's imports/exports and the embedded host WIT |
| notify | 8.2 | CC0-1.0 | `rustic run --watch` |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | package checksums |
| assert_cmd / predicates (dev) | 2.2 / 3 | MIT OR Apache-2.0 | end-to-end CLI tests |
| zip (dev) | 9.5 | MIT | building a test jar for `bridge analyze` |

### Rustic Bridge (ADRs 0009, 0040-0042)

`bridge/rusticmc-bridge` (MIT OR Apache-2.0) adds no new crate: `rusticmc-core`, `serde`,
`serde_json`, `toml`, `semver`, `log` (dev: `tempfile`). The JNI spike `bridge/spikes/jni` (its own
workspace, not shipped) uses `jni` 0.22 (MIT OR Apache-2.0) with `java-locator` (MIT OR Apache-2.0)
and `libloading` (ISC).

`bridge/java` (GPL-3.0-only) is compiled against, and runs with, these jars fetched at build time
from `repo.papermc.io` (`bridge/java/deps.txt`, SHA-256 pinned, never committed):

| Artifact | Licence |
|---|---|
| `io.papermc.paper:paper-api` 26.3.build.246-stable (Bukkit API) | GPL-3.0 (Bukkit API), MIT (Paper additions) |
| `net.kyori:adventure-*` 5.2.0, `net.kyori:option` 1.1.0 | MIT |
| `com.google.code.gson:gson` 2.14.0, `com.google.guava:guava` 33.6.0-jre (+ failureaccess, listenablefuture, j2objc-annotations, error_prone_annotations) | Apache-2.0 |
| `org.yaml:snakeyaml` 2.2 | Apache-2.0 |
| `org.joml:joml` 1.10.9 | MIT |
| `it.unimi.dsi:fastutil` 8.5.18 | Apache-2.0 |
| `org.apache.logging.log4j:log4j-api` 2.26.0 | Apache-2.0 |
| `org.slf4j:slf4j-api` 2.0.17 | MIT |
| `com.mojang:brigadier` 1.3.11 | MIT |
| `net.md-5:bungeecord-chat` 1.21-R0.2-deprecated+build.21 | BSD-3-Clause |
| `org.jspecify:jspecify` 1.0.0, `org.checkerframework:checker-qual` 4.2.3, `org.jetbrains:annotations` 26.1.0, `com.google.auto.service:auto-service-annotations` 1.1.1 | Apache-2.0, MIT, Apache-2.0, Apache-2.0 |
| `org.xerial:sqlite-jdbc` 3.50.3.0 (test plugin only, shaded into `Probe.jar`) | Apache-2.0 |

Concepts: Bukkit's class loader per jar, `HandlerList` dispatch and main-thread scheduler model are
followed as the API documents them; no Bukkit/Spigot/Paper/CraftBukkit/Glowstone implementation
code is used. A portable Apache Maven 3.9.11 (Apache-2.0) was used once, outside the repository,
to resolve the dependency list; it is not needed to build.

### Benchmarks (`bench/`, docs/PERFORMANCE.md)

Tools used by the benchmark scripts, never shipped and never committed:

| Tool / artifact | Licence | Use |
|---|---|---|
| Paper 26.3 (server jar from PaperMC's download API, `bench/fetch-paper.sh`) and the vanilla server jar its Paperclip launcher downloads from Mojang | GPL-3.0 (Paper) / Minecraft EULA (vanilla) | the comparison server, run locally in `.local/paper-bench/` (gitignored) with the user's authorisation |
| `psutil` (Python package, `bench/sample.py`) | BSD-3-Clause | process CPU time and resident memory, portable across Windows and Linux |
| Aikar's JVM flags (as published in Paper's documentation) | (command-line options) | the Paper JVM's flags, stated in the report |

The Bukkit workload and probe plugins (`bench/bukkit/`) are original code compiled against the
published Paper API jar fetched by `bridge/java/build.sh`; like `bridge/java` they are
GPL-3.0-only (SPDX headers), separate from Rustic's `MIT OR Apache-2.0` code.

## Rustic edition client (launcher branch): in-game look

- **Lunar Client** inspired the *organisation* of the in-game menus and the Discover screen (a server directory
  with a featured hero, sections and Join buttons, small stacked title buttons). No Lunar code, asset, text, icon
  or colour is used; Rustic keeps its own identity (orange accent, Inter, its own icons).
- **Language flags** on the edition's language screen are **drawn in code** (`client-mod/rustic-companion/core/src/flags.rs`:
  bands, crosses, discs, triangles; simplified, no coats of arms or stars). No third-party flag set is bundled.
- **Inter** (SIL OFL 1.1), already bundled for the title menu (`client-mod/rustic-companion/mod/assets/`), sets every
  label of the Rustic widget skin.

