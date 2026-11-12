#!/usr/bin/env python3
"""Summarise benchmark runs (docs/PERFORMANCE.md).

  python -I bench/summarize.py run DIR             # DIR/result.json from one bench/run.sh output
  python -I bench/summarize.py table DIR...        # one markdown table, median over repetitions
  python -I bench/summarize.py report DIR...       # the tables of docs/PERFORMANCE.md

Tick percentiles of both servers use Horizon's report definition (horizon-client
game/dedicated/report.rs `pct`): the sorted sample at index floor(n * q), capped at n - 1.
"""
import json
import os
import re
import statistics
import sys
from collections import defaultdict


def pct(sorted_values: list, q: float) -> float:
    if not sorted_values:
        return 0.0
    return sorted_values[min(int(len(sorted_values) * q), len(sorted_values) - 1)]


def load(path: str):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


TICK_AVG = re.compile(r"Average time per tick: ([0-9.]+) ?ms")
TICK_PCT = re.compile(r"P50: ([0-9.]+) ?ms,? P95: ([0-9.]+) ?ms,? P99: ([0-9.]+) ?ms")
DONE = re.compile(r"Done \(([0-9.]+)s\)!")


def run(d: str) -> dict:
    meta = load(os.path.join(d, "meta.json")) or {}
    res = dict(meta)
    res["startup"] = load(os.path.join(d, "startup.json"))
    log = open(os.path.join(d, "server.log"), encoding="utf-8", errors="replace").read()
    m = DONE.search(log)
    if m and res["startup"]:
        res["startup"]["self_reported_done_s"] = float(m.group(1))
    # `tick query`, once every 5 s of the window (vanilla's last-100-ticks statistics).
    avgs = [float(x) for x in TICK_AVG.findall(log)]
    pcts = [tuple(map(float, x)) for x in TICK_PCT.findall(log)]
    if avgs and pcts:
        res["tick_query"] = {
            "samples": len(pcts),
            "average_ms": round(statistics.mean(avgs), 3),
            "p50_ms_median": round(statistics.median(p[0] for p in pcts), 3),
            "p95_ms_mean": round(statistics.mean(p[1] for p in pcts), 3),
            "p99_ms_mean": round(statistics.mean(p[2] for p in pcts), 3),
            "p99_ms_max": round(max(p[2] for p in pcts), 3),
        }
    probe = load(os.path.join(d, "probe.json"))
    report = load(os.path.join(d, "report.json"))
    if probe:
        ticks = sorted(probe["tick_ms"])
        n = max(len(ticks), 1)
        res["ticks"] = {
            "source": "Paper ServerTickEndEvent (bench/bukkit/probe)",
            "count": len(ticks),
            "tps": round(len(ticks) / probe["window_s"], 2),
            "mean_ms": round(sum(ticks) / n, 3),
            "p50_ms": round(pct(ticks, 0.5), 3),
            "p95_ms": round(pct(ticks, 0.95), 3),
            "p99_ms": round(pct(ticks, 0.99), 3),
            "max_ms": round(ticks[-1] if ticks else 0, 3),
            "over_50_ms": sum(1 for t in ticks if t > 50),
            "main_thread_cpu_ms_per_tick": round(probe["main_thread_cpu_ms"] / n, 3),
        }
        res["jvm"] = {k: probe[k] for k in ("gc_count", "gc_ms", "heap_used_mb", "heap_committed_mb", "heap_max_mb")}
    elif report:
        t = report["ticks"]
        res["ticks"] = {
            "source": "Horizon horizon-report (per-tick wall time)",
            "count": t["count"],
            "tps": t["tps"],
            "mean_ms": t["mspt"]["average"],
            "p50_ms": t["mspt"]["p50"],
            "p95_ms": t["mspt"]["p95"],
            "p99_ms": t["mspt"]["p99"],
            "max_ms": t["mspt"]["max"],
            "over_50_ms": t["over_50_ms"],
            "main_thread_cpu_ms_per_tick": t.get("cpu_ms_average"),
            "recording_overhead_ms_per_tick": (report.get("overhead") or {}).get("ms_per_tick"),
        }
    res["process"] = load(os.path.join(d, "proc.json"))
    res["bots_process"] = load(os.path.join(d, "bots-proc.json"))
    gamerule = re.findall(r"Game ?rule [^\r\n]*", log)
    if gamerule:
        res["gamerule_log"] = gamerule
    bots = load(os.path.join(d, "bots.json"))
    if bots:
        res["bots_report"] = {k: v for k, v in bots.items() if not isinstance(v, (list, dict))}
    # Lines worth a look: kicks, errors, the workload counters.
    res["log"] = {
        "kicked": len(re.findall(r"was kicked|kicked for", log)),
        "lost_connection": len(re.findall(r"lost connection", log)),
        "errors": len(re.findall(r"/ERROR\]", log)),
        "workload": (re.findall(r"workload: [^\r\n]*", log) or [None])[-1],
    }
    # `execute if entity` for @e, players, slimes, items (bench/run.sh), in that order.
    tests = re.findall(r"Test (?:passed[.,] [Cc]ount: (\d+)|failed)", log)[-4:]
    if len(tests) == 4:
        res["entities_end"] = dict(zip(("all", "players", "slimes", "items"), (int(t or 0) for t in tests)))
    with open(os.path.join(d, "result.json"), "w", encoding="utf-8") as f:
        json.dump(res, f, indent=1)
    t = res.get("ticks") or {}
    p = res.get("process") or {}
    print(
        f"{res.get('server')}/{res.get('plugin')}/{res.get('bots')} bots: ready {res['startup'] and res['startup']['ready_ms']} ms, "
        f"mspt mean {t.get('mean_ms')} p50 {t.get('p50_ms')} p95 {t.get('p95_ms')} p99 {t.get('p99_ms')} max {t.get('max_ms')}, "
        f"tps {t.get('tps')}, cpu {p.get('cpu_cores')} cores, rss {p.get('rss_mb_mean')} MB, "
        f"tick query {res.get('tick_query')}, log {res['log']}"
    )
    return res


def med(values):
    values = [v for v in values if v is not None]
    return round(statistics.median(values), 2) if values else None


def table(dirs: list) -> None:
    groups = defaultdict(list)
    for d in dirs:
        r = load(os.path.join(d, "result.json"))
        if r:
            groups[(r["server"], r["plugin"], r["bots"])].append(r)
    print("| server | plugin | bots | runs | ready ms | MSPT mean | p50 | p95 | p99 | max | TPS | tick query avg / p95 | main-thread CPU ms/tick | CPU cores | RSS MB |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for (server, plugin, bots), rs in sorted(groups.items(), key=lambda kv: (kv[0][2], kv[0][1], kv[0][0])):
        g = lambda f: med([f(r) for r in rs])  # noqa: E731
        t = lambda k: g(lambda r: (r.get("ticks") or {}).get(k))  # noqa: E731
        print(
            f"| {server} | {plugin} | {bots} | {len(rs)} | {g(lambda r: (r.get('startup') or {}).get('ready_ms'))} "
            f"| {t('mean_ms')} | {t('p50_ms')} | {t('p95_ms')} | {t('p99_ms')} | {t('max_ms')} | {t('tps')} "
            f"| {g(lambda r: (r.get('tick_query') or {}).get('average_ms'))} / {g(lambda r: (r.get('tick_query') or {}).get('p95_ms_mean'))} "
            f"| {t('main_thread_cpu_ms_per_tick')} | {g(lambda r: (r.get('process') or {}).get('cpu_cores'))} "
            f"| {g(lambda r: (r.get('process') or {}).get('rss_mb_mean'))} |"
        )


def spread(values, digits=1):
    """median (min-max) of the runs."""
    values = [v for v in values if v is not None]
    if not values:
        return "n/a"
    m = statistics.median(values)
    if len(values) == 1:
        return f"{m:.{digits}f}"
    return f"{m:.{digits}f} ({min(values):.{digits}f}-{max(values):.{digits}f})"


def report(dirs: list) -> None:
    """The three tables of docs/PERFORMANCE.md, median (min-max) over repetitions."""
    groups = defaultdict(list)
    for d in dirs:
        r = load(os.path.join(d, "result.json"))
        if r:
            groups[(r["server"], r["plugin"], r["bots"])].append(r)
    get = lambda rs, *path: [_dig(r, path) for r in rs]  # noqa: E731
    name = {"rustic": "Rustic", "paper": "Paper"}

    print("Startup and idle (0 bots)\n")
    print('| server | runs | launch to "Done", ms | self-reported "Done", s | RSS idle, MB | CPU idle, cores | MSPT mean | p50 | p95 | p99 | max |')
    print("|---|---|---|---|---|---|---|---|---|---|---|")
    for s in ("rustic", "paper"):
        rs = groups.get((s, "none", 0), [])
        print(f"| {name[s]} | {len(rs)} | {spread(get(rs, 'startup', 'done_ms'), 0)} | {spread(get(rs, 'startup', 'self_reported_done_s'), 2)} "
              f"| {spread(get(rs, 'process', 'rss_mb_mean'), 0)} | {spread(get(rs, 'process', 'cpu_cores'), 3)} "
              f"| {spread(get(rs, 'ticks', 'mean_ms'), 2)} | {spread(get(rs, 'ticks', 'p50_ms'), 2)} | {spread(get(rs, 'ticks', 'p95_ms'), 2)} "
              f"| {spread(get(rs, 'ticks', 'p99_ms'), 2)} | {spread(get(rs, 'ticks', 'max_ms'), 1)} |")

    print("\nLoad (no plugin)\n")
    print("| bots | server | runs | MSPT mean | p50 | p95 | p99 | max | TPS | `tick query` avg | CPU, cores | RSS, MB | entities at end (slimes), median | bots' CPU, cores |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for n in sorted({k[2] for k in groups}):
        for s in ("rustic", "paper"):
            rs = groups.get((s, "none", n))
            if not rs:
                continue
            ents = f"{med(get(rs, 'entities_end', 'all'))} ({med(get(rs, 'entities_end', 'slimes'))})"
            print(f"| {n} | {name[s]} | {len(rs)} | {spread(get(rs, 'ticks', 'mean_ms'))} | {spread(get(rs, 'ticks', 'p50_ms'))} "
                  f"| {spread(get(rs, 'ticks', 'p95_ms'))} | {spread(get(rs, 'ticks', 'p99_ms'))} | {spread(get(rs, 'ticks', 'max_ms'), 0)} "
                  f"| {spread(get(rs, 'ticks', 'tps'), 2)} | {spread(get(rs, 'tick_query', 'average_ms'))} | {spread(get(rs, 'process', 'cpu_cores'), 2)} "
                  f"| {spread(get(rs, 'process', 'rss_mb_mean'), 0)} | {ents} | {spread(get(rs, 'bots_process', 'cpu_cores'), 2)} |")

    print("\nPlugin overhead\n")
    print("| server | plugin | bots | runs | MSPT mean | delta vs no plugin (medians) | p95 | p99 | max | CPU, cores | RSS, MB | workload counters (last run) |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|")
    shown = set()
    for (s, p, n), rs in sorted(groups.items()):
        if p == "none" or not rs:
            continue
        base = groups.get((s, "none", n), [])
        delta = None
        if base:
            delta = med(get(rs, "ticks", "mean_ms")) - med(get(base, "ticks", "mean_ms"))
        for label, group in (("none", base), (p, rs)):
            if not group or (s, label, n) in shown:
                continue
            shown.add((s, label, n))
            print(f"| {name[s]} | {label} | {n} | {len(group)} | {spread(get(group, 'ticks', 'mean_ms'))} "
                  f"| {'' if label == 'none' else (f'{delta:+.1f} ms' if delta is not None else 'n/a')} "
                  f"| {spread(get(group, 'ticks', 'p95_ms'))} | {spread(get(group, 'ticks', 'p99_ms'))} | {spread(get(group, 'ticks', 'max_ms'), 0)} "
                  f"| {spread(get(group, 'process', 'cpu_cores'), 2)} | {spread(get(group, 'process', 'rss_mb_mean'), 0)} "
                  f"| {(group[-1].get('log') or {}).get('workload') or ''} |")


def _dig(r, path):
    for k in path:
        if not isinstance(r, dict):
            return None
        r = r.get(k)
    return r


if __name__ == "__main__":
    if len(sys.argv) >= 3 and sys.argv[1] == "run":
        run(sys.argv[2])
    elif len(sys.argv) >= 3 and sys.argv[1] == "table":
        table(sys.argv[2:])
    elif len(sys.argv) >= 3 and sys.argv[1] == "report":
        report(sys.argv[2:])
    else:
        sys.exit(__doc__)
