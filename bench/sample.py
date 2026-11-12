#!/usr/bin/env python3
"""CPU and memory of a server process tree over a window (docs/PERFORMANCE.md).

  python -I bench/sample.py --match paper.jar --seconds 60 --out cpu.json
  python -I bench/sample.py --match rustic-server --seconds 0      # one RSS snapshot
  python -I bench/sample.py --match paper.jar --kill                # kill that tree (cleanup)

The root is the newest process whose command line contains --match (case-insensitive); its
children (Rustic Bridge's JVM sidecar) are counted with it. CPU is user + system time of the tree
divided by wall time, in cores (1.0 = one core busy). RSS is the resident set (the working set on
Windows), summed over the tree. Needs psutil (BSD-3-Clause).
"""
import argparse
import json
import sys
import time

import psutil


def find(match: str) -> psutil.Process:
    me = psutil.Process().pid
    best = None
    for p in psutil.process_iter(["pid", "cmdline", "create_time", "name"]):
        try:
            cmd = " ".join(p.info["cmdline"] or [])
        except (psutil.Error, TypeError):
            continue
        if p.pid == me or match.lower() not in cmd.lower() or "sample.py" in cmd:
            continue
        if best is None or p.info["create_time"] > best.info["create_time"]:
            best = p
    if best is None:
        sys.exit(f"no process matches {match!r}")
    return best


def tree(root: psutil.Process) -> list:
    try:
        return [root] + root.children(recursive=True)
    except psutil.Error:
        return [root]


def measure(procs: list) -> tuple:
    cpu = rss = 0.0
    for p in procs:
        try:
            t = p.cpu_times()
            cpu += t.user + t.system
            rss += p.memory_info().rss
        except psutil.Error:
            pass
    return cpu, rss


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--match", required=True)
    ap.add_argument("--seconds", type=float, default=60)
    ap.add_argument("--interval", type=float, default=1.0)
    ap.add_argument("--out")
    ap.add_argument("--kill", action="store_true")
    a = ap.parse_args()
    root = find(a.match)
    if a.kill:
        for p in reversed(tree(root)):
            try:
                p.kill()
            except psutil.Error:
                pass
        print(f"killed {root.pid}")
        return
    procs = tree(root)
    t0 = time.perf_counter()
    cpu0, rss0 = measure(procs)
    rss = [rss0]
    while time.perf_counter() - t0 < a.seconds:
        time.sleep(a.interval)
        procs = tree(root)
        _, r = measure(procs)
        rss.append(r)
    cpu1, rss1 = measure(tree(root))
    wall = time.perf_counter() - t0
    mb = 1024 * 1024
    out = {
        "pid": root.pid,
        "processes": len(procs),
        "window_s": round(wall, 3),
        "cpu_cores": round((cpu1 - cpu0) / wall, 4) if wall > 0.5 else None,
        "cpu_s_total": round(cpu1, 2),
        "rss_mb_end": round(rss1 / mb, 1),
        "rss_mb_mean": round(sum(rss) / len(rss) / mb, 1),
        "rss_mb_max": round(max(rss + [rss1]) / mb, 1),
        "logical_cpus": psutil.cpu_count(),
    }
    text = json.dumps(out)
    if a.out:
        with open(a.out, "w", encoding="utf-8") as f:
            f.write(text + "\n")
    print(text)


if __name__ == "__main__":
    main()
