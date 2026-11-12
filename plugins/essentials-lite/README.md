# Essentials Lite

The everyday commands of a survival server: spawn, homes, warps, teleport requests, `/back`,
private messages and broadcasts, with a teleport warmup (cancelled when the player moves) and a
cooldown. Messages in English or French. Concepts follow the classic "essentials" plugins; the
code and texts are written from scratch for Rustic.

It uses the standard services when a provider is installed and degrades gracefully otherwise:

* **chat-meta** (e.g. [Perms](../perms/README.md)): the sender's and receiver's prefix in private
  messages (`[[Staff] Alice0 → me] hi`); without a provider, the bare name;
* **economy**: the optional price of a new home (`sethome_cost`); without a provider, homes are
  free.

## Commands

| Command | Permission (default) | What |
|---|---|---|
| `/spawn` | `essentials.spawn` (everyone) | teleport to the spawn (the world's until `/setspawn`) |
| `/setspawn` | `essentials.setspawn` (op) | set the spawn where you stand (also the world spawn) |
| `/home [name]`, alias `/h` | `essentials.home` (everyone) | go home; without a name: your only home, else `home`, else the list |
| `/homes` | `essentials.home` | your homes, clickable, with the count and your limit |
| `/sethome [name]` | `essentials.sethome` (everyone) | set or move a home (default name `home`) |
| `/delhome <name>` | `essentials.sethome` | delete a home |
| `/warp <name>` | `essentials.warp` (everyone) | go to a warp |
| `/warps` | `essentials.warp` | the warps, clickable (only those you may use) |
| `/setwarp <name>` | `essentials.setwarp` (op) | create or move a warp where you stand |
| `/delwarp <name>` | `essentials.delwarp` (op) | delete a warp (console too) |
| `/tpa <player>` | `essentials.tpa` (everyone) | ask to teleport to a player |
| `/tpahere <player>` | `essentials.tpahere` (everyone) | ask a player to teleport to you |
| `/tpaccept [player]`, alias `/tpyes` | `essentials.tpaccept` (everyone) | accept the newest request (or that player's) |
| `/tpdeny [player]`, alias `/tpno` | `essentials.tpaccept` | deny it |
| `/back` | `essentials.back` (everyone) | return to where you were before your last teleport or death |
| `/msg <player> <message>`, aliases `/m`, `/pm`, `/whisper` | `essentials.msg` (everyone) | private message (console too) |
| `/r <message>`, alias `/reply` | `essentials.msg` | answer the last private message |
| `/broadcast <message>`, alias `/bc` | `essentials.broadcast` (op) | announce to everyone (MiniMessage allowed) |

Home, warp and request arguments tab-complete. A teleport request shows clickable
**[Accept] [Deny]** buttons and expires after `tpa_timeout_seconds` (both players are told);
requests are dropped when either player leaves. `/back` also remembers teleports made by anything
else (vanilla `/tp`, other plugins) and is saved with the player.

Other permissions:

| Node | Default | |
|---|---|---|
| `essentials.homes.<n>` for each `home_tiers` value | no one | up to `n` homes (the largest tier held wins) |
| `essentials.homes.unlimited` | op | no home limit |
| `essentials.sethome.free` | op | new homes cost nothing |
| `essentials.warps.<name>` | op | a warp, only when `per_warp_permissions = true` (grant `essentials.warps.*`) |
| `essentials.warmup.bypass` | op | teleport at once |
| `essentials.cooldown.bypass` | op | no cooldown |

A permission provider's wildcard (`essentials.*`) grants the bypass and unlimited nodes too; negate
the ones you do not want (`-essentials.warmup.bypass`).

## Configuration (`plugins/essentials-lite/config.toml`)

```toml
config_version = 1
lang = "en"                      # "en" or "fr"
teleport_warmup_seconds = 3      # stand still this long before /spawn /home /warp /back /tpa (0 = instant)
warmup_cancel_distance = 0.5     # moving farther (blocks) cancels the teleport
teleport_cooldown_seconds = 5    # between two teleports of a player (0 = none)
tpa_timeout_seconds = 60
default_max_homes = 1
home_tiers = [3, 5, 10]          # essentials.homes.<n>
sethome_cost = 0                 # minor units of the economy (100 = 1.00); 0 = free; needs an economy plugin
per_warp_permissions = false
broadcast_format = "<dark_gray>[<gold>Broadcast</gold>]</dark_gray> <white>{message}"
back_on_death_hint = true        # "Use /back to return to where you died."
```

Missing keys take these defaults. Times are counted in server ticks (20 a second).

## Data

* Spawn and warps: plugin storage (`spawn`, `warps`, JSON locations).
* Homes and the `/back` location: per-player data (`homes`, `back`), saved with the player.
* Requests, warmups, cooldowns and reply targets live in memory (lost on restart, by design).

## On rustic-server (Horizon)

* A plugin command of a vanilla name replaces vanilla's, as on Paper, so **`/msg` is this
  plugin's** (with `/m`, `/pm`, `/whisper`, `/r` and the click-to-reply); `/tell` and `/w`, which
  it does not register, stay vanilla's.
* Horizon hides commands a player has no permission for, so a denied command looks like an
  unknown one to that player.
* The plugin's own teleports do not reach its `player-teleport` handler (the runtime never
  re-enters a plugin; logged once as a warning); it records `/back` itself before teleporting.

## Build, test, size

```sh
cargo build --manifest-path plugins/Cargo.toml -p essentials-lite --target wasm32-wasip2 --release
cargo test  --manifest-path plugins/Cargo.toml -p essentials-lite   # 12 unit + 1 manifest + 15 integration
target/debug/rustic package plugins/essentials-lite                  # plugins/target/rustic/essentials-lite.rplugin
```

Component 659 KiB, `.rplugin` 236 KiB (release, `opt-level = "s"`, LTO, stripped).

The integration tests (`tests/plugin.rs`) run the real component in `rusticmc-test`: every
command and its permission denial, homes limits and tiers, clickable lists, completion, warps with
and without per-warp permissions, `/tpa` / `/tpahere` / accept / deny / expiry / quit, `/back`
after a teleport, a vanilla teleport and a death, warmup (wait, small moves, cancel, replace,
bypass), cooldown, `/msg` / `/r` with the console, broadcast, the Perms prefix and permissions
(with the real `perms.wasm`), the paid `/sethome` (with `tests/fixtures/economy`, an economy whose
accounts start at 150 coins) and the French messages.

## Live check (rustic-server on Horizon, 2026-11-11)

Two bots (`Alice0` in Perms' `staff` group with prefix `&c[Staff]`, `Bob0`), commands sent as raw
chat-command packets; system chat taken from their packet captures:

```
Alice0  2.10s  [Ess] Home base set.
Alice0  3.08s  [Ess] Your homes (1/∞), click one:   [base] (run_command /home base, hover "overworld 0 -60 0")
Alice0  4.08s  [Ess] Warp plaza set.
Alice0  6.08s  [Ess] Teleporting to spawn in 3s. Don't move!
Alice0  9.03s  [Ess] Teleported to spawn.
Alice0 11.08s  [Ess] Wait 3s before teleporting again.                 <- cooldown 5 s
Alice0 15.08s  [Ess] Request sent to Bob0. It expires in 60s.
Bob0   13.01s  [Ess] Alice0 wants to teleport to you.  [Accept] [Deny] (60s)  (run_command /tpaccept Alice0)
Bob0   16.06s  [Ess] You accepted the request from Alice0.
Alice0 18.13s  [Ess] Teleporting to Bob0 in 3s. Don't move!
Alice0 21.08s  [Ess] Teleported to Bob0.
Bob0    5.06s  [Ess] Warp x set.            6.06s  [Ess] Warp x deleted.
Bob0   32.01s  [Broadcast] Live test done
Bob0   35.01s  [Ess] Teleported to warp plaza.
Alice0  9.02s  [Ess] Teleported to your previous location.            <- /back (second run)
Alice0 10.07s  [Ess] Home base deleted.
Bob0   20.09s  [Ess] You denied the request from Alice0.     Alice0: [Ess] Bob0 denied your request.
Bob0   21.09s  [Ess] There is no warp named nowhere.
Bob0    7.18s  [[Staff] Alice0 → me] hello from Alice                  <- /m, prefix from Perms' chat-meta
Alice0  7.99s  [Bob0 → me] hi Alice                                    <- /r
Bob0   15.18s  [[Staff] Alice0 → me] third                             <- /whisper
```

Server log (`.local/batchA-live.log`): `[essentials-lite] essentials-lite enabled: 0 warps, spawn
from the world, warmup 3s, cooldown 5s, lang En`, then a hot reload with `rustic reload
essentials-lite` (`reloaded essentials-lite`, `1 warps` kept).
