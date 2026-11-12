#!/usr/bin/env python3
"""The bots' fixed chat and command script, as horizon-bots `--raw TICK:ID:HEX,...`.

  python -I bench/botscript.py --from-tick 200 --to-tick 2400 --chat-every 40 --command-every 100

Every bot sends the same packets at the same ticks after it joined (bots join a fixed interval
apart, so their messages are staggered). Serverbound play ids of 26.3 (as bridge/java/e2e.sh):
ChatCommand (7) = String; Chat (9) = String, timestamp (i64 ms), salt (i64), no signature,
last-seen update (offset 0, empty 20-bit set, checksum 0 = not checked). Timestamps grow with the
tick so a server checking chat order accepts them. Messages contain no banned word of the
workload plugins (they are counted and scanned, never cancelled).
"""
import argparse
import time


def string(text: str) -> bytes:
    b = text.encode()
    n, out = len(b), b""
    while True:
        x = n & 0x7F
        n >>= 7
        out += bytes([x | (0x80 if n else 0)])
        if not n:
            break
    return out + b


def chat(text: str, ms: int) -> str:
    body = string(text) + ms.to_bytes(8, "big") + (1).to_bytes(8, "big") + b"\0" * 6
    return "9:" + body.hex()


def command(text: str) -> str:
    return "7:" + string(text).hex()


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--from-tick", type=int, default=200)
    ap.add_argument("--to-tick", type=int, default=2400)
    ap.add_argument("--chat-every", type=int, default=40)
    ap.add_argument("--command-every", type=int, default=100)
    ap.add_argument("--command", default="workload")
    a = ap.parse_args()
    now = int(time.time() * 1000)
    steps = []
    for n, tick in enumerate(range(a.from_tick, a.to_tick, a.chat_every)):
        steps.append(f"{tick}:{chat(f'bench message {n}: walking around the spawn', now + tick * 50)}")
    for tick in range(a.from_tick + a.chat_every // 2, a.to_tick, a.command_every):
        steps.append(f"{tick}:{command(a.command)}")
    print(",".join(steps))


if __name__ == "__main__":
    main()
