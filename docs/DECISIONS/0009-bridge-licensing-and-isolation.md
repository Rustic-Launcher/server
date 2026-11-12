# ADR 0009: Rustic Bridge licensing and isolation

- Status: accepted
- Date: 2026-11-11

## Context

Rustic Bridge runs existing Bukkit/Paper plugins on Rustic (phase 4). A plugin is a jar compiled
against the Bukkit/Paper API, whose interfaces (`org.bukkit.*`, `io.papermc.paper.*`) are
published under the GPL-3.0. Something has to *implement* those interfaces for the plugin's calls
to land in Rustic. Rustic itself is `MIT OR Apache-2.0` (ADR 0006) and must stay so: Horizon is
MIT, wasmtime Apache-2.0, and patches go upstream under MIT. No code of Bukkit, Spigot, Paper,
CraftBukkit or Glowstone may be copied anywhere (AGENTS.md).

## Decision

1. **Two programs.** The bridge is split along a process boundary:
   - `bridge/java/` (**GPL-3.0-only**, SPDX header in every file, `COPYING` beside it): the Java
     layer that implements the published API interfaces. It is compiled against the Paper API jar
     (`io.papermc.paper:paper-api:26.3.build.246-stable`, the newest 26.3 build on
     `repo.papermc.io` on 2026-11-10) and its libraries, fetched at build time with SHA-256 checks
     (`bridge/java/deps.txt`) into the gitignored `bridge/java/build/`. Nothing from the API is
     committed; nothing from Bukkit/Spigot/Paper/CraftBukkit/Glowstone implementation code is used
     or copied: the implementations are written from the published interfaces and their javadoc
     behaviour (the API's own concrete classes, such as `JavaPlugin`, `PluginCommand`,
     `HandlerList`, `YamlConfiguration`, run as-is from the API jar).
   - `bridge/rusticmc-bridge/` (**MIT OR Apache-2.0**): the Rust side. It starts the Java program
     as a child process and talks to it over a socket with its own JSON protocol (ADR 0041). It
     does not implement any Bukkit interface, does not link or embed Java, and contains no Bukkit
     code: it maps Rustic core concepts (plugins, events, commands, permissions, ticks) to protocol
     messages.
2. **Optional.** `rustic-server` gets the bridge only with the cargo feature `bridge`; a server
   without `plugins/bukkit/*.jar` never starts a JVM. The GPL program is distributed separately
   (the server finds `rustic-bridge.jar` in `bridge/` or a configured folder), so a Rustic binary
   carries no GPL code. The Paper API jar and its libraries ship with the Java layer as its
   runtime classpath, as a Paper server does (they are GPL-3.0 / MIT / Apache-2.0, see
   `docs/THIRD_PARTY.md`).
3. **Isolation is also the licence boundary.** The same boundary that keeps the GPL program a
   separate work (two processes exchanging data over a documented protocol, either replaceable)
   is what keeps a misbehaving plugin from taking the server down (ADR 0040).

## Consequences

- Distributing `rustic-bridge.jar` means honouring the GPL-3.0 for it (source available:
  `bridge/java/` in this repository).
- The Java layer may evolve freely under the GPL; the protocol is the stable interface. A
  different implementation of the protocol (another language, another API) is possible.
- Developers of the MIT/Apache crates must not move Bukkit-facing code into Rust: anything that
  implements or mirrors an `org.bukkit` interface belongs in `bridge/java`.

## Alternatives considered

- **One GPL program for all of Rustic**: rejected, it would force the GPL on the core and on
  Horizon patches.
- **Embedding the JVM in the server process (JNI)**: legally similar to dynamic linking, which is
  exactly the grey zone the GPL FAQ warns about; also rejected on isolation grounds (ADR 0040).
- **Reusing Glowstone or CraftBukkit**: not allowed (copied code), and both carry Mojang-derived
  or GPL code into the core path.
