# Running Horizon's dedicated server locally (what worked)

Horizon `a84f637dd`, Windows 11, Git Bash, 2026-11-10. Every path is relative to the Rustic repo root.
No file under `horizon/` was edited; prepared resources and game files live only in gitignored folders
(`horizon/.local/`, `.local/`, `tools/jdk*`).

## 1. Game files (the user's own copy)

`horizon-prepare` reads the official launcher's folder and **never downloads the game**
(`horizon/crates/horizon-prepare/src/artifacts.rs`, `LAUNCHER_HINT`). It needs the pinned version's JSON, the
**client jar**, the client libraries (each checked against the JSON's SHA-1 and `research-lock.json`), and the
server jar (`--server-jar`). The asset store (`assets/indexes`, `objects/`) is only needed for
`horizon-prepare sounds` (optional; not done).

Found on this machine: `%APPDATA%\.minecraft\versions\26.3\26.3.json` only. **The client jar
`26.3.jar` was absent and 98 of 114 libraries were missing** (26.3 had been selected in the launcher but never
played). So:

```sh
# Client jar + client libraries listed in the user's own 26.3.json, SHA-1 checked, into a private
# launcher-shaped folder (the launcher's own folder is not touched; libraries it already has are copied):
python -I scripts/horizon-fetch-game-files.py            # -> .local/minecraft-home (124 MB)

# Server jar: downloads.server.url of the same JSON (sha1 33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c)
curl -sSL -o .local/downloads/server-26.3.jar \
  https://piston-data.mojang.com/v1/objects/33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c/server.jar
```

These are the user's licensed files from the URLs their launcher wrote; they are never committed.

## 2. JDK 25 (portable)

Only JDK 21 is installed system-wide; `horizon-prepare` runs the 26.3 exporters on Java 25
(`research-lock.json` `java_major`). Portable Eclipse Temurin from the Adoptium API:

```sh
curl -sSL "https://api.adoptium.net/v3/assets/latest/25/hotspot?architecture=x64&image_type=jdk&os=windows&vendor=eclipse"
# -> OpenJDK25U-jdk_x64_windows_hotspot_25.0.5_8.zip, sha256 636bdc13dc649ddc7d5921c5c7d23cca6e44fd19d662b45cc9c02135a6c96c42 (checked)
unzip into tools/jdk25            # tools/jdk25/bin/java -version -> 25.0.5+8 LTS
```

## 3. Prepare the resources

```sh
R=$PWD
JAVA_HOME=$R/tools/jdk25 PATH="$R/tools/jdk25/bin:$PATH" \
horizon/target/fast/horizon-prepare.exe --prepare \
  --server-jar "$R/.local/downloads/server-26.3.jar" \
  --minecraft-dir "$R/.local/minecraft-home" \
  --root "$R/horizon" --java-home "$R/tools/jdk25"
```

Result: `Prepared Horizon native-client resources: horizon/.local/horizon-client/26.3`, **Done in 70.5 s**, exit 0.
`horizon/.local` is 587 MB and is ignored by Horizon's own `.gitignore` (`/.local/`); `git -C horizon status`
stays clean.

## 4. Start a dedicated server (offline, scratch folder)

```sh
scripts/horizon-test-server.sh [PORT]      # default 25599, binds 127.0.0.1
```

What the script does (read it before running):

- Scratch homes per horizon/AGENTS.md "Keep your runs away from real player data": `HORIZON_HOME`,
  `HORIZON_LAUNCHER_HOME`, `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `HORIZON_LAUNCHER_{TRASH,CLIPBOARD,DESKTOP}` all
  under `.local/server-test/home`, plus `HORIZON_NO_AUDIO=1`, `HORIZON_LAUNCHER_NO_SELF_UPDATE=1`.
- Server folder `.local/server-test/server`: a first-run `server.properties` (`online-mode=false`,
  `server-ip=127.0.0.1`, flat world, `allow-flight=true` for the flying bots, view distance 6) and
  `eula.txt` with `eula=true`. **Writing `eula=true` accepts the Minecraft EULA** for this local test of the
  user's own copy; the engine refuses to start without it (`game/dedicated.rs` `preflight`).
- Runs the engine exactly as the supervisor does (`crates/horizon-server/src/engine.rs` `command`), minus the
  control socket: `horizon/target/fast/horizon-client.exe --resources horizon/.local/horizon-client/26.3 --dedicated --nogui`
  in the server folder.

Console input is stdin. To keep a handle on it from scripts, feed it from a file:

```sh
: > .local/server-test/console.in
(tail -f .local/server-test/console.in & echo $! > .local/server-test/tail.pid; wait) \
  | scripts/horizon-test-server.sh > .local/server-run.log 2>&1 &
echo "tick query" >> .local/server-test/console.in      # any console command
echo stop >> .local/server-test/console.in; kill $(cat .local/server-test/tail.pid)
```

Observed (`.local/server-run3.log`):

```
[Server thread/INFO]: Starting minecraft server version 26.3
[Server thread/INFO]: Starting Minecraft server on 127.0.0.1:25599
[Server thread/WARN]: **** SERVER IS RUNNING IN OFFLINE/INSECURE MODE!
[Server thread/INFO]: Preparing level "world"
[Server thread/INFO]: Done (1.788s)! For help, type "help"
```

Also printed once: `[Worker-Main-1/ERROR]: No key layers in MapLike[{}]` (the empty `generator-settings={}`
of a flat world written by hand; harmless here, the flat world generated). `stop` saves and exits with code 0.
Idle, `tick query` reports 0.3 ms average per tick (P95 0.5 ms).

## 5. A protocol client joins: `horizon-bots`

```sh
horizon/target/fast/horizon-bots.exe --server 127.0.0.1:25599 --bots 3 --behaviour walk \
  --seconds 20 --join-every 1 --name RusticBot --report .local/bots-report.json
```

Result: exit 0, `joined: 3`, no errors, join time p50 57 ms / max 63 ms, 939 chunks received, the server log shows
`RusticBot0[/127.0.0.1:...] logged in with entity id 1 at (0.5, -60.0, 0.5)`, `joined the game`, and at the end
`lost connection: Disconnected`.

Pitfalls met on the way (useful for Rustic's own end-to-end tests):

- With `allow-flight=false` (vanilla default) the bots, which fly 24 blocks above spawn by default, are kicked
  after ~4 s: `was kicked for floating too long!` (`multiplayer.disconnect.flying`).
- `--height 0 --ground` instead got them kicked by Horizon's **anticheat** (`failed ground ... claims the ground in
  the air`), which was active with no `server.toml` (the engine was run without the supervisor). Note
  `anticheat-strikes.json` appears in the server folder.
- `horizon-ctl` (agent control) drives a full game client and needs a window/GPU; not used.

The user's own launcher-managed Horizon server (`%APPDATA%\horizon-launcher\servers\test`, a separate
`horizon-client.exe --dedicated` process) was running throughout and was not touched.

## Tests

Run in `horizon/` with the shared target dir, after the builds above (dev profile, so each compiles its own
debug artefacts).

| Command | Result | Wall time (incl. build) |
|---|---|---|
| `cargo test -p horizon-server` | **168 passed, 0 failed, 1 ignored** (supervisor: config, RCON, query, management protocol, lists, updates...) | 109 s |
| `cargo test -p horizon-client --no-default-features --test mod_api_hooks` | **16 passed, 0 failed** (a mod crate's view of `mod_api`: cancellation order, write-back of mutable fields, `can_build` denial, `entity_interact_specific` claim, chat replacement...) | 373 s |

The second one builds the whole `horizon-client` library in the dev profile: `horizon/target` went from ~6.8 GB to
**15 GB**. Delete `horizon/target/debug/deps/*horizon_client*` test artefacts if disk gets tight; never
`cargo clean` the shared dir without the lead's agreement.

## rustic-server (Horizon + Rustic)

The same engine with Rustic linked in: build and run recipes, the supervisor (`engine.path`) and
`rustic run` paths, with their logs, are in [crates/rusticmc-server/README.md](../../crates/rusticmc-server/README.md)
(`scripts/rustic-server-test.sh [PORT]` is this page's script for it, default port 25601).
