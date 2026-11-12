# bench: Rustic server vs Paper

Reproducible scripts behind the "Rustic server vs Paper" section of
[docs/PERFORMANCE.md](../docs/PERFORMANCE.md), which has the results, the method and the caveats.

| File | What |
|---|---|
| `fetch-paper.sh [VERSION]` | latest STABLE Paper build of a version (26.3 by default) from PaperMC's download API (Fill v3), SHA-256 checked, into `.local/paper-bench/dist/` |
| `build.sh` | builds the workload plugin twice (Rustic WASM, Bukkit jar) and the Paper tick probe |
| `run.sh --server paper\|rustic [--bots N] [--plugin none\|wasm\|bukkit] [--warmup S] [--window S] [--out DIR]` | one run: fresh flat world, startup time, N bots, warm-up, a recorded window, stop |
| `matrix.sh [REPS]` | every scenario of the report, `REPS` times each (default 3), then the tables |
| `summarize.py run DIR` / `table DIR...` | one run's `result.json` / markdown tables (median over repetitions) |
| `sample.py` | CPU (cores) and RSS of a server's process tree (psutil), `--kill` for cleanup |
| `botscript.py` | the bots' chat and command packets for `horizon-bots --raw` |
| `plugins/rustic-workload/` | the workload as a Rustic WASM plugin (Rust SDK; own Cargo workspace, built into `plugins/target`) |
| `bukkit/workload/` | the same workload as a Bukkit plugin (GPL-3.0-only, compiled against the published Paper API) |
| `bukkit/probe/` | Paper-side tick recorder (`ServerTickEndEvent`, main-thread CPU, GC), GPL-3.0-only |

## Quick start

```sh
bridge/java/build.sh --skip-tests     # fetches the Paper API jar the Bukkit plugins compile against
bench/build.sh
bench/fetch-paper.sh                  # Paper 26.3, latest stable build
bench/run.sh --server rustic --bots 10 --window 30
bench/run.sh --server paper --bots 10 --window 30
GAMERULES="locator_bar=false" bench/matrix.sh 3   # everything in docs/PERFORMANCE.md (about 85 min)
python -I bench/summarize.py report .local/rustic-bench/results/matrix-*/*/
```

Environment knobs of `run.sh` (and so of `matrix.sh`):

| Variable | Effect |
|---|---|
| `GAMERULES="locator_bar=false ..."` | `gamerule` commands on the console before the bots join (the report's main tables turn the locator bar off on both servers) |
| `PAPER_HEAP` (default `4G`), `PAPER_JVM_FLAGS` | Paper's heap with Aikar's flags, or a whole replacement flag list |
| `BRIDGE_JVM_ARGS` (default `'-Xmx512m'`) | TOML array items for the Rustic Bridge sidecar's JVM |
| `BOT_CHAT=0` | bots only move (no chat, no commands) |
| `BOT_BEHAVIOUR` (default `walk`) | any `horizon-bots --behaviour` |
| `BOT_CAPTURE=1` | records the first bot's packets to `capture.bin` (`horizon-bots --capture` format) |
| `RUSTIC_TIMINGS=1` | `rustic timings` over the window, report in the server log |

Prerequisites: `horizon/target/fast/rustic-server` and `horizon-bots` (crates/rusticmc-server/README.md),
the prepared game resources (docs/horizon/run.md), JDK 25 in `tools/jdk25` (or adapt `java` in
`run.sh`), Python 3 with `psutil`, a Rust toolchain with `wasm32-wasip2`.

## What a run does

1. Writes the same `server.properties` for both servers (offline, 127.0.0.1, flat world without
   structures, creative, view and simulation distance 4, `allow-flight=true`,
   `enforce-secure-profile=false`, `pause-when-empty-seconds=0` so an empty server keeps ticking) and
   `eula.txt` (`eula=true`: accepts the Minecraft EULA for this local benchmark of the user's own copy).
   Worlds, logs and plugin folders are deleted first; caches (Paper's patched jar and libraries,
   Rustic's compiled WASM cache) are kept.
2. Starts the server with its console on a file (`console.in`), times "Done (" and the ready line
   (Paper: "Done ("; Rustic: "Rustic ready", printed once the plugin runtime is up).
3. Connects N `horizon-bots` (`--behaviour walk`, one every 0.1 s): each walks at sprint speed between
   random points within 48 blocks of spawn (a move packet every tick), chats every 2 s and runs
   `/workload` every 5 s (unknown command when no workload plugin is loaded, on both servers).
4. After the warm-up, records the window: Paper's `benchprobe start/stop` (every tick's
   `ServerTickEndEvent` duration), Rustic's `horizon-report <window>` (Horizon's per-tick record),
   `tick query` on the console every 5 s on both, and `sample.py` (CPU and RSS of the server and
   of the bot process).
5. Prints the workload counters and `execute if entity` counts (all, players, slimes, items),
   `stop`, waits for the process to exit, kills leftovers, writes `result.json`.

Results go to `.local/rustic-bench/results/` (gitignored); Paper lives in `.local/paper-bench/`.
Nothing downloaded by these scripts (Paper, the vanilla jar Paperclip fetches) is ever committed.

Ports: Paper 25621, Rustic 25622, both bound to 127.0.0.1.
