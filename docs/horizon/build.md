# Building Horizon for Rustic (measured)

Horizon `a84f637dd` (submodule `horizon/`), Windows 11, 16 logical CPUs, rustc/cargo 1.99.1,
2026-11-09/10. Every command runs in `horizon/` with the shared target dir
`CARGO_TARGET_DIR=<server checkout>/horizon/target` (AGENTS.md: Horizon is built once
and reused by `crates/rusticmc-server`). Nothing under `horizon/` is edited.

## Commands and timings (cold target dir)

| Step | Command | Wall time | Result |
|---|---|---|---|
| Check (generates the mod API) | `cargo check -p horizon-client --no-default-features` | **3 min 46 s** | OK, no warnings. Writes `target/debug/build/horizon-client-*/out/mod_api_generated.rs` (630 KB), `mod_api_generated_host.rs` (236 KB), `mod_api_host_generated.rs` (57 KB) |
| Dedicated engine | `cargo build --profile fast -p horizon-client --no-default-features` | **12 min 20 s** (740 s) | `target/fast/horizon-client.exe`, **103 MB** (98.2 MiB) |
| Resource preparer | `cargo build --profile fast -p horizon-prepare` | 31 s (after the engine) | `target/fast/horizon-prepare.exe`, 2.2 MB |
| Load-test bots | `cargo build --profile fast -p horizon-bots` | 1 min 52 s | `target/fast/horizon-bots.exe`, 4.3 MB |

The test runs are in [run.md](run.md#tests).

`--no-default-features` is the server build (`crates/horizon-client/Cargo.toml` `[features]`: drops
`audio`, `controllers`, `horizon-extras`), the one Horizon releases as `horizon-engine`. Profile `fast`
(`horizon/Cargo.toml`: `inherits = "release"`, `lto = false`, `codegen-units = 16`, `incremental = true`)
is what the launcher uses for native-mod builds (`docs/native-mods.md`, #6304: "first build 4.5 min on a fast
Linux desktop, 7-12 min on Windows"); our 12 min matches the Windows figure. The README builds
`horizon-prepare` with `--release`; we used `--profile fast` to share the already built dependencies (same code,
no LTO), which made it a 31 s step.

## Disk

| What | Size |
|---|---|
| `horizon/target/fast` (engine + prepare + bots, with deps and incremental data) | 5.2 GB |
| `horizon/target/debug` (the `cargo check` metadata) | 1.6 GB |
| `horizon/target` total after both (before tests) | ~6.8 GB |
| `horizon/target` after the two test runs of run.md | 15 GB |
| `horizon/.local` (prepared resources and exporter scratch, gitignored in Horizon) | 587 MB |
| `.local/minecraft-home` (client jar + libraries, see run.md) | 124 MB |
| `.local/downloads` (server jar) | 60 MB |
| `tools/jdk25` (portable Temurin 25.0.5+8) | 291 MB |

A `rustic-server` executable (the engine plus Rustic linked in as a trait mod) rebuilds only
`horizon-client`'s gameplay crate and the Rustic crates on top of the deps above: expect the
engine's last steps (the `horizon-client` crate, the bulk of the 12 minutes is `horizon-client-render`,
`horizon-client-lib` and `horizon-client`) rather than a full rebuild. Not measured yet (crates/rusticmc-server
is another agent's).

## Regenerating the API reference

```sh
cd horizon && CARGO_TARGET_DIR=$PWD/target cargo check -p horizon-client --no-default-features
cd .. && python -I scripts/horizon-extract-mod-api.py        # -> docs/horizon/{hooks.md,hooks.json,host.md}
```

The script picks the newest `target/*/build/horizon-client-*/out`, reads `horizon/native-hooks.json` and the
pending fragments `horizon/tools/native-hook-fragments/*.json` (sides and contracts), and writes the three
files. Rerun it whenever the submodule moves: a diff of `hooks.json` is the upstream drift Rustic has to absorb.
