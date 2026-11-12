#!/usr/bin/env python3
"""Summarise a horizon-bots `--capture` file: the play packets a bot got that show what plugins
put on its screen (menus, boss bars, scoreboard objectives, system chat lines), with their time.

  python -I scripts/bot-capture-summary.py .local/ui1-capture.txt

Record layout (horizon/crates/horizon-bots/src/main.rs `Capture`): little-endian u32 ms since the
bot joined, i32 packet id, u32 body length, then the body. Ids are 26.3's clientbound play ids
(horizon/crates/horizon-protocol/src/ids.rs).
"""
import struct
import sys

NAMES = {
    9: "BossEvent",
    17: "ContainerClose",
    18: "ContainerSetContent",
    60: "OpenScreen",
    100: "SetDisplayObjective",
    108: "SetObjective",
    112: "SetScore",
}


def strings(body: bytes) -> list[str]:
    """Printable runs of the body (titles and names are plain in their component NBT)."""
    out, run = [], bytearray()
    for b in body:
        if 32 <= b < 127:
            run.append(b)
        else:
            if len(run) >= 4:
                out.append(run.decode())
            run = bytearray()
    if len(run) >= 4:
        out.append(run.decode())
    return out


def main(path: str) -> None:
    data = open(path, "rb").read()
    i = 0
    while i + 12 <= len(data):
        ms, pid, n = struct.unpack_from("<IiI", data, i)
        body = data[i + 12:i + 12 + n]
        i += 12 + n
        if pid in NAMES:
            print(f"{ms / 1000:7.2f}s {NAMES[pid]:<20} {' | '.join(strings(body))[:150]}")


if __name__ == "__main__":
    main(sys.argv[1])
