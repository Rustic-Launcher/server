#!/usr/bin/env bash
# Build the benchmark plugins (docs/PERFORMANCE.md):
#   plugins/target/wasm32-wasip2/release/bench_workload.wasm   Rustic WASM workload
#   .local/rustic-bench/build/BenchWorkload.jar                 Bukkit workload (Paper and Rustic Bridge)
#   .local/rustic-bench/build/BenchProbe.jar                    Paper tick recorder
#
# The Java plugins compile against the Paper API jar that bridge/java/build.sh fetched
# (SHA-256 checked, bridge/java/deps.txt): run that first.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
out="$repo/.local/rustic-bench/build"
libs="$repo/bridge/java/build/libs"
jbin="$repo/tools/jdk25/bin/"
[ -x "${jbin}javac" ] || [ -x "${jbin}javac.exe" ] || jbin=""
case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) wp() { cygpath -w "$1"; } ;; *) wp() { echo "$1"; } ;; esac

echo "build bench-workload (wasm32-wasip2, release)"
CARGO_TARGET_DIR="$repo/plugins/target" cargo build --quiet \
  --manifest-path "$repo/bench/plugins/rustic-workload/Cargo.toml" --target wasm32-wasip2 --release

ls "$libs"/paper-api-*.jar >/dev/null 2>&1 || { echo "no Paper API in $libs: run bridge/java/build.sh" >&2; exit 1; }
mkdir -p "$out"
for name in workload probe; do
  src="$repo/bench/bukkit/$name"
  classes="$out/classes-$name"
  rm -rf "$classes" && mkdir -p "$classes"
  find "$src/src" -name '*.java' | while read -r f; do wp "$f"; done >"$out/sources-$name.txt"
  echo "javac $name"
  (cd "$out" && "${jbin}javac" --release 25 -encoding UTF-8 -Xlint:all,-processing,-deprecation \
    -cp "$(wp "$libs")/*" -d "classes-$name" "@sources-$name.txt")
  cp "$src/resources/plugin.yml" "$classes/"
  jar="$(sed -n 's/^name: *//p' "$src/resources/plugin.yml" | tr -d '\r')"
  (cd "$out" && "${jbin}jar" --create --file "$jar.jar" -C "classes-$name" .)
done
echo "built: $repo/plugins/target/wasm32-wasip2/release/bench_workload.wasm, $out/BenchWorkload.jar, $out/BenchProbe.jar"
