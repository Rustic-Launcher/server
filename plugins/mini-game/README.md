# mini-game

Arena mini-games for Rustic servers, written to exercise the scheduler and world APIs: **Spleef**
and **King of the Hill**, with a lobby countdown (boss bar, titles, sounds), timed rounds with a
live sidebar, world changes, stats, a clickable leaderboard and optional economy rewards.
English and French.

## Modes

* **Spleef**: a disc of `floor_block` (snow) under the arena, rebuilt with one `set_blocks`
  batch when the arena is created, before each round and after it. Players get a marked
  Efficiency V shovel and survival mode; only floor blocks can be broken (tracked, no drops,
  each one a point). Falling below the floor eliminates (checked every tick from player
  locations): the player goes back to the lobby. The last one standing wins; when the time is up,
  the best digger still standing wins (a tie is a draw).
* **King of the Hill**: a round zone (radius around the block under the creator, up to 4 blocks
  above it) checked **every tick** from player locations; standing alone in it scores a point
  per second (contested: nobody scores); particles (`minecraft:dust`, gold held / red contested /
  white free) draw its border twice a second; an action bar says who holds it. First to
  `points_to_win` wins, else the best score when the time is up.

## Flow

1. `/mg join [arena]` (the fullest open arena without a name): teleport to the lobby, a boss bar
   "Waiting for players 1/2".
2. At `min_players`: a countdown of `countdown_seconds` (boss bar progress, a title and a tick
   sound for the last 5 seconds). Dropping below the minimum stops it.
3. Start: players are spread around the start point, game mode set (survival for Spleef,
   adventure for KotH), "Go!" title, a sidebar `mini-game.g_<arena>` with every score and a
   "Time left" line, updated every second by `Scheduler::run_timer`; the boss bar shows the
   clock. A per-tick timer runs only while some round is played.
4. End: "Victory!" / "Game over" titles with the winner, a sound, stats saved
   (`stats/<uuid>`: wins, games, points), rewards paid through the `economy` service when a
   provider is installed (`[rewards] play` to everyone, `+ win` to the winner; none otherwise).
5. After `end_seconds`: everyone goes back to the lobby with their previous game mode, the
   shovels are taken back, the sidebar and boss bars removed, the Spleef floor restored.

Inside a game players take no damage and cannot place blocks; arena floors with a game in
progress cannot be broken by anyone else.

**Disconnects**: a player who quits leaves the game (Spleef: the last one standing may win
then); when they come back they are put in the lobby, without the game's items, in their
previous game mode. **Disabling the plugin** (or `/rustic reload mini-game`, or a shutdown)
mid-game ends every game the same way: players to the lobby, items taken back, modes restored,
bars and sidebars removed, floors rebuilt.

## Commands

All under `/mg` (alias `/minigame`):

| Command | Permission | |
|---|---|---|
| `/mg`, `/mg help` | `minigame.play` | the command list (admin lines for admins) |
| `/mg join [arena]` | `minigame.play` | |
| `/mg leave` | `minigame.play` | |
| `/mg list` | `minigame.play` | arenas, mode, state, players; click a line to join |
| `/mg stats [player]` | `minigame.play` | wins, games, win rate, points (online or offline players) |
| `/mg top [wins\|games\|points]` | `minigame.play` | the ten best; click a line for that player's stats |
| `/mg create <arena> [spleef\|koth]` | `minigame.admin` | at your position: start point and lobby here, floor/zone centred on the block under you, `default_radius` |
| `/mg setspawn <arena>` | `minigame.admin` | start point = your position |
| `/mg setlobby <arena>` | `minigame.admin` | lobby = your position |
| `/mg setarea <arena> [radius]` | `minigame.admin` | floor/zone centre = the block under you (0..30) |
| `/mg delete <arena>` | `minigame.admin` | refused while a game exists |
| `/mg start <arena>` | `minigame.admin` | starts the countdown (at most 5 s) whatever the number of players (at least one) |
| `/mg stop <arena>` | `minigame.admin` | ends the game without a winner (no stats), everyone back to the lobby |

Arena names: `a-z`, `0-9`, `_`, 16 at most. Arenas are stored under `arenas/<name>`. Arena and
player names complete with Tab.

## Permissions

| Node | Default |
|---|---|
| `minigame.play` | everyone |
| `minigame.admin` | op |

## Configuration (`plugins/mini-game/config.toml`)

See the shipped [`config.toml`](config.toml) (`config_version = 1`): `language`, `min_players`,
`max_players`, `countdown_seconds`, `round_seconds`, `end_seconds`, `default_radius`,
`[spleef] floor_block, tool`, `[koth] points_to_win, particle`, `[rewards] win, play` (economy
minor units).

## Limits

* The scoreboard API has one server-wide sidebar: everyone sees the sidebar of the round that
  started last. A per-player sidebar (or the team-coloured sidebar slots) is not in
  `rustic:api` 0.1.
* On Rustic Server, `set-block` with `physics = false` is ignored (the floor uses physics-on
  batches anyway).

## Build and test

```sh
cd plugins/mini-game
cargo build --target wasm32-wasip2 --release        # ../target/wasm32-wasip2/release/mini_game.wasm
cargo test                                          # unit tests + tests/plugin.rs on the real .wasm
../../target/debug/rustic build --release && ../../target/debug/rustic check ../target/rustic/mini-game.rplugin --deny-warnings
```

Release component 750.8 KiB, `.rplugin` 267.3 KiB.

## Live check (2026-11-11, rustic-server `--profile fast`, two `horizon-bots`)

`.rplugin`s of chat-format, menus, mini-game and perms in `.local/rustic-server-test/server/plugins/`,
`config.toml` with a 5 s countdown, 40 s rounds, 5 points to win. Bot Alpha0 (op) ran
`/mg create hill koth` and `/mg join hill`, Beta0 `/mg join hill`; the console moved Beta0 out
of the zone after the start. Server log:

```
[06:15:28] [Server thread/INFO]: [rustic] [mini-game] mini-game enabled: 0 arenas, economy rewards: false
[06:16:48] [Server thread/INFO]: System chat: Teleported Beta0 to 40.500000, -60.000000, 40.500000
[06:17:27] [Server thread/INFO]: [rustic] [mini-game] disabled (reloading)
[06:17:27] [Server thread/INFO]: [rustic] [mini-game] mini-game enabled: 1 arenas, economy rewards: false
[06:17:27] [Server thread/INFO]: [rustic] reloaded mini-game
```

What Alpha0's client received (`--capture`, seconds since it joined; summary of the packets):

```
  19.10s SystemChat           Arena hill (King of the Hill) created here, radius 8. ...
  20.10s SystemChat           You joined hill (1/8).
  20.10s BossEvent            Waiting for players 1/2
  20.95s SystemChat           Beta0 joined (2/8).
  20.95s BossEvent            Starting in 5 s
  21.65s Title / Subtitle     4 / Get ready!          (and a note-block sound, each second to 1)
  25.65s Title / Subtitle     Go! / Hold the zone alone to score!
  25.70s SetObjective         mini-game.g_hill | King of the Hill hill
  25.70s SetDisplayObjective  mini-game.g_hill
  25.70s SetScore             Alpha0 / Beta0
  25.70s BossEvent            King of the Hill · 0:40 left
  26.65s ActionBar            Contested!              (both bots in the zone)
  32.65s ActionBar            King: Alpha0 (0/5)      (Beta0 teleported away)
  33.65s SetScore             Alpha0                  (1, 2, 3, 4, 5 every second)
  37.65s BossEvent            Winner: Alpha0
  39.70s SetDisplayObjective  (cleared), SetObjective mini-game.g_hill (removed), BossEvent (removed)
```

Second session, the stats and the clickable top:

```
  23.11s SystemChat           Stats of Alpha0: Wins 1 · games 1 · win rate 100% · points 5
  24.11s SystemChat           [run_command /mg stats Alpha0] #1 Alpha0 1 wins · 1 games · 5 points
  24.11s SystemChat           [run_command /mg stats Beta0] #2 Beta0 0 wins · 1 games · 0 points
  25.11s SystemChat           [run_command /mg join hill] - hill King of the Hill · waiting · 0/8
```

Spleef rounds (floor batch, digging, falls, restore) are covered by the integration tests; the
bots were not scripted to dig.
