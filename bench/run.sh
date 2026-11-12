#!/usr/bin/env bash
# One benchmark run (docs/PERFORMANCE.md "Rustic server vs Paper"): start a server on a fresh flat
# world, time its startup, connect N protocol bots that walk, chat and run a command on a fixed
# script, let it warm up, then record a window of ticks, CPU and memory, stop everything.
#
#   bench/run.sh --server paper|rustic [--bots N] [--plugin none|wasm|bukkit]
#                [--warmup S] [--window S] [--out DIR]
#
#   --plugin wasm    Rustic only: bench/plugins/rustic-workload as a dev folder
#   --plugin bukkit  the BenchWorkload jar: on Paper as a plugin, on Rustic through Rustic Bridge
#
# Results land in DIR (default .local/rustic-bench/results/<server>-<plugin>-<N>b-<time>):
# server.log, bots.json/bots.log, proc.json (CPU, RSS), probe.json (Paper) or report.json (Rustic),
# startup.json, then bench/summarize.py writes result.json. Needs: bench/build.sh, bench/fetch-paper.sh
# (Paper), horizon/target/fast/{rustic-server,horizon-bots}, the prepared resources
# (docs/horizon/run.md), python with psutil.
#
# eula.txt: both servers refuse to start without `eula=true`; writing it accepts the Minecraft EULA
# (https://aka.ms/MinecraftEULA) for this local benchmark of the user's own copy, as
# scripts/rustic-server-test.sh does. Paper's jar fetches the vanilla server jar from Mojang on
# its first start, into .local/paper-bench/server (never committed).
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
kind="" bots=0 plugin=none warmup=30 window=60 out=""
while [ $# -gt 0 ]; do
  case "$1" in
    --server) kind="$2"; shift 2 ;;
    --bots) bots="$2"; shift 2 ;;
    --plugin) plugin="$2"; shift 2 ;;
    --warmup) warmup="$2"; shift 2 ;;
    --window) window="$2"; shift 2 ;;
    --out) out="$2"; shift 2 ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done
case "$kind" in paper|rustic) ;; *) echo "--server paper|rustic" >&2; exit 2 ;; esac
case "$plugin" in none|bukkit) ;; wasm) [ "$kind" = rustic ] || { echo "--plugin wasm needs --server rustic" >&2; exit 2; } ;;
  *) echo "--plugin none|wasm|bukkit" >&2; exit 2 ;; esac

case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) wp() { cygpath -w "$1"; } ;; *) wp() { echo "$1"; } ;; esac
exe() { if [ -x "$1" ]; then echo "$1"; else echo "$1.exe"; fi; }
py() { python -I "$@"; }
# sample.py needs psutil, often a user-site package that -I would hide: plain python for it.
sample() { python "$repo/bench/sample.py" "$@"; }
now_ms() { date +%s%3N; }

java="$(exe "$repo/tools/jdk25/bin/java")"
botsbin="$(exe "$repo/horizon/target/fast/horizon-bots")"
build="$repo/.local/rustic-bench/build"
out="${out:-$repo/.local/rustic-bench/results/$kind-$plugin-${bots}b-$(date +%Y%m%d-%H%M%S)}"
mkdir -p "$out" && out="$(cd "$out" && pwd)"

# Heap of the Paper JVM, and Aikar's flags (https://docs.papermc.io/paper/aikars-flags), what
# Paper's documentation recommends to admins; Xms = Xmx with AlwaysPreTouch commits the heap upfront.
PAPER_HEAP="${PAPER_HEAP:-4G}"
paper_flags=(-Xms"$PAPER_HEAP" -Xmx"$PAPER_HEAP" -XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200
  -XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:+AlwaysPreTouch -XX:G1NewSizePercent=30
  -XX:G1MaxNewSizePercent=40 -XX:G1HeapRegionSize=8M -XX:G1ReservePercent=20 -XX:G1HeapWastePercent=5
  -XX:G1MixedGCCountTarget=4 -XX:InitiatingHeapOccupancyPercent=15 -XX:G1MixedGCLiveThresholdPercent=90
  -XX:SurvivorRatio=32 -XX:+PerfDisableSharedMem -XX:MaxTenuringThreshold=1
  -Dusing.aikars.flags=https://mcflags.emc.gs -Daikars.new.flags=true)
[ -n "${PAPER_JVM_FLAGS:-}" ] && read -r -a paper_flags <<<"$PAPER_JVM_FLAGS"

if [ "$kind" = paper ]; then
  dir="$repo/.local/paper-bench/server"; port=25621
  jar="$repo/.local/paper-bench/dist/paper.jar"
  [ -f "$jar" ] || { echo "no Paper: run bench/fetch-paper.sh" >&2; exit 1; }
  mkdir -p "$dir"
  rm -rf "$dir"/world* "$dir/logs" "$dir/plugins" "$dir/crash-reports"
  mkdir -p "$dir/plugins/bStats"
  # No usage statistics leave the machine.
  printf 'enabled: false\nserverUuid: 00000000-0000-0000-0000-000000000000\nlogFailedRequests: false\n' \
    >"$dir/plugins/bStats/config.yml"
  cp "$build/BenchProbe.jar" "$dir/plugins/"
  [ "$plugin" = bukkit ] && cp "$build/BenchWorkload.jar" "$dir/plugins/"
  cmd=("$java" "${paper_flags[@]}" -jar "$(wp "$jar")" --nogui)
  match="$(basename "$jar")"
  ready='Done ('
else
  dir="$repo/.local/rustic-bench/server"; port=25622
  engine="$(exe "$repo/horizon/target/fast/rustic-server")"
  resources="$repo/horizon/.local/horizon-client/26.3"
  [ -d "$resources" ] || { echo "no prepared resources (docs/horizon/run.md)" >&2; exit 1; }
  home="$repo/.local/rustic-bench/home"
  for d in game launcher data config trash clipboard desktop; do mkdir -p "$home/$d"; done
  export HORIZON_HOME="$home/game" HORIZON_LAUNCHER_HOME="$home/launcher"
  export XDG_DATA_HOME="$home/data" XDG_CONFIG_HOME="$home/config"
  export HORIZON_NO_AUDIO=1 HORIZON_LAUNCHER_NO_SELF_UPDATE=1
  export HORIZON_LAUNCHER_TRASH="$home/trash" HORIZON_LAUNCHER_CLIPBOARD="$home/clipboard"
  export HORIZON_LAUNCHER_DESKTOP="$home/desktop"
  mkdir -p "$dir"
  rm -rf "$dir"/world* "$dir/logs" "$dir/plugins" "$dir/reports" "$dir/crash-reports" "$dir/rustic-bridge.toml"
  mkdir -p "$dir/plugins"
  if [ "$plugin" = wasm ]; then
    mkdir -p "$dir/plugins/bench-workload-dev"
    cp "$repo/bench/plugins/rustic-workload/rustic.plugin.toml" "$dir/plugins/bench-workload-dev/"
    cp "$repo/plugins/target/wasm32-wasip2/release/bench_workload.wasm" "$dir/plugins/bench-workload-dev/"
  elif [ "$plugin" = bukkit ]; then
    mkdir -p "$dir/plugins/bukkit"
    cp "$build/BenchWorkload.jar" "$dir/plugins/bukkit/"
    # The sidecar JVM: Rustic Bridge's home (bridge/java/build.sh), heap as bridge/java/e2e.sh.
    bridge_args="${BRIDGE_JVM_ARGS:-'"-Xmx512m"'}" # TOML array items
    cat >"$dir/rustic-bridge.toml" <<TOML
java = "$(wp "$java" | tr '\\' '/')"
home = "$(wp "$repo/bridge/java/build" | tr '\\' '/')"
jvm_args = [$bridge_args]
TOML
  fi
  cmd=("$engine" --resources "$(wp "$resources")" --dedicated --nogui)
  match="$(basename "$engine")"
  ready='Rustic ready'
fi

cat >"$dir/server.properties" <<EOF
online-mode=false
server-ip=127.0.0.1
server-port=$port
level-name=world
level-type=minecraft\:flat
generate-structures=false
gamemode=creative
max-players=200
allow-flight=true
view-distance=4
simulation-distance=4
spawn-protection=0
enforce-secure-profile=false
pause-when-empty-seconds=0
white-list=false
motd=Rustic bench
EOF
echo "eula=true" >"$dir/eula.txt"

console() { echo "$1" >>"$out/console.in"; }
wait_for() { # pattern seconds
  local end=$((SECONDS + $2))
  while [ $SECONDS -lt $end ]; do grep -qF "$1" "$out/server.log" 2>/dev/null && return 0; sleep 0.1; done
  echo "timed out waiting for: $1" >&2; return 1
}
cleanup() {
  [ -n "${botpid:-}" ] && kill "$botpid" 2>/dev/null || true
  sample --match "$match" --kill >/dev/null 2>&1 || true
  # The `tail -f` feeding the console (no pkill in Git Bash: scan /proc).
  { for c in /proc/[0-9]*/cmdline; do
    if tr '\0' ' ' <"$c" | grep -qF "tail -f $out/console.in"; then
      p="${c#/proc/}"; kill "${p%/cmdline}" || true
    fi
  done; } 2>/dev/null
}
trap cleanup EXIT

: >"$out/console.in"
echo "[$kind/$plugin/${bots} bots] starting in $dir -> $out"
t0="$(now_ms)"
(cd "$dir" && exec "${cmd[@]}" < <(tail -f "$out/console.in") >"$out/server.log" 2>&1) &
wait_for 'Done (' 600
t_done="$(now_ms)"
wait_for "$ready" 120
t_ready="$(now_ms)"
printf '{"done_ms": %d, "ready_ms": %d}\n' $((t_done - t0)) $((t_ready - t0)) >"$out/startup.json"
echo "  ready after $((t_ready - t0)) ms"
# GAMERULES="locator_bar=false ...": set on the console before the bots join (same on both servers).
for rule in ${GAMERULES:-}; do console "gamerule ${rule%%=*} ${rule#*=}"; done
sleep 5

join_every=0.1
join_s="$(py -c "print(int($bots * $join_every + 0.999))")"
if [ "$bots" -gt 0 ]; then
  bot_seconds=$((join_s + warmup + window + 15))
  extra=()
  # BOT_CHAT=0: no chat or commands (moves only). BOT_CAPTURE=1: the first bot's packets.
  if [ "${BOT_CHAT:-1}" = 1 ]; then
    extra+=(--raw "$(py "$repo/bench/botscript.py" --from-tick 100 --to-tick $((bot_seconds * 20)))")
  fi
  [ "${BOT_CAPTURE:-0}" = 1 ] && extra+=(--capture "$(wp "$out/capture.bin")")
  "$botsbin" --server "127.0.0.1:$port" --bots "$bots" --behaviour "${BOT_BEHAVIOUR:-walk}" \
    --join-every "$join_every" --seconds "$bot_seconds" --name Bench --seed 12345 "${extra[@]}" \
    --report "$(wp "$out/bots.json")" >"$out/bots.log" 2>&1 &
  botpid=$!
fi
sleep $((join_s + warmup))

echo "  recording $window s"
if [ "$kind" = paper ]; then console "benchprobe start"; else console "horizon-report $window"; fi
# RUSTIC_TIMINGS=1: Rustic's per-plugin timings over the window too (they add a little work).
[ "$kind" = rustic ] && [ "${RUSTIC_TIMINGS:-0}" = 1 ] && { console "rustic timings reset"; console "rustic timings on"; }
sample --match "$match" --seconds "$window" --out "$out/proc.json" >/dev/null &
samplepid=$!
# The load generator runs on the same machine: its CPU too.
[ "$bots" -gt 0 ] && { sample --match horizon-bots --seconds "$window" --out "$out/bots-proc.json" >/dev/null & }
for _ in $(seq 1 $((window / 5))); do sleep 5; console "tick query"; done
wait "$samplepid" || true
if [ "$kind" = paper ]; then
  console "benchprobe stop $(wp "$out/probe.json" | tr '\\' '/')"
  for _ in $(seq 1 100); do [ -s "$out/probe.json" ] && break; sleep 0.1; done
else
  for _ in $(seq 1 300); do ls "$dir"/reports/perf-*.json >/dev/null 2>&1 && break; sleep 0.1; done
  sleep 1
  cp "$(ls -t "$dir"/reports/perf-*.json | head -1)" "$out/report.json"
fi
[ "$kind" = rustic ] && [ "${RUSTIC_TIMINGS:-0}" = 1 ] && console "rustic timings report"
[ "$plugin" != none ] && console "workload" # the counters, in the log (console sender)
# What the world held at the end (both answer "Test passed, count: N"): entities, then mobs by kind.
for sel in '@e' '@e[type=minecraft:player]' '@e[type=minecraft:slime]' '@e[type=minecraft:item]'; do
  console "execute if entity $sel"
done
sleep 2
console "stop"
wait_for_exit() { for _ in $(seq 1 600); do sample --match "$match" --seconds 0 >/dev/null 2>&1 || return 0; sleep 0.1; done; }
wait_for_exit
[ -n "${botpid:-}" ] && { kill "$botpid" 2>/dev/null || true; wait "$botpid" 2>/dev/null || true; }
cat >"$out/meta.json" <<EOF
{"server": "$kind", "plugin": "$plugin", "bots": $bots, "warmup_s": $warmup, "window_s": $window,
 "join_every_s": $join_every, "paper_heap": "$PAPER_HEAP", "bot_chat": ${BOT_CHAT:-1},
 "bot_behaviour": "${BOT_BEHAVIOUR:-walk}", "gamerules": "${GAMERULES:-}"}
EOF
py "$repo/bench/summarize.py" run "$out"
