# ADR 0008: Bridge analyzer: own class-file reader, static classification rules

- Status: accepted
- Date: 2026-11-10

## Context

Rustic Bridge (phase 4) wants to run some Bukkit/Spigot/Paper plugins through a GPLv3 Java layer
(ADR 0009). Before building it we need numbers: which API members real plugins call, how many
plugins only need what Rustic's WIT already offers, and which ones can never work because they
reach into server internals. `rustic bridge analyze` must also tell a server owner, for one jar,
whether it has a chance. The analysis must be static (no JVM, no class loading of untrusted
code), fast enough for a panel of ~50 jars, and must not depend on downloading the Bukkit API.

## Decision

### Crate and parser

`bridge/rusticmc-bridge-analyze` is a library (`rusticmc_bridge_analyze`) plus the binary
`rustic-bridge-analyze`; the CLI calls the library (`analyze_path`, `analyze_bytes`,
`analyze_all`, `build_panel`, `render_*_markdown`, serde types for JSON).

Class files are read by a **compact parser written for this crate** (`src/classfile.rs`, ~400
lines): constant pool, declared members, `Code` attributes walked instruction by instruction
(full opcode length table, `tableswitch`/`lookupswitch` padding, `wide`), `BootstrapMethods`.
It returns referenced classes (constant-pool `Class` entries plus every descriptor and generic
signature, annotation types included), referenced fields/methods with their bytecode site counts
(`get*`/`put*`, `invoke*`, method handles in `invokedynamic` bootstrap arguments and `ldc`), and
identifier-like string constants. Every read is bounds-checked; a bad method body is a warning.

Crates considered (all checked on crates.io, 2026-11):

| Crate | Licence | Why not |
|---|---|---|
| `cafebabe` 0.9 | MIT | Good full parser, but we would still walk bytecode ourselves for call-site counts and lenient recovery; it parses far more (attributes, verification data) than needed. |
| `ristretto_classfile` 0.35 | MIT/Apache | Reader + writer + verifier; heavy for a read-only scan. |
| `classfile-parser` 0.3 | MIT/Apache | nom-based, little maintenance, fails a whole class on one unknown attribute shape. |
| `classfile` 0.4 | MIT/Apache | no_std validator, strict by design (we want leniency on obfuscated jars). |

Other dependencies (all MIT and/or Apache-2.0 unless noted, allowed by `deny.toml`):
`zip` 9 (MIT; `default-features = false`, `deflate-flate2-zlib-rs` only: pure Rust inflate via
`flate2` + `zlib-rs` (Zlib); no bzip2/zstd/lzma/aes), `yaml_serde` 0.10 (the maintained
continuation of `serde_yaml`, MIT/Apache, pure-Rust `libyaml-rs` MIT) for `plugin.yml`, `toml` 1
for the coverage table, `serde`/`serde_json` for the report. Transitive licences: MIT, Apache-2.0,
Zlib, BSL-1.0 (`ryu`), Unicode-3.0, CC0/MIT-0 options.

### What is analyzed

- Descriptors: `plugin.yml` and `paper-plugin.yml` (name, version, main, api-version, depend,
  softdepend, loadbefore, commands, permissions, libraries, Paper bootstrapper/loader and
  `dependencies.server`). `api-version: 1.20` is kept literally (YAML would read `1.2`).
- Classes: every `.class` of the jar and of jars nested in it (`*.jar`, `*.jarinjar`, two levels,
  as LuckPerms ships its code), skipping `META-INF/versions/*` and `module-info`. Limits: 64 MiB
  per entry, 200 000 entries.
- **Targets exclude every class present in the jar** (the plugin's own and shaded libraries).
  A member called on a jar class but declared outside it (`this.getConfig()` in a `JavaPlugin`
  subclass, `task.runTaskTimer` on a `BukkitRunnable` subclass) is attributed to the external
  superclass (superclass chain first, then interfaces when the chain ends in the JDK).
- **Own code vs bundled library**: own code is under the first three segments of `main`'s
  package, without a shading marker segment (`lib`, `libs`, `shaded`, `relocated`, `dependencies`,
  `deps`, `external`, `vendor`, ...). Everything else is a bundled library (reported by root).
- Namespaces of referenced classes: **API** (`org.bukkit` except `craftbukkit`, `io.papermc.paper`,
  `com.destroystokyo.paper`, `net.kyori`, `org.spigotmc`, BungeeCord chat, `co.aikar.timings`,
  `com.mojang.brigadier`, Purpur API, and Vault, which the table maps onto `rustic:services`),
  **internal**, **packet library**, **instrumentation**, JDK, **platform library** (Gson, Guava,
  SnakeYAML, log4j, slf4j, fastutil, JOML, annotations: what the server puts on the classpath),
  other platforms (BungeeCord proxy, Velocity, Fabric...) and external (other plugins' APIs,
  `libraries:` downloads).

### Coverage table

`data/coverage.toml` maps API classes and members to Rustic: a WIT function (`players.send-message`),
an event (`events.player-join`), `java-side: ...` (pure-Java API the bridge implements without a
host call: configuration, Adventure/BungeeCord components, `Location`, `Vector`, colours...),
`partial: ...` (covered with a stated semantic gap; counted as covered, listed separately) or
`uncovered: why`. Absent means uncovered. Lookup: owner class (`name(descriptor)`, `name`, `*`),
then its `inherits` list; **a class absent from the table is uncovered with all its members**,
so `PlayerKickEvent#getPlayer` does not ride on `PlayerEvent#getPlayer`. Constructors and
`Object`/`Enum` methods follow their class. The table holds names of public API types and members
only (facts needed for interoperability analysis), written by hand from the WIT; a unit test
checks that every `iface.function` it names exists in `crates/rusticmc-api-wit/wit`.

### Verdict

- 🔴 **red** if any *decisive* signal:
  - references to `net.minecraft.*` (NMS), `org.bukkit.craftbukkit.*`, other server internals
    (`com.mojang.*` except Brigadier, `io.netty.*`, `ca.spottedleaf.*`, Paper's `PaperAdventure`),
    ProtocolLib (`com.comphenix.protocol`) or PacketEvents, from own code **or** a bundled library
    (library code runs too);
  - reflection into internals: the jar calls reflection (`Class.forName`, `getDeclared*`,
    `setAccessible`, `MethodHandles.Lookup#find*`, `ClassLoader#loadClass`...) and a class holds
    a string naming internals (`net.minecraft.`, `org.bukkit.craftbukkit`, `com.mojang.authlib`,
    `io.netty.channel`, `getHandle`, a `Craft*` class name such as `.entity.CraftHumanEntity`);
  - ASM/ByteBuddy/Javassist/`java.lang.instrument` or `sun.misc.Unsafe`/`jdk.internal` **in own
    code** (in a bundled general-purpose library, e.g. Caffeine's `Unsafe`, they are recorded as
    non-decisive);
  - a hard `depend` on ProtocolLib, PacketEvents, ProtocolSupport, TinyProtocol or NBTAPI.
  The report flags red plugins whose decisive signals all come from bundled libraries
  (`red_only_in_libraries`) and those red only through reflection strings (often a guarded
  fallback).
- 🟡 **yellow** otherwise, if any referenced API member or type is uncovered (listed with site
  counts).
- 🟢 **green** otherwise.
- ⚠️ error when the jar cannot be opened.

## Consequences

- One small, dependency-light, fully safe crate; no JVM needed to vet a plugin, ~0.2 s per jar.
- The verdict is conservative (static references, not reachability): a reference in a dead or
  guarded branch still counts. Red therefore means "touches internals somewhere", not "cannot
  start". Blind spots are listed in `docs/BRIDGE_ANALYSIS.md`.
- The coverage table must follow the WIT: a new WIT function or event should come with table
  rows; the WIT-consistency test catches rows naming functions that do not exist, not missing rows.
- First results (55 popular plugins): docs/BRIDGE_ANALYSIS.md.

## Alternatives

- **Load jars in a JVM and resolve with the real Bukkit API jar**: precise inheritance and
  overloads, but needs a JDK at analysis time, downloads the GPL API, and executes static
  initialisers of untrusted code. Rejected for the default path; possible later as an opt-in
  "precise" mode in `bridge/java`.
- **Use `cafebabe` and only walk the constant pool**: simpler, but loses call-site counts and the
  method-handle targets of lambdas (`Player::getName`), which the rankings need.
- **Classify by namespace only (no coverage table)**: would make every Bukkit user "yellow" and
  say nothing about what to build next.
