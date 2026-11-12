#!/usr/bin/env python3
"""Download a panel of popular Bukkit/Spigot/Paper plugin jars for LOCAL static analysis.

Sources: the public Modrinth API (https://api.modrinth.com/v2) and the public Hangar API
(https://hangar.papermc.io/api/v1). Nothing requires a login; SpigotMC is never scraped.

The jars land in bridge/panel/jars/ (gitignored). They are third-party works under their own
licences: they are read locally by `rustic-bridge-analyze` and must never be committed or
redistributed. Only metadata (name, version, source URL, licence, sha256) is written to
bridge/panel/panel.json, which is committed.

Usage (stdlib only, Python 3.9+):

    python -I bridge/panel/fetch.py [--count 50] [--max-mb 60] [--dry-run]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
JARS = HERE / "jars"
PANEL = HERE / "panel.json"

USER_AGENT = "rustic-mc/bridge-panel 0.1 (static API-usage analysis; github.com/Rustic-Launcher/server)"
MODRINTH = "https://api.modrinth.com/v2"
HANGAR = "https://hangar.papermc.io/api/v1"
LOADERS = ["paper", "spigot", "bukkit"]
TOTAL_BUDGET_MB = 900  # stay under the 1 GB download budget

# Well-known server plugins we want in the panel when the public APIs carry them.
# (source, slug) pairs; tried first, then the panel is filled from the most-downloaded lists.
WELL_KNOWN = [
    ("modrinth", "essentialsx"),
    ("modrinth", "luckperms"),
    ("modrinth", "vaultunlocked"),
    ("modrinth", "worldedit"),
    ("modrinth", "worldguard"),
    ("hangar", "ViaVersion"),
    ("hangar", "ViaBackwards"),
    ("modrinth", "chunky"),
    ("modrinth", "discordsrv"),
    ("hangar", "PlaceholderAPI"),
    ("modrinth", "coreprotect"),
    ("modrinth", "multiverse-core"),
    ("modrinth", "multiverse-portals"),
    ("modrinth", "griefprevention"),
    ("modrinth", "geyser"),
    ("modrinth", "tab-was-taken"),
    ("modrinth", "skinsrestorer"),
    ("modrinth", "bluemap"),
    ("modrinth", "squaremap"),
    ("modrinth", "dynmap"),
    ("modrinth", "decentholograms"),
    ("modrinth", "fancyholograms"),
    ("modrinth", "fancynpcs"),
    ("modrinth", "plan"),
    ("modrinth", "grimac"),
    ("modrinth", "simple-voice-chat"),
    ("hangar", "CommandAPI"),
    ("hangar", "ProtocolLib"),
    ("modrinth", "packetevents"),
    ("modrinth", "fastasyncworldedit"),
    ("modrinth", "chestsort"),
    ("modrinth", "maintenance"),
    ("hangar", "QuickShop-Hikari"),
    ("modrinth", "nuvotifier"),
]

# Client-side mod loaders: projects that also ship these are mostly client mods with a small
# server companion, which would skew a "popular server plugins" panel. They are only taken
# from WELL_KNOWN, never from the most-downloaded fill.
MOD_LOADERS = {"fabric", "forge", "neoforge", "quilt"}


def http_json(url: str):
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT, "Accept": "application/json"})
    for attempt in range(4):
        try:
            with urllib.request.urlopen(req, timeout=60) as resp:
                return json.load(resp)
        except urllib.error.HTTPError as err:
            if err.code == 404:
                return None
            if err.code in (401, 403):
                return None  # needs a login or is forbidden: skip
            if err.code == 429 or err.code >= 500:
                time.sleep(2 + attempt * 3)
                continue
            raise
        except (urllib.error.URLError, TimeoutError):
            time.sleep(2 + attempt * 3)
    return None


def game_version_key(v: str):
    """Sortable key for release game versions ("1.21.11" < "26.3"); None for snapshots."""
    if not re.fullmatch(r"\d+(\.\d+)*", v):
        return None
    return tuple(int(p) for p in v.split("."))


def newest_game_version(versions) -> tuple:
    keys = [k for k in (game_version_key(v) for v in versions) if k is not None]
    return max(keys) if keys else ()


def norm(name: str) -> str:
    return re.sub(r"[^a-z0-9]", "", name.lower())


# ---------------------------------------------------------------- Modrinth


def modrinth_candidate(slug: str, project: dict | None = None):
    project = project or http_json(f"{MODRINTH}/project/{urllib.parse.quote(slug)}")
    if not project:
        return None
    loaders = urllib.parse.quote(json.dumps(LOADERS))
    versions = http_json(f"{MODRINTH}/project/{project['id']}/version?loaders={loaders}")
    if not versions:
        return None
    # Prefer releases; among them the one supporting the newest game version, then the newest.
    def score(v):
        return (
            v.get("version_type") == "release",
            newest_game_version(v.get("game_versions", [])),
            v.get("date_published", ""),
        )

    versions.sort(key=score, reverse=True)
    for v in versions:
        if not set(v.get("loaders", [])) & set(LOADERS):
            continue
        files = [f for f in v.get("files", []) if f["filename"].endswith(".jar")]
        if not files:
            continue
        primary = next((f for f in files if f.get("primary")), files[0])
        lic = project.get("license") or {}
        return {
            "name": project.get("title", slug),
            "slug": project.get("slug", slug),
            "source": "modrinth",
            "project_url": f"https://modrinth.com/plugin/{project.get('slug', slug)}",
            "version": v.get("version_number", "?"),
            "game_versions_newest": ".".join(map(str, newest_game_version(v.get("game_versions", [])))),
            "loaders": v.get("loaders", []),
            "license": lic.get("id") if isinstance(lic, dict) else str(lic),
            "download_url": primary["url"],
            "filename": primary["filename"],
            "size": primary.get("size", 0),
            "expected_sha512": primary.get("hashes", {}).get("sha512"),
            "downloads": project.get("downloads", 0),
        }
    return None


def modrinth_top(limit: int):
    facets = json.dumps([[f"categories:{l}" for l in LOADERS], ["project_type:plugin"]])
    out, offset = [], 0
    while len(out) < limit and offset < 1000:
        q = urllib.parse.urlencode({"facets": facets, "index": "downloads", "limit": 100, "offset": offset})
        page = http_json(f"{MODRINTH}/search?{q}")
        if not page or not page.get("hits"):
            break
        out.extend(h["slug"] for h in page["hits"] if not set(h.get("categories", [])) & MOD_LOADERS)
        offset += 100
    return out[:limit]


# ---------------------------------------------------------------- Hangar


def hangar_candidate(slug: str, project: dict | None = None):
    project = project or http_json(f"{HANGAR}/projects/{urllib.parse.quote(slug)}")
    if not project:
        return None
    page = http_json(f"{HANGAR}/projects/{urllib.parse.quote(slug)}/versions?limit=25&platform=PAPER")
    if not page or not page.get("result"):
        return None
    versions = page["result"]

    def score(v):
        release = (v.get("channel") or {}).get("name", "").lower() == "release"
        gv = newest_game_version((v.get("platformDependencies") or {}).get("PAPER", []))
        return (release, gv, v.get("createdAt", ""))

    versions.sort(key=score, reverse=True)
    for v in versions:
        dl = (v.get("downloads") or {}).get("PAPER")
        if not dl or not dl.get("downloadUrl"):
            continue  # external links (other sites) are skipped
        info = dl.get("fileInfo") or {}
        lic = ((project.get("settings") or {}).get("license") or {})
        name = project.get("name", slug)
        return {
            "name": name,
            "slug": slug,
            "source": "hangar",
            "project_url": f"https://hangar.papermc.io/{project['namespace']['owner']}/{project['namespace']['slug']}",
            "version": v.get("name", "?"),
            "game_versions_newest": ".".join(
                map(str, newest_game_version((v.get("platformDependencies") or {}).get("PAPER", [])))
            ),
            "loaders": ["paper"],
            "license": lic.get("name") or lic.get("type"),
            "download_url": dl["downloadUrl"],
            "filename": info.get("name") or f"{norm(name)}.jar",
            "size": info.get("sizeBytes", 0),
            "expected_sha256": info.get("sha256Hash"),
            "downloads": (project.get("stats") or {}).get("downloads", 0),
        }
    return None


def hangar_top(limit: int):
    out, offset = [], 0
    while len(out) < limit and offset < 1000:
        page = http_json(f"{HANGAR}/projects?sort=-downloads&limit=25&offset={offset}")
        if not page or not page.get("result"):
            break
        for p in page["result"]:
            if "PAPER" in (p.get("supportedPlatforms") or {}):
                out.append(p["namespace"]["slug"])
        offset += 25
    return out[:limit]


# ---------------------------------------------------------------- download


def download(entry: dict, max_bytes: int) -> bool:
    JARS.mkdir(parents=True, exist_ok=True)
    safe = re.sub(r"[^A-Za-z0-9._+-]", "_", f"{norm(entry['name'])}--{entry['filename']}")
    if not safe.endswith(".jar"):
        safe += ".jar"
    dest = JARS / safe
    if not dest.exists():
        req = urllib.request.Request(entry["download_url"], headers={"User-Agent": USER_AGENT})
        try:
            with urllib.request.urlopen(req, timeout=120) as resp:
                data = resp.read(max_bytes + 1)
        except (urllib.error.URLError, TimeoutError) as err:
            print(f"  ! download failed: {err}", file=sys.stderr)
            return False
        if len(data) > max_bytes:
            print("  ! too large, skipped", file=sys.stderr)
            return False
        tmp = dest.with_suffix(".part")
        tmp.write_bytes(data)
        os.replace(tmp, dest)
    data = dest.read_bytes()
    sha256 = hashlib.sha256(data).hexdigest()
    if entry.get("expected_sha256") and entry["expected_sha256"].lower() != sha256:
        print("  ! sha256 mismatch, discarded", file=sys.stderr)
        dest.unlink()
        return False
    if entry.get("expected_sha512") and entry["expected_sha512"].lower() != hashlib.sha512(data).hexdigest():
        print("  ! sha512 mismatch, discarded", file=sys.stderr)
        dest.unlink()
        return False
    entry["sha256"] = sha256
    entry["size"] = len(data)
    entry["jar"] = safe
    return True


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--count", type=int, default=50, help="panel size (default 50)")
    ap.add_argument("--max-mb", type=int, default=60, help="skip jars larger than this")
    ap.add_argument("--dry-run", action="store_true", help="resolve versions, download nothing")
    args = ap.parse_args()

    seen: set[str] = set()
    panel: list[dict] = []
    total = 0
    budget = TOTAL_BUDGET_MB * 1024 * 1024

    def consider(source: str, slug: str) -> None:
        nonlocal total
        if len(panel) >= args.count or norm(slug) in seen:
            return
        cand = modrinth_candidate(slug) if source == "modrinth" else hangar_candidate(slug)
        if not cand:
            print(f"- {source}:{slug}: no Bukkit/Paper jar available, skipped")
            seen.add(norm(slug))
            return
        key = norm(cand["name"])
        if key in seen:
            return
        seen.update({key, norm(slug), norm(cand["slug"])})
        if cand["size"] and cand["size"] > args.max_mb * 1024 * 1024:
            print(f"- {cand['name']}: {cand['size'] // 1048576} MB > --max-mb, skipped")
            return
        if total + (cand["size"] or 0) > budget:
            print(f"- {cand['name']}: download budget reached, skipped")
            return
        print(f"+ {cand['name']} {cand['version']} ({source}, MC {cand['game_versions_newest']})")
        if not args.dry_run and not download(cand, args.max_mb * 1024 * 1024):
            return
        total += cand.get("size") or 0
        panel.append(cand)

    for source, slug in WELL_KNOWN:
        consider(source, slug)
    top_m = modrinth_top(120)
    top_h = hangar_top(100)
    # Interleave both sources so the panel reflects both ecosystems.
    for i in range(max(len(top_m), len(top_h))):
        if len(panel) >= args.count:
            break
        if i < len(top_h):
            consider("hangar", top_h[i])
        if i < len(top_m):
            consider("modrinth", top_m[i])

    for e in panel:
        e.pop("expected_sha256", None)
        e.pop("expected_sha512", None)
    panel.sort(key=lambda e: e["name"].lower())
    meta = {
        "generated": time.strftime("%Y-%m-%d", time.gmtime()),
        "note": "Metadata only. Jars are fetched locally into bridge/panel/jars (gitignored) "
        "for static analysis and are never committed or redistributed.",
        "sources": [MODRINTH, HANGAR],
        "plugins": panel,
    }
    if not args.dry_run:
        keep = {e["jar"] for e in panel}
        for stale in JARS.glob("*.jar"):
            if stale.name not in keep:
                stale.unlink()  # from an earlier run, no longer in the panel
        PANEL.write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8")
    print(f"{len(panel)} plugins, {total / 1048576:.1f} MB")
    return 0


if __name__ == "__main__":
    sys.exit(main())
