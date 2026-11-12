#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
#
# Builds the Rustic Bridge Java layer without Gradle or Maven: fetches the Paper API and its
# libraries (deps.txt, checked by SHA-256) into build/libs, compiles with javac --release 25,
# runs the unit tests, and packages
#   build/rustic-bridge.jar        the bridge (Main-Class rustic.bridge.Main)
#   build/libs/*.jar               its runtime classpath (the Paper API, Adventure, Gson...)
#   build/plugins/*.jar            the test plugins (ordinary Bukkit plugin jars)
#
# Usage: bridge/java/build.sh [--offline] [--skip-tests]
# JDK: $JAVA_HOME, else <repo>/tools/jdk25, else javac on PATH (25 or newer).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
build="$here/build"
offline=0
tests=1
for a in "$@"; do
  case "$a" in
    --offline) offline=1 ;;
    --skip-tests) tests=0 ;;
    *) echo "unknown option $a" >&2; exit 2 ;;
  esac
done

if [ -n "${JAVA_HOME:-}" ] && [ -x "$JAVA_HOME/bin/javac" -o -x "$JAVA_HOME/bin/javac.exe" ]; then
  jbin="$JAVA_HOME/bin/"
elif [ -d "$repo/tools/jdk25/bin" ]; then
  jbin="$repo/tools/jdk25/bin/"
else
  jbin=""
fi
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) sep=";"; winpath() { cygpath -w "$1"; } ;;
  *) sep=":"; winpath() { echo "$1"; } ;;
esac
# javac @argfiles: one source per line, in the JDK's own path syntax.
sources() { find "$1" -name '*.java' | while read -r f; do winpath "$f"; done >"$2"; }
ver="$("${jbin}java" -XshowSettings:properties -version 2>&1 | sed -n 's/^ *java.specification.version = //p')"
if [ "${ver%%.*}" -lt 25 ] 2>/dev/null; then
  echo "error: JDK 25 or newer needed (found $ver); set JAVA_HOME" >&2
  exit 1
fi

repo_url="https://repo.papermc.io/repository/maven-public"
sha() { if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -c1-64; else shasum -a 256 "$1" | cut -c1-64; fi; }

fetch() { # scope coord sha dir
  local coord="$2" want="$3" dir="$4"
  IFS=: read -r g a v <<<"$coord"
  local file="$dir/$a-$v.jar"
  if [ -f "$file" ] && [ "$(sha "$file")" = "$want" ]; then return; fi
  if [ "$offline" = 1 ]; then echo "error: $file missing (offline)" >&2; exit 1; fi
  local url="$repo_url/${g//.//}/$a/$v/$a-$v.jar"
  echo "fetch $coord"
  curl -fsSL --retry 3 -o "$file.part" "$url"
  local got; got="$(sha "$file.part")"
  if [ "$got" != "$want" ]; then
    rm -f "$file.part"
    echo "error: $coord: SHA-256 $got, expected $want" >&2
    exit 1
  fi
  mv "$file.part" "$file"
}

mkdir -p "$build/libs" "$build/test-libs"
while read -r scope coord hash; do
  case "$scope" in ''|\#*) continue ;; esac
  if [ "$scope" = runtime ]; then fetch "$scope" "$coord" "$hash" "$build/libs"
  else fetch "$scope" "$coord" "$hash" "$build/test-libs"; fi
done <"$here/deps.txt"

cp_libs="$(winpath "$build/libs")/*"
javac_flags=(--release 25 -encoding UTF-8 -Xlint:all,-processing,-serial,-classfile,-deprecation,-removal -Xmaxwarns 200)

echo "javac bridge"
rm -rf "$build/classes" && mkdir -p "$build/classes"
sources "$here/src/main/java" "$build/sources.txt"
(cd "$build" && "${jbin}javac" "${javac_flags[@]}" -cp "$cp_libs" -d classes @sources.txt)
cp -r "$here/src/main/resources/." "$build/classes/"
cp "$here/COPYING" "$build/classes/COPYING.rustic-bridge" 2>/dev/null || true
printf 'Main-Class: rustic.bridge.Main\nImplementation-Title: Rustic Bridge\nImplementation-Version: 0.1.0\n' >"$build/manifest.txt"
(cd "$build" && "${jbin}jar" --create --file rustic-bridge.jar --manifest manifest.txt -C classes .)

# Test plugins: each test-plugins/<dir> has src/ (Java) and resources/ (plugin.yml, config.yml).
# A plugin with a `shade.txt` lists jars of build/test-libs to unpack into it (a shaded jar).
mkdir -p "$build/plugins"
for dir in "$here"/test-plugins/*/; do
  name="$(basename "$dir")"
  out="$build/plugin-classes/$name"
  rm -rf "$out" && mkdir -p "$out"
  sources "$dir/src" "$build/plugin-sources.txt"
  echo "javac plugin $name"
  (cd "$build" && "${jbin}javac" "${javac_flags[@]}" -cp "$cp_libs" -d "plugin-classes/$name" @plugin-sources.txt)
  cp -r "$dir/resources/." "$out/"
  if [ -f "$dir/shade.txt" ]; then
    while read -r jarname; do
      [ -z "$jarname" ] && continue
      (cd "$out" && "${jbin}jar" --extract --file "$(winpath "$build/test-libs/$jarname")")
    done <"$dir/shade.txt"
    rm -rf "$out/META-INF/MANIFEST.MF"
  fi
  jarname="$(sed -n 's/^name: *//p' "$dir/resources/plugin.yml" | tr -d '\r' | head -1)"
  (cd "$build" && "${jbin}jar" --create --file "plugins/$jarname.jar" -C "plugin-classes/$name" .)
done

if [ "$tests" = 1 ]; then
  echo "javac tests"
  rm -rf "$build/test-classes" && mkdir -p "$build/test-classes"
  sources "$here/src/test/java" "$build/test-sources.txt"
  (cd "$build" && "${jbin}javac" "${javac_flags[@]}" -cp "classes$sep$cp_libs" -d test-classes @test-sources.txt)
  echo "run tests"
  (cd "$build" && "${jbin}java" -cp "test-classes${sep}classes${sep}$cp_libs" \
    -Drustic.test.plugins="$(winpath "$build/plugins")" rustic.bridge.TestMain)
fi
echo "built $build/rustic-bridge.jar ($(ls "$build/libs" | wc -l | tr -d ' ') libraries, $(ls "$build/plugins" | wc -l | tr -d ' ') test plugins)"
