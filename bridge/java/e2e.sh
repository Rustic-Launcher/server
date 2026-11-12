#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
#
# End-to-end test of Rustic Bridge on the real server: rustic-server (built with
# the default feature `bridge`) loads the test plugins of build/plugins/ from plugins/bukkit/, a protocol
# bot joins, chats and runs their commands, then the console checks crash recovery.
#
#   bridge/java/build.sh && bridge/java/e2e.sh [PORT]
#
# Needs: horizon/target/fast/rustic-server(.exe) built with the `bridge` feature
# (crates/rusticmc-server/README.md), horizon-bots, and the prepared game resources
# (docs/horizon/run.md). Scratch folder: .local/rustic-bridge-e2e (gitignored); log in
# .local/rustic-bridge-e2e/server.log. Writing eula.txt accepts the Minecraft EULA for this local
# test of the user's own copy, as scripts/rustic-server-test.sh does.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
port="${1:-25611}"
scratch="$repo/.local/rustic-bridge-e2e"
server="$scratch/server"
engine="$repo/horizon/target/fast/rustic-server"; [ -x "$engine" ] || engine="$engine.exe"
bots="$repo/horizon/target/fast/horizon-bots"; [ -x "$bots" ] || bots="$bots.exe"
resources="$repo/horizon/.local/horizon-client/26.3"
java="$repo/tools/jdk25/bin/java"; [ -x "$java" ] || java="$java.exe"
case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) wp() { cygpath -w "$1"; } ;; *) wp() { echo "$1"; } ;; esac

rm -rf "$scratch" && mkdir -p "$server/plugins/bukkit"
for d in game launcher data config trash clipboard desktop; do mkdir -p "$scratch/home/$d"; done
export HORIZON_HOME="$scratch/home/game" HORIZON_LAUNCHER_HOME="$scratch/home/launcher"
export XDG_DATA_HOME="$scratch/home/data" XDG_CONFIG_HOME="$scratch/home/config"
export HORIZON_NO_AUDIO=1 HORIZON_LAUNCHER_NO_SELF_UPDATE=1
export HORIZON_LAUNCHER_TRASH="$scratch/home/trash" HORIZON_LAUNCHER_CLIPBOARD="$scratch/home/clipboard"
export HORIZON_LAUNCHER_DESKTOP="$scratch/home/desktop"
cp "$here"/build/plugins/*.jar "$server/plugins/bukkit/"
printf 'online-mode=false\nserver-port=%s\nserver-ip=127.0.0.1\nlevel-name=world\nlevel-type=minecraft\\:flat\nwhite-list=false\nenforce-whitelist=false\nallow-flight=true\nview-distance=4\nsimulation-distance=4\nspawn-protection=0\nmotd=Rustic Bridge e2e\n' "$port" >"$server/server.properties"
echo "eula=true" >"$server/eula.txt"
# Forward slashes: TOML basic strings treat backslashes as escapes.
cat >"$server/rustic-bridge.toml" <<TOML
java = "$(wp "$java" | tr '\\' '/')"
home = "$(wp "$here/build" | tr '\\' '/')"
call_timeout_ms = 5000
jvm_args = ["-Xmx512m"]
TOML

: >"$scratch/console.in"
(cd "$server" && tail -f "$scratch/console.in" | "$engine" --resources "$resources" --dedicated --nogui \
  >"$scratch/server.log" 2>&1) &
console() { echo "$1" >>"$scratch/console.in"; }
wait_for() { # pattern seconds
  for _ in $(seq 1 "$2"); do grep -q "$1" "$scratch/server.log" 2>/dev/null && return 0; sleep 1; done
  echo "timed out waiting for: $1" >&2; return 1
}
wait_for "Rustic ready" 120
console "rustic plugins"
sleep 1

# Raw serverbound packets: ChatCommand (id 7) = String; Chat (id 9) = String, timestamp, salt,
# no signature, last-seen update (offset 0, 20-bit set, checksum 0).
raw="$(python -I - <<'PY'
import time
def s(t):
    b = t.encode(); n = len(b); out = b""
    while True:
        x = n & 0x7f; n >>= 7
        out += bytes([x | (0x80 if n else 0)])
        if not n: break
    return out + b
def chat(t): return "09:" + (s(t) + int(time.time()*1000).to_bytes(8, "big") + (1).to_bytes(8, "big") + b"\0\0\0\0\0\0").hex()
def cmd(t): return "07:" + s(t).hex()
steps = [(60, chat("hello from a bot")), (90, chat("this is a badword")), (120, cmd("welcome")),
         (140, cmd("heal")), (190, cmd("heal")), (200, cmd("fly")), (210, cmd("welcome reload")),
         (230, cmd("bc hello everyone")), (280, cmd("probe sqlite")), (300, cmd("probe async")),
         (320, cmd("probe exit")), (340, cmd("probe throw")), (360, chat("still alive")),
         (380, cmd("probe loop")), (560, chat("the server survived a stuck plugin"))]
print(",".join(f"{t}:{p}" for t, p in steps))
PY
)"
"$bots" --server "127.0.0.1:$port" --bots 1 --behaviour idle --seconds 32 --name BridgeBot \
  --raw "$raw" --capture "$(wp "$scratch/capture.txt")" >"$scratch/bots.log" 2>&1 &
botpid=$!
# The first /heal (tick 140, 7 s after joining) runs before the bot is op: hidden by its node.
wait_for "BridgeBot0.*logged in" 30 || true
sleep 7
console "op BridgeBot0"
wait "$botpid" || true

console "rustic plugins"
console "rustic reload bukkit-bridge"
sleep 3
console "rustic enable bukkit-welcome"
sleep 2
console "rustic plugins"
console "stop"
wait_for "JVM stopped\|Stopping the server\|Saving worlds" 60 || true
sleep 3
pkill -f "tail -f $scratch/console.in" 2>/dev/null || true
echo "--- server log: $server/logs/latest.log"
grep -E "bukkit|Bridge|rustic\]|System chat|BridgeBot0" "$server/logs/latest.log" | grep -vE "^\s+at |\] \s+at |\.\.\. [0-9]+ more" || true
echo "--- what the bot was told"
# Capture records: u32 ms, i32 id, u32 length, body (little-endian); 124 = SystemChat.
python -I - "$scratch/capture.txt" <<'PY'
import struct, sys
data = open(sys.argv[1], "rb").read()
i = 0
while i + 12 <= len(data):
    ms, pid, n = struct.unpack_from("<IiI", data, i)
    body = data[i + 12:i + 12 + n]
    i += 12 + n
    if pid != 124:
        continue
    runs, run = [], bytearray()
    for b in body + b"\0":
        if 32 <= b < 127 or b >= 0xa0:
            run.append(b)
        else:
            if len(run) >= 3:
                runs.append(run.decode("utf-8", "replace"))
            run = bytearray()
    text = " | ".join(r for r in runs if r not in ("text", "color", "extra", "bold", "italic", "translate", "with"))
    print(f"{ms / 1000:7.2f}s SystemChat  {text}")
PY
