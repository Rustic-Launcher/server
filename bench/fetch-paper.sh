#!/usr/bin/env bash
# Download the latest STABLE Paper build for a Minecraft version from PaperMC's official download
# API (Fill v3) into .local/paper-bench/dist/ (gitignored), checking its SHA-256.
#
#   bench/fetch-paper.sh [VERSION]          # default 26.3
#   PAPER_BUILD=248 bench/fetch-paper.sh    # a given build instead of the latest stable
#
# Paper is GPL-3.0 software published by PaperMC; its jar (Paperclip) downloads the vanilla server
# jar from Mojang on its first start. Both stay in .local/paper-bench/ and are never committed:
# they are fetched for a local benchmark of the user's own copy (docs/PERFORMANCE.md).
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
version="${1:-26.3}"
dist="$repo/.local/paper-bench/dist"
api="https://fill.papermc.io/v3/projects/paper/versions/$version/builds"
mkdir -p "$dist"

json="$(curl -fsSL --retry 3 -H 'User-Agent: rustic-bench (local benchmark)' "$api")"
# One line "build name sha256 url" (tr: Python on Windows ends its lines with CRLF).
line="$(PAPER_BUILD="${PAPER_BUILD:-}" python -I -c '
import json, os, sys
builds = json.load(sys.stdin)
want = os.environ.get("PAPER_BUILD")
pick = [b for b in builds if (str(b["id"]) == want if want else b["channel"] == "STABLE")]
if not pick:
    sys.exit("no matching build")
b = max(pick, key=lambda b: b["id"])
d = b["downloads"]["server:default"]
print(b["id"], d["name"], d["checksums"]["sha256"], d["url"])
' <<<"$json" | tr -d '\r')"
read -r build name sha url <<<"$line"

sha256() { if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -c1-64; else shasum -a 256 "$1" | cut -c1-64; fi; }
out="$dist/paper.jar" # one copy; paper.properties names the build
if [ -f "$out" ] && [ "$(sha256 "$out")" = "$sha" ]; then
  echo "have $out"
else
  echo "fetch Paper $version build $build ($url)"
  curl -fsSL --retry 3 -o "$out.part" "$url"
  got="$(sha256 "$out.part")"
  [ "$got" = "$sha" ] || { rm -f "$out.part"; echo "SHA-256 mismatch: $got, expected $sha" >&2; exit 1; }
  mv "$out.part" "$out"
fi
printf '%s\n' "version=$version" "build=$build" "jar=$name" "sha256=$sha" >"$dist/paper.properties"
echo "Paper $version build $build -> $dist/paper.jar"
