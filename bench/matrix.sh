#!/usr/bin/env bash
# Every scenario of docs/PERFORMANCE.md "Rustic server vs Paper", REPS times (default 3), servers and
# scenarios interleaved so slow drift on the machine hits both alike; then the tables.
#
#   bench/matrix.sh [REPS]
#   BOT_COUNTS="0 10 50 100" PLUGIN_BOTS=50 WINDOW=60 WARMUP=30 bench/matrix.sh 3
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
reps="${1:-3}"
counts="${BOT_COUNTS:-0 10 50 100}"
plugin_bots="${PLUGIN_BOTS:-50}"
window="${WINDOW:-60}"
warmup="${WARMUP:-30}"
root="$repo/.local/rustic-bench/results/matrix-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$root"

one() { # server plugin bots rep
  local d="$root/$1-$2-$3b-r$4"
  if ! "$repo/bench/run.sh" --server "$1" --plugin "$2" --bots "$3" --window "$window" --warmup "$warmup" \
    --out "$d" >"$d.out" 2>&1; then
    echo "run failed: $d (see $d.out)" >&2
  fi
  grep -h "bots:" "$d.out" || tail -1 "$d.out"
}

for rep in $(seq 1 "$reps"); do
  for n in $counts; do
    one rustic none "$n" "$rep"
    one paper none "$n" "$rep"
  done
  one rustic wasm "$plugin_bots" "$rep"
  one rustic bukkit "$plugin_bots" "$rep"
  one paper bukkit "$plugin_bots" "$rep"
done
python -I "$repo/bench/summarize.py" table "$root"/*/ | tee "$root/tables.md"
echo "results: $root"
