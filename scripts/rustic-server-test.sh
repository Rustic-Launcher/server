#!/usr/bin/env bash
# Start rustic-server (Horizon's dedicated engine + Rustic) in a scratch folder, offline mode, for local
# tests (crates/rusticmc-server/README.md). Same isolation as scripts/horizon-test-server.sh: every Horizon
# home is a scratch folder, audio off, nothing self-updates.
#
#   scripts/rustic-server-test.sh [PORT]        # default port 25601, binds 127.0.0.1
#
# Folder: .local/rustic-server-test/server (gitignored), plugins in its plugins/ (dev folders
# `<id>-dev/` with rustic.plugin.toml + the .wasm, or .rplugin files). RUSTIC_DEMO_PLUGINS=1 adds the
# built-in native test plugins. Console input is stdin.
#
# eula.txt: the server refuses to start without `eula=true`. Writing it accepts the Minecraft EULA
# (https://aka.ms/MinecraftEULA) for this local test of the user's own licensed copy.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
PORT="${1:-25601}"
SCRATCH="$REPO/.local/rustic-server-test"
SERVER="$SCRATCH/server"
ENGINE="${RUSTIC_SERVER_BIN:-$REPO/horizon/target/fast/rustic-server}"
[ -x "$ENGINE" ] || ENGINE="$ENGINE.exe"
RESOURCES="$REPO/horizon/.local/horizon-client/26.3"

[ -x "$ENGINE" ] || { echo "no rustic-server: build it (crates/rusticmc-server/README.md)" >&2; exit 1; }
[ -d "$RESOURCES" ] || { echo "no prepared resources: run horizon-prepare (docs/horizon/run.md)" >&2; exit 1; }

mkdir -p "$SERVER/plugins"
for d in game launcher data config trash clipboard desktop; do mkdir -p "$SCRATCH/home/$d"; done
export HORIZON_HOME="$SCRATCH/home/game" HORIZON_LAUNCHER_HOME="$SCRATCH/home/launcher"
export XDG_DATA_HOME="$SCRATCH/home/data" XDG_CONFIG_HOME="$SCRATCH/home/config"
export HORIZON_NO_AUDIO=1 HORIZON_LAUNCHER_NO_SELF_UPDATE=1
export HORIZON_LAUNCHER_TRASH="$SCRATCH/home/trash" HORIZON_LAUNCHER_CLIPBOARD="$SCRATCH/home/clipboard"
export HORIZON_LAUNCHER_DESKTOP="$SCRATCH/home/desktop"

if [ ! -f "$SERVER/server.properties" ]; then
  cat > "$SERVER/server.properties" <<EOF
online-mode=false
server-port=$PORT
server-ip=127.0.0.1
level-name=world
level-type=minecraft\:flat
white-list=false
max-players=20
allow-flight=true
view-distance=6
simulation-distance=6
spawn-protection=0
motd=Rustic test server
EOF
fi
[ -f "$SERVER/eula.txt" ] || echo "eula=true" > "$SERVER/eula.txt"

cd "$SERVER"
exec "$ENGINE" --resources "$RESOURCES" --dedicated --nogui
