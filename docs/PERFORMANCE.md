# Performance

Two sets of measurements:

1. [**Rustic server vs Paper**](#rustic-server-vs-paper): the whole server (Horizon's engine with
   the Rustic runtime) against Paper on the same machine, the same world settings and the same
   protocol bots: startup, memory, tick times, CPU, and the cost of the same plugin written as a
   Rustic WASM plugin, as a Bukkit plugin on Paper, and as that Bukkit jar through Rustic Bridge.
2. [**WASM boundary micro-benchmarks**](#wasm-boundary-micro-benchmarks): what one event, command or
   host call costs inside `rusticmc-host` (criterion). Core-only numbers (event bus with native
   listeners, `has_listeners_for`) are in ADR 0003.

## Rustic server vs Paper

Measured 2026-11-11 with the scripts in [`bench/`](../bench/README.md); every number below comes
from those runs (raw results: `.local/rustic-bench/results/`, not committed).

### Verdict

- **Startup and memory: Rustic wins by an order of magnitude.** A warm start to "Done" takes
  ~2 s against 13-26 s for Paper (fresh world both times; ~26 s in the matrix runs, 12.7-14.9 s
  in the lead's re-check on a quieter machine, see "Re-check"), and an idle server holds ~185 MB of RSS
  against ~4.5 GB for Paper with the usual Aikar flags (`-Xms4G -Xmx4G -XX:+AlwaysPreTouch`
  commit the whole heap; without them an idle Paper still held 922 MB).
- **Idle ticks: a tie.** Both servers tick an empty world in ~0.4 ms (p50 ~0.35 ms); Paper's
  p99 and max are higher (JIT, GC, background profiler), and it idles at 0.12 core against 0.03.
- **Under load Rustic loses.** With 10 walking, chatting bots Paper's mean tick is a little lower
  (5.2 vs 6.1 ms) but Rustic's tail is tighter (p99 8.3 vs 20.2 ms). From 50 bots on, Rustic's
  mean tick is about twice Paper's (19.0 vs 9.1 ms at 50, 30.3 vs 16.8 ms at 100), its p95/p99 are
  higher, and it burns 1.5-2.2 CPU cores against 0.4-1.3 for Paper (4.6-4.9 cores at 10 bots in
  the re-check: Horizon's polling threads take whatever CPU is free). Both keep 20 TPS up to 100
  bots; at 200 Rustic falls to 14.2 TPS (70 ms ticks) while Paper holds 19.8 (33 ms). The cost is
  in Horizon's engine, not in Rustic's plugin layer (these runs load no plugin): see "Where
  Rustic's time goes".
- **Plugins: WASM is cheap, the Bridge is not.** The same workload (move + chat listeners, a
  command, a task every tick) costs +1.1 ms per tick as a Rustic WASM plugin (1.05 ms in Rustic's
  own timings, most of it the task's 50 `location()` calls; **0.27 ms after the backend fix** below)
  and +0.2 ms as a Bukkit plugin on
  Paper. The same jar through Rustic Bridge costs +12.9 ms per tick at 50 bots: every event and
  every API call crosses to the JVM sidecar and back (~1 000 moves a second here).
- **With vanilla defaults (locator bar on) Paper did not keep 50+ of these bots**: it timed them
  all out ("kicked due to keepalive timeout") 40-90 s after they joined, which also invalidates its
  numbers there; see "Locator bar on". The main tables turn the locator bar off on both servers.
  At 200 bots Paper timed the bots out even so, 52 s into the 66 s window.

### Setup

| | |
|---|---|
| Machine | Intel Core i5-13400F (10 cores, 16 threads), 32 GB RAM, NVMe SSD, Windows 11 Home 10.0.26200 |
| Rustic | `rustic-server` (`--profile fast`, feature `bridge`), Rustic at `9877a4c` + working tree of 2026-11-11, Horizon `a84f637d` + `patches/` (branch `rustic-patched` at `7ef3fdd9e`), rustic.toml defaults |
| Paper | Paper 26.3 build 248 (latest STABLE, PaperMC download API, SHA-256 `352dbbf9...b92c2`), defaults (bundled spark with its background profiler on, bStats off) |
| Paper JVM | Temurin 25.0.5+8, Aikar's flags as published in Paper's documentation: `-Xms4G -Xmx4G -XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:+AlwaysPreTouch -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 -XX:G1HeapRegionSize=8M -XX:G1ReservePercent=20 -XX:G1HeapWastePercent=5 -XX:G1MixedGCCountTarget=4 -XX:InitiatingHeapOccupancyPercent=15 -XX:G1MixedGCLiveThresholdPercent=90 -XX:SurvivorRatio=32 -XX:+PerfDisableSharedMem -XX:MaxTenuringThreshold=1` |
| Bridge JVM | the same JDK, `-Xmx512m` (as `bridge/java/e2e.sh`) |
| World (both) | fresh each run: `level-type=minecraft:flat`, `generate-structures=false`, creative, view and simulation distance 4, offline, `allow-flight=true`, `enforce-secure-profile=false`, `pause-when-empty-seconds=0` (an empty server keeps ticking), `gamerule locator_bar false` (see "Locator bar on") |
| Bots | `horizon-bots --behaviour walk`, one joining every 0.1 s: each flies 24 blocks above the ground and walks at sprint speed between random points within 48 blocks of spawn (a move packet every tick, so every bot sees every other), chats every 2 s (unsigned, as offline clients do) and runs `/workload` every 5 s (an unknown command when the workload plugin is absent, on both servers) |
| Timeline | start, 5 s, bots join, 45 s warm-up, 60 s recorded window, stop; 3 repetitions of every scenario, servers interleaved (`bench/matrix.sh 3`) |

### Method

- **Tick time.** Each server's own per-tick record over the 60 s window (about 1 200 ticks):
  Paper's `ServerTickEndEvent` duration, recorded by a small probe plugin
  (`bench/bukkit/probe`), and Horizon's `horizon-report 60` (every tick's wall time; its own
  recording overhead is ~0.003 ms per tick). Percentiles of both are computed the same way (sorted
  sample at `floor(n * q)`). As a cross-check, the vanilla `tick query` command (same definition on
  both: the last 100 ticks) runs every 5 s on both consoles; its averages agree with the per-tick
  records within ~0.3 ms (Paper's event-based duration reads ~0.1 ms higher than vanilla's when
  idle). Horizon's tick includes handling the players' packets. Paper's main-thread CPU (probe,
  `ThreadMXBean`) was 5.8 / 10.6 / 15.8 ms per tick at 10 / 50 / 100 bots against tick times of
  5.2 / 9.1 / 16.8 ms: at most ~1.5 ms per tick of main-thread work (packet tasks run between
  ticks) falls outside Paper's measured tick, which does not change the picture. Horizon's
  server-thread CPU inside the tick was 5.8 / 13.2 / 22.0 ms (the rest of its tick is waiting).
- **CPU and memory.** `bench/sample.py` (psutil) reads user + system CPU time and RSS (working set)
  of the server's process tree once a second during the window: CPU in cores (1.0 = one core
  busy), RSS averaged. Rustic Bridge's JVM sidecar is a child process and is counted. The bot
  process is sampled too (it runs on the same machine: 0.04-0.33 cores).
- **Startup.** Wall time from launching the process to the "Done (" line, plus the server's own
  "Done (X s)" (which excludes JVM and Paperclip start-up for Paper). Rustic prints "Rustic ready"
  ~50 ms after "Done" (2.5 s with the Bridge, which starts its JVM). Warm file cache; cold numbers
  are under "Caveats".
- **Entities.** `execute if entity @e[...]` on both consoles after the window: on a superflat
  world below Y=40 slimes spawn in slime chunks, so both worlds fill with slimes (Rustic 78-100,
  Paper 118-141 at 10-100 bots), plus the animals generated with the chunks.

### 1. Startup and idle

| server | runs | launch to "Done", ms | self-reported "Done", s | RSS idle, MB | CPU idle, cores | MSPT mean | p50 | p95 | p99 | max |
|---|---|---|---|---|---|---|---|---|---|---|
| Rustic | 3 | 1961 (1860-1970) | 1.76 (1.67-1.84) | 186 (185-186) | 0.034 (0.022-0.054) | 0.37 (0.37-0.38) | 0.35 (0.34-0.36) | 0.52 (0.51-0.52) | 0.65 (0.61-0.65) | 3.2 (1.0-3.4) |
| Paper | 3 | 26356 (25762-28463) | 24.80 (24.67-26.85) | 4497 (4493-4500) | 0.124 (0.122-0.137) | 0.41 (0.41-0.45) | 0.35 (0.34-0.37) | 0.59 (0.59-0.64) | 2.27 (2.09-2.71) | 16.1 (16.1-16.1) |

### 2. Idle MSPT

Same runs as above (0 bots, 60 s window, ~1 200 ticks each): both ~0.4 ms mean. Paper's tail is
longer (p99 2.3 ms, max up to 16 ms: JIT compilation, G1, spark's background sampler),
Rustic's p99 is 0.65 ms. Idle CPU: Rustic 0.03 cores, Paper 0.12 cores.

### 3. Load

Median of 3 runs (min-max in parentheses), no plugin:

| bots | server | runs | MSPT mean | p50 | p95 | p99 | max | TPS | `tick query` avg | CPU, cores | RSS, MB | entities at end (slimes), median | bots' CPU, cores |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | Rustic | 3 | 0.4 (0.4-0.4) | 0.3 (0.3-0.4) | 0.5 (0.5-0.5) | 0.7 (0.6-0.7) | 3 (1-3) | 20.01 (20.01-20.01) | 0.4 (0.4-0.4) | 0.03 (0.02-0.05) | 186 (185-186) | 0 (0) | n/a |
| 0 | Paper | 3 | 0.4 (0.4-0.4) | 0.3 (0.3-0.4) | 0.6 (0.6-0.6) | 2.3 (2.1-2.7) | 16 (16-16) | 20.00 (20.00-20.00) | 0.3 (0.3-0.3) | 0.12 (0.12-0.14) | 4497 (4493-4500) | 0 (0) | n/a |
| 10 | Rustic | 3 | 6.1 (6.1-6.3) | 6.0 (6.0-6.2) | 7.3 (7.2-7.7) | 8.3 (8.2-8.6) | 13 (11-15) | 20.01 (20.01-20.01) | 6.2 (6.2-6.4) | 2.13 (1.98-2.16) | 218 (216-218) | 115 (87) | 0.04 (0.04-0.04) |
| 10 | Paper | 3 | 5.2 (5.1-5.7) | 4.7 (4.4-4.8) | 8.4 (7.1-9.3) | 20.2 (18.8-20.4) | 64 (42-105) | 20.00 (19.99-20.00) | 5.1 (4.9-5.6) | 0.36 (0.34-0.39) | 4570 (4567-4580) | 139 (119) | 0.05 (0.03-0.06) |
| 50 | Rustic | 3 | 19.0 (18.4-19.1) | 15.4 (15.3-15.6) | 34.5 (34.0-36.2) | 51.3 (46.4-58.3) | 255 (221-333) | 20.01 (20.01-20.01) | 19.2 (18.7-19.4) | 1.80 (1.80-1.81) | 231 (228-232) | 158 (80) | 0.15 (0.15-0.16) |
| 50 | Paper | 3 | 9.1 (8.9-9.9) | 8.3 (8.1-9.1) | 12.7 (12.4-13.8) | 25.9 (25.6-26.2) | 105 (70-108) | 20.00 (20.00-20.00) | 9.1 (8.8-9.8) | 0.72 (0.60-0.80) | 4596 (4594-4603) | 192 (127) | 0.18 (0.13-0.19) |
| 100 | Rustic | 3 | 30.3 (30.2-30.3) | 26.2 (26.2-26.5) | 54.5 (53.5-56.3) | 78.9 (74.0-81.9) | 325 (153-366) | 20.00 (19.99-20.01) | 30.5 (30.4-30.6) | 1.59 (1.57-1.69) | 244 (243-247) | 201 (81) | 0.33 (0.32-0.34) |
| 100 | Paper | 3 | 16.8 (16.8-17.0) | 14.2 (14.2-14.4) | 32.1 (31.8-32.3) | 58.0 (48.1-63.5) | 156 (136-321) | 20.00 (19.98-20.00) | 16.6 (16.6-16.9) | 1.28 (1.21-1.33) | 4628 (4614-4630) | 235 (118) | 0.31 (0.29-0.31) |
| 200 | Rustic | 1 | 70.1 | 59.4 | 123.9 | 162.3 | 466 | **14.24** | 70.3 | 1.54 | 276 | 321 (86) | 0.54 |
| 200 | Paper | 1 | 33.0 | 23.7 | 71.0 | 137.6 | 377 | 19.84 | 31.1 | 1.84 | 4663 | 336 (121) | 0.50 |

Where Rustic's time goes (Horizon's own report of the same runs):

- **Main thread.** At 50 bots ~13 ms of the ~19 ms tick is server-thread CPU (level ticking,
  mobs, entity tracking, the 50 connections); the rest is waiting. The worst ticks (100-400 ms)
  show ~15 ms of CPU: the server thread is blocked, mostly in `lan_track_entities` (entity
  tracking), once for 2.5 s in `lan_guests` (the 2 579 ms max of one Bridge run, with 6 ms of CPU).
- **Other threads.** As soon as a player is online Horizon keeps one thread at ~0.95 core and its
  ~15 chunk workers at ~0.1 core each (its report counts ~576 000 near-empty "other" worker jobs a
  second): 1.5-2.2 cores whatever the player count, against 0.03 core empty. This looks like busy
  polling; it is reported upstream through docs/HANDOFF.md.
- **Network.** Horizon sends 6x more entity-move packets than Paper (49 000 vs 8 000 to one bot in
  100 s at 50 bots) and Paper ~5x more locator-bar waypoints (when on).

None of it is in Rustic's plugin layer: the no-plugin runs load no plugin, and Rustic's own tick
work with no plugin is a few microseconds.

The 200-bot row is a single run each (stress): Rustic kept all 200 bots but ran at 14.2 TPS (808
ticks over 50 ms); Paper held 19.8 TPS but timed every bot out 52 s into the 66 s window (even
with the locator bar off), so its last ~14 s had idle players.

### 4. Plugin overhead

One workload written three ways (original code): `bench/plugins/rustic-workload` (Rust SDK, WASM)
and `bench/bukkit/workload` (Bukkit API; the same jar on Paper and through Rustic Bridge). Each
listens to player move (per-player move count and distance), chat (lower-case, five banned words,
cancel and warn on a match; the bots never match), answers `/workload` with its counters, and runs
a task every tick reading every online player's location. 50 bots, same script as above:

| server | plugin | bots | runs | MSPT mean | delta vs no plugin (medians) | p95 | p99 | max | CPU, cores | RSS, MB | workload counters (last run) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Paper | none | 50 | 3 | 9.1 (8.9-9.9) |  | 12.7 (12.4-13.8) | 25.9 (25.6-26.2) | 105 (70-108) | 0.72 (0.60-0.80) | 4596 (4594-4603) |  |
| Paper | bukkit | 50 | 3 | 9.3 (8.9-9.5) | +0.2 ms | 12.6 (12.1-13.6) | 24.0 (23.7-25.0) | 215 (57-395) | 0.71 (0.67-0.73) | 4594 (4591-4605) | workload: 2263 ticks, 105513 moves (30071.6 blocks), 2594 chats (0 blocked), farthest 58.8 |
| Rustic | none | 50 | 3 | 19.0 (18.4-19.1) |  | 34.5 (34.0-36.2) | 51.3 (46.4-58.3) | 255 (221-333) | 1.80 (1.80-1.81) | 231 (228-232) |  |
| Rustic | bukkit | 50 | 3 | 31.9 (31.6-36.0) | +12.9 ms | 48.3 (44.4-48.9) | 69.3 (69.0-73.7) | 149 (131-2579) | 1.49 (1.43-1.59) | 617 (614-619) | workload: 2419 ticks, 113008 moves (32170.2 blocks), 2762 chats (0 blocked), farthest 59.0 |
| Rustic | wasm | 50 | 3 | 20.1 (19.4-20.5) | +1.1 ms | 34.9 (28.0-35.6) | 61.9 (58.6-67.8) | 141 (120-311) | 1.83 (1.80-1.84) | 232 (231-233) | workload: 2453 ticks, 113447 moves (32293.2 blocks), 2757 chats (0 blocked), farthest 58.6 |

- The **WASM plugin** costs +1.1 ms of tick time (medians; the ranges overlap the baseline's). A
  separate run with `rustic timings on` (`RUSTIC_TIMINGS=1`) attributes 1.05 ms per tick (5.2 % of
  the tick) to it over 66.8 s: 66 302 move events (3 µs each, p99 29 µs), 1 664 chats (11 µs),
  669 `/workload` commands (45 µs) and 1 336 runs of the 1-tick task (0.85 ms each: `Player::online()`
  plus 50 `location()` calls, ~17 µs per call on the Horizon backend against 0.37 µs per import in
  the micro-benchmarks below).
- **Fixed since (lead, 2026-11-11):** each player read rebuilt the whole `host.server_players()`
  list, so the task was quadratic in players. The backend now reads that list once per runtime
  entry and drops it on any host call that may change the game
  (`crates/rusticmc-server/src/backend.rs`, unit test with a counting host). Same scenario with
  `RUSTIC_TIMINGS=1`, one run: the task fell from **0.850 to 0.108 ms** per tick (~2 µs per
  `location()`) and the whole plugin from **1.05 to 0.27 ms per tick** (5.2 % → 1.1 % of the
  tick); moves 3 µs, chats 11 µs and the command 49 µs are unchanged. The table above predates the
  fix. (That run's engine numbers, 24-40 ms mean ticks with a 2.4 s stall in `lan_track_entities`,
  were again a busier machine: compare the plugin's own timings, not the run's MSPT.)
- The **Bukkit plugin on Paper** costs +0.2 ms (Paper fires `AsyncPlayerChatEvent` off the
  main thread; moves and the task are in-process calls).
- **Rustic Bridge** costs +12.9 ms per tick for the same jar: every event and every API call
  (`getLocation` x 50 each tick) is a round trip to the JVM sidecar (docs/BRIDGE.md measured
  60-80 µs per event). It held 20 TPS in two runs and 19.2 in the third; it is the compatibility
  path, not the fast one: port hot listeners to WASM.
- Workload counters show both Rustic variants received ~113 000 moves in a run against ~105 000 on
  Paper (Paper fires `PlayerMoveEvent` only past its own movement thresholds; Rustic's default
  `[events] move_threshold` is 0.0625 blocks): Rustic's plugins did slightly more work.

### Locator bar on (vanilla defaults)

The first round ran with the vanilla default `locator_bar = true` (1 repetition):

| bots | server | MSPT mean | p95 | p99 | max | TPS | CPU, cores | bots kicked by the server |
|---|---|---|---|---|---|---|---|---|
| 0 | Rustic | 0.4 | 0.5 | 0.7 | 3 | 20.01 | 0.02 | - |
| 0 | Paper | 0.5 | 0.6 | 2.6 | 44 | 20.00 | 0.13 | - |
| 10 | Rustic | 6.2 | 7.4 | 8.2 | 9 | 20.01 | 2.14 | 0 |
| 10 | Paper | 5.4 | 9.8 | 21.2 | 100 | 20.00 | 0.40 | 0 |
| 50 | Rustic | 20.0 | 36.3 | 62.3 | 305 | 20.01 | 1.76 | 0 |
| 50 | Paper | 15.6 | 41.3 | 86.9 | 389 | 19.90 | 1.94 | 50 of 50 |
| 50 + Bukkit workload | Paper | 15.7 | 46.3 | 150.3 | 363 | 19.95 | 1.40 | 50 of 50 |
| 50 + Bukkit via Bridge | Rustic | 33.2 | 47.7 | 70.0 | 383 | 20.01 | 1.50 | 0 |
| 50 + WASM workload | Rustic | 20.6 | 32.6 | 58.1 | 99 | 20.01 | 1.77 | 0 |
| 100 | Rustic | 37.7 | 62.2 | 102.8 | 413 | 20.00 | 1.48 | 0 |
| 100 | Paper | 25.1 | 58.7 | 317.9 | 1161 | 18.24 | 1.72 | 100 of 100 |

With the locator bar on, Paper sent each bot ~1 670 `Waypoint` packets a second at 50 bots (8 MB
in 100 s, vs 1.5 MB from Rustic) and a keep-alive every ~1.4 s, and at 50 and 100 bots it logged
"kicked due to keepalive timeout" for every bot 40-90 s after they joined; the kicks completed only
at shutdown, so the bots stayed in the world without being processed and the window measured a
server with idle players (its workload plugin saw ~61 000 moves instead of ~104 000). The bot
process used ~0.2 core, so this is not the load generator falling behind; whether Paper's network
threads or Windows loopback buffering is the bottleneck was not investigated further. Turning the
locator bar off on both servers (`gamerule locator_bar false`, the main tables) removed the kicks
and nearly halved Paper's tick time at 50 bots (15.6 to 9.1 ms, CPU 1.94 to 0.72 cores); Rustic's
changed little (20.0 to 19.0 ms).

### Re-check (lead, 2026-11-11, after the matrix)

Four extra runs with the same scripts, about 20 minutes after the matrix, to check its numbers:

| run | Rustic | Paper |
|---|---|---|
| launch to ready, 0 bots (Paper ×3, short window) | 2.2-2.3 s | 12.7 / 14.9 / 13.3 s (self-reported "Done" 11.1-12.9 s) |
| 10 bots, 45 s warm-up, 60 s window (Rustic ×1) | MSPT mean 4.23, p99 8.5, max 10.8; CPU 4.62 cores; RSS 216 MB | n/a |
| 10 bots, 30 s warm-up, 30 s window (×1 each) | MSPT mean 3.96, p99 7.3, max 9.6; CPU 4.94 cores; RSS 219 MB | MSPT mean 3.58, p99 7.4, max 42.9; CPU 0.33 core; RSS 4 583 MB |

- **The matrix ran on a busier machine.** Paper's JVM bootstrap alone took 9 s in the matrix and
  4 s in the re-check, and both servers' 10-bot ticks were ~30 % faster afterwards (Rustic 6.1 →
  4.0-4.2 ms, Paper 5.2 → 3.6 ms). The cause was not identified (no other benchmark or build of
  ours ran at the time; antivirus scanning of the freshly downloaded Paper files is a guess).
  The relative picture holds: at 10 bots Paper's mean is ~10-15 % lower and Rustic's max is lower.
- **Rustic's CPU figure depends on the machine's load.** With more free CPU, Horizon's busy
  worker threads took 4.6-4.9 cores at 10 bots instead of 2.1: the CPU column of the matrix is a
  lower bound for Rustic, which makes the busy-polling finding (docs/HANDOFF.md) more pressing.
- **Startup**: Paper's warm start is 13-15 s on a quiet machine, not 26 s; Rustic's ~2 s is
  unchanged. Rustic still starts 6-13x faster.

### Caveats

- **Same machine for server and bots** (16 threads; the bots used up to 0.33 core): fine for
  relative numbers, not a capacity claim. Windows, not Linux (the primary CI target); absolute
  numbers will differ on a Linux host.
- **Bots are not players.** They fly in creative, walk in straight lines inside a 48-block circle
  (every bot tracks every other), never break or place blocks, and chat unsigned. Real servers
  spread players out; per-player costs here are a crowded-spawn worst case.
- **Different engines, different defaults.** Horizon runs its chat filter (45 words, on without a
  `server.toml`) and its anticheat; Paper runs neither but runs spark's background profiler. Mob
  counts differ (more slimes on Paper). Neither was tuned: these are defaults an admin gets.
- **Paper's RSS is mostly the pre-touched heap** (Aikar's flags). Started with only
  `-Xmx4G -XX:+UseG1GC` (one run, idle), Paper held 922 MB of RSS with 486 MB of live heap: still
  ~5x Rustic's 186 MB. Under load its live heap was 1.2-1.9 GB (2.8 GB at 200 bots).
- **JIT warm-up**: Paper ran 45 s of load before the window; a longer warm-up could lower its
  numbers slightly.
- **Cold starts.** The first start after a rebuild or after the file cache was evicted took
  ~66-69 s for Rustic (Horizon loads ~590 MB of prepared resources; "load items" alone 19-21 s)
  and ~62 s for Paper's very first start (Paperclip downloads and patches the vanilla jar). The
  table uses warm starts.
- **Rustic Bridge** numbers depend on the JVM sidecar's flags (`-Xmx512m` here) and on the Bridge's
  current one-round-trip-per-event design (docs/BRIDGE.md).
- **Variance.** Mean tick times vary by up to ~10 % between repetitions and max ticks far more
  (mob spawning, bot timing, GC): compare medians and ranges, not single runs. The 200-bot rows,
  the timings run and the no-pre-touch run are single runs.
- **Paper's keep-alive time-outs** (50+ bots with the locator bar on, 200 bots with it off) were
  not root-caused; the bot process stayed at 0.2-0.5 core, but a client-side cause on Windows
  loopback is not excluded.

### Reproduce

```sh
bridge/java/build.sh --skip-tests          # the Paper API jar the Bukkit plugins compile against
bench/build.sh                             # the WASM and Bukkit workload plugins, the Paper probe
bench/fetch-paper.sh                       # Paper 26.3, latest stable build -> .local/paper-bench/
GAMERULES="locator_bar=false" bench/matrix.sh 3
python -I bench/summarize.py report .local/rustic-bench/results/matrix-*/*/
```

Paper and the vanilla jar it downloads are fetched for this local benchmark of the user's own copy
and are never committed (`.local/` is gitignored); `bench/run.sh` writes `eula=true` in the scratch
server folders, accepting the Minecraft EULA as `scripts/rustic-server-test.sh` does.

## WASM boundary micro-benchmarks

### Machine and method

- Intel Core i5-13400F (10 cores / 16 threads), 32 GB RAM, Windows 11 Home, rustc 1.99.1,
  wasmtime 50.0.1 (Cranelift, `OptLevel::Speed`, fuel + epoch interruption on, on-demand
  allocator), 2026-11-10.
- `cargo bench -p rusticmc-host --bench host -- --warm-up-time 1 --measurement-time 3`
  (criterion, release profile: thin LTO, 1 codegen unit). Guests are release builds of
  `plugins/fixtures/bench` (SDK-based, no-op handlers), `plugins/hello` and
  `plugins/fixtures/noop` (`opt-level = "s"`, LTO, `panic = "abort"`).
- Every iteration includes building the event record (for player-move: two `Location`s with a
  heap-allocated world name), `Rustic::fire` (context, watchdog tracker, `catch_unwind`), the
  host side (fuel, epoch deadline, conversion to the WIT record, canonical ABI lowering and
  lifting) and the SDK side (typed event wrapper, handler table lookup).

### WASM boundary

| benchmark | median | notes |
|---|---|---|
| `dispatch/player-move/0` | 0.20 µs | no subscriber: the cost of constructing the event + `fire` |
| `dispatch/player-move/1` | 2.2 µs | one plugin with a no-op handler (target: well under 5 µs) |
| `dispatch/player-move/10` | 21.3 µs | ten plugins (the bench component under ten ids): ~2.1 µs per subscriber |
| `dispatch/player-join/1` | 1.6 µs | smaller record |
| `command/bench-noop` | 1.3 µs | console command: parse, permission, `execute-command` into the plugin |
| `import/players.name/0` | 2.9 µs | custom event into the plugin, handler does nothing |
| `import/players.name/1000` | 377 µs | same handler calling `players.name` 1000 times |
| → one host import round trip | **0.37 µs** | (377 − 2.9) / 1000: guest → host → `Backend` → guest, string result |
| `lifecycle/reload-hello` | 4.5 ms | disable + re-read package + instantiate main and async instances + `on-load`/`on-enable` in both (config read through WASI), compiled component from memory |

Compile and memory (printed by the bench):

| | value |
|---|---|
| `hello.wasm` (460 KiB) cold compile | 180 ms |
| same, from wasmtime's disk cache (`<server>/cache/rustic/`, ADR 0023) | 6.3 ms |
| same, in-memory cache hit (reload of an unchanged plugin) | 0.24 ms |
| linear memory of an idle plugin, main + async instance | 2.1 MiB (`noop` and `hello` alike: ~1.06 MiB per instance, mostly the Rust allocator's initial heap and stack) |

### MSPT budget estimate

At 20 TPS a tick has 50 ms; the goal is that the plugin layer stays **under 2 % (1 ms per
tick)** at typical loads. Per tick, with the measured costs (≈ 2.1 µs per handler call,
0.37 µs per import, 1.3 µs per command, ≈ 2 µs per scheduled task):

| source | typical server (50 players, 10 plugins) | cost |
|---|---|---|
| `player-move`: 25 moving players over the move threshold × 3 subscribing plugins | 75 calls | 0.16 ms |
| other events (chat, interact, block break/place, damage...) | ~20 events × 2 subscribers | 0.08 ms |
| imports made by handlers | ~200 | 0.07 ms |
| sync timers and tasks | ~10 | 0.02 ms |
| commands | ~1 | < 0.01 ms |
| **total** | | **≈ 0.35 ms = 0.7 % of a tick** |

A heavier server (100 players all moving, 5 plugins on `player-move`) reaches
100 × 5 × 2.1 µs ≈ 1.05 ms for moves alone (≈ 2.1 %): raise `[events] move_threshold`, keep
`player-move` handlers to plugins that need them (a plugin that does not subscribe costs
nothing: the core skips it, and the adapter skips building events nobody listens to with
`has_listeners_for`, 0.7 ns). Async tasks run on the pool threads and do not count against
MSPT; plugin code itself (beyond the no-op handlers measured here) is the plugin author's
budget and shows up per plugin in `/rustic timings`.

### Where the per-call time goes

Roughly a third each: the core's call wrapper (call context push, watchdog tracker, timings
check, `catch_unwind`, ~0.2 µs measured with native listeners in ADR 0003), the host
(fuel/epoch setup, record conversion and canonical-ABI copies of strings), and the SDK (typed
event wrapper and handler table). Fuel instrumentation slows guest code itself by 10-30 % on
tight loops; it is required for the sandbox (ADR 0020).

### Reproduce

```sh
cargo bench -p rusticmc-host --bench host
```

The bench builds the guests first (`plugins/target`, `RUSTIC_SKIP_GUEST_BUILD=1` to skip).
