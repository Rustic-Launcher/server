#!/usr/bin/env bash
# Start Horizon's dedicated engine in a scratch folder, offline mode, for local tests (docs/horizon/run.md).
#
#   scripts/horizon-test-server.sh [PORT]          # default port 25599
#
# Follows horizon/AGENTS.md "Keep your runs away from real player data": every Horizon home is a fresh
# scratch folder under .local/server-test/home, audio is off, nothing self-updates. The server folder
# (world, server.properties, logs) is .local/server-test/server (gitignored). The engine is the one
# `cargo build --profile fast -p horizon-client --no-default-features` built in horizon/target/fast,
# run as horizon-server's supervisor runs it (crates/horizon-server/src/engine.rs `command`):
# `--resources <prepared> --dedicated --nogui`, in the server folder.
#
# eula.txt: the server refuses to start without `eula=true`. Writing it means accepting the Minecraft
# EULA (https://aka.ms/MinecraftEULA) for this local test of the user's own licensed copy.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
PORT="${1:-25599}"
SCRATCH="$REPO/.local/server-test"
SERVER="$SCRATCH/server"
ENGINE="$REPO/horizon/target/fast/horizon-client"
[ -x "$ENGINE" ] || ENGINE="$ENGINE.exe"
RESOURCES="$REPO/horizon/.local/horizon-client/26.3"

[ -x "$ENGINE" ] || { echo "no engine: build it (docs/horizon/build.md)" >&2; exit 1; }
[ -d "$RESOURCES" ] || { echo "no prepared resources: run horizon-prepare (docs/horizon/run.md)" >&2; exit 1; }

mkdir -p "$SERVER" "$SCRATCH/home/game" "$SCRATCH/home/launcher" "$SCRATCH/home/data" \
         "$SCRATCH/home/config" "$SCRATCH/home/trash" "$SCRATCH/home/clipboard" "$SCRATCH/home/desktop"
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
motd=Rustic test server (Horizon)
EOF
fi
[ -f "$SERVER/eula.txt" ] || echo "eula=true" > "$SERVER/eula.txt"

cd "$SERVER"
exec "$ENGINE" --resources "$RESOURCES" --dedicated --nogui
