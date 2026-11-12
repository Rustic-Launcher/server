#!/usr/bin/env python3
"""Fetch the pinned Java Edition client jar and its libraries for horizon-prepare.

Horizon's preparer (horizon/crates/horizon-prepare) never downloads the game: it reads a
launcher folder. When the official launcher has only the version JSON (the version was never
played), this script downloads the files that JSON lists - the client jar and the client
libraries, each checked against the SHA-1 in the JSON - into a private launcher-shaped folder
(default `.local/minecraft-home`, gitignored), WITHOUT touching the launcher's own folder.
Pass that folder to horizon-prepare with `--minecraft-dir`.

These are the user's own licensed game files (downloaded from Mojang's URLs that the user's
launcher wrote into the version JSON). Never commit them: they live only under `.local/`.

Usage:
    python scripts/horizon-fetch-game-files.py [--version 26.3] [--launcher-dir DIR] [--out DIR]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import sys
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


def default_launcher_dir() -> Path:
    if sys.platform == "win32":
        return Path(os.environ["APPDATA"]) / ".minecraft"
    if sys.platform == "darwin":
        return Path.home() / "Library/Application Support/minecraft"
    return Path.home() / ".minecraft"


def os_name() -> str:
    return {"win32": "windows", "darwin": "osx"}.get(sys.platform, "linux")


def allowed(library: dict) -> bool:
    rules = library.get("rules")
    if not rules:
        return True
    result = False
    for rule in rules:
        os_rule = rule.get("os")
        if os_rule and os_rule.get("name") not in (None, os_name()):
            continue
        result = rule.get("action") == "allow"
    return result


def sha1_of(path: Path) -> str:
    digest = hashlib.sha1()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def fetch(url: str, destination: Path, sha1: str) -> str:
    if destination.is_file() and sha1_of(destination) == sha1:
        return "kept"
    destination.parent.mkdir(parents=True, exist_ok=True)
    partial = destination.with_suffix(destination.suffix + ".part")
    with urllib.request.urlopen(url, timeout=120) as response, partial.open("wb") as out:
        shutil.copyfileobj(response, out)
    got = sha1_of(partial)
    if got != sha1:
        partial.unlink()
        raise SystemExit(f"checksum mismatch for {url}: {got} != {sha1}")
    partial.replace(destination)
    return "fetched"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--version", default="26.3")
    parser.add_argument("--launcher-dir", type=Path, default=default_launcher_dir())
    parser.add_argument("--out", type=Path, default=REPO / ".local" / "minecraft-home")
    args = parser.parse_args()

    v = args.version
    source_json = args.launcher_dir / "versions" / v / f"{v}.json"
    if not source_json.is_file():
        raise SystemExit(f"no {source_json}: open the official launcher and select {v} once")
    manifest = json.loads(source_json.read_text(encoding="utf-8"))

    out = args.out
    version_dir = out / "versions" / v
    version_dir.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source_json, version_dir / f"{v}.json")

    client = manifest["downloads"]["client"]
    print("client jar:", fetch(client["url"], version_dir / f"{v}.jar", client["sha1"]))

    counts = {"kept": 0, "fetched": 0, "skipped": 0}
    for library in manifest.get("libraries", []):
        artifact = library.get("downloads", {}).get("artifact")
        if not artifact or not allowed(library):
            counts["skipped"] += 1
            continue
        # Reuse the launcher's own copy when it has one.
        local = args.launcher_dir / "libraries" / artifact["path"]
        destination = out / "libraries" / artifact["path"]
        if local.is_file() and sha1_of(local) == artifact["sha1"] and not destination.exists():
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(local, destination)
            counts["kept"] += 1
            continue
        counts[fetch(artifact["url"], destination, artifact["sha1"])] += 1
    print(f"libraries: {counts} (os={os_name()}, arch={platform.machine()})")
    print(f"pass --minecraft-dir {out} to horizon-prepare")
    return 0


if __name__ == "__main__":
    sys.exit(main())
