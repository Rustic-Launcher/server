#!/bin/sh
# Regenerate the panel report from the jars fetched by fetch.py (run from anywhere).
#   python -I bridge/panel/fetch.py          # downloads into bridge/panel/jars (gitignored)
#   sh bridge/panel/analyze.sh               # writes report.md and report.json here
# report.json is the compact form (per plugin: the 15 most used uncovered members/types);
# pass --json without --compact to the binary for the full listing.
set -e
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
cd "$root"
cargo run --release -q -p rusticmc-bridge-analyze --bin rustic-bridge-analyze -- \
    --panel "$here/panel.json" \
    --json "$here/report.json" --compact 15 \
    --markdown "$here/report.md" --details \
    "$here/jars"
