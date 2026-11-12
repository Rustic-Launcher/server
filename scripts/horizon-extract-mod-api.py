#!/usr/bin/env python3
"""Extract Horizon's generated trait-mod API into Rustic's reference docs.

Reads the files horizon-client's build script generates (OUT_DIR/mod_api_generated.rs and
OUT_DIR/mod_api_generated_host.rs, from `horizon/native-hooks.json` and the game's source
signatures) plus the catalog itself, and writes:

  docs/horizon/hooks.md    every server-side ("server" or "both") `Hooks` method, its exact
                           signature, its event struct's fields, and its condensed doc
  docs/horizon/hooks.json  the same, machine-readable: {name, side, cancellable, contract,
                           version, signature, has_host, event, event_fields, target, neoforge}
  docs/horizon/host.md     every `Host` method, exact signature, condensed doc

Usage (after `cargo check -p horizon-client --no-default-features` in horizon/ with
CARGO_TARGET_DIR=<repo>/horizon/target):

    python -I scripts/horizon-extract-mod-api.py [--out-dir DIR]

`--out-dir` is the build script's OUT_DIR; by default the newest
horizon/target/*/build/horizon-client-*/out holding mod_api_generated.rs.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
HORIZON = REPO / "horizon"
DOCS = REPO / "docs" / "horizon"


def find_out_dir() -> Path:
    candidates = sorted(
        (p.parent for p in (HORIZON / "target").glob("*/build/horizon-client-*/out/mod_api_generated.rs")),
        key=lambda p: (p / "mod_api_generated.rs").stat().st_mtime,
        reverse=True,
    )
    if not candidates:
        raise SystemExit("no generated mod API: run cargo check -p horizon-client first")
    return candidates[0]


def squash(sig: str) -> str:
    """Normalise the generator's token spacing (`& 'a str`, `Vec < Text >`) to rustfmt-like text."""
    s = re.sub(r"\s+", " ", sig).strip()
    s = re.sub(r"\s*([<>\[\];:,()])\s*", r"\1", s)
    s = s.replace("&'", "&'").replace("& '", "&'")
    s = re.sub(r"&\s+", "&", s)
    s = re.sub(r",(?=\S)", ", ", s)
    s = re.sub(r":(?=[^:\s])", ": ", s)
    s = s.replace(": :", "::")
    s = re.sub(r"(\w): : ", r"\1::", s)
    s = re.sub(r"\)->", ") -> ", s)
    s = re.sub(r"(\S)->(\S)", r"\1 -> \2", s)
    s = s.replace("&mut", "&mut ").replace("&mut  ", "&mut ")
    s = s.replace("&'a ", "&'a ").replace(", )", ")").replace(",)", ")")
    s = re.sub(r"&'(\w+)(\w)", r"&'\1\2", s)
    s = s.replace(": :", "::")
    s = re.sub(r"(\w)\s*::\s*(\w)", r"\1::\2", s)
    s = s.replace("dyn Host", "dyn Host").replace("&self", "&self")
    s = re.sub(r"\[(\S+); (\d+)\]", r"[\1; \2]", s)
    s = re.sub(r";(?=\S)", "; ", s)
    s = re.sub(r"'_>", "'_>", s)
    s = re.sub(r"&'(\w+)\s+", r"&'\1 ", s)
    return s


def condense(doc_lines: list[str], limit: int = 420) -> str:
    text = " ".join(l.strip() for l in doc_lines if l.strip())
    text = re.sub(r"\s+", " ", text)
    if len(text) > limit:
        cut = text[:limit]
        dot = cut.rfind(". ")
        text = (cut[: dot + 1] if dot > 120 else cut.rstrip() + "...")
    return text


def body_end(line: str) -> int:
    """Index where the signature stops: the `{` that opens the default body."""
    depth = 0
    for i, ch in enumerate(line):
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        elif ch == "{" and depth == 0:
            return i
    return len(line)


def parse_trait(text: str, trait: str) -> list[dict]:
    lines = text.splitlines()
    start = next(i for i, l in enumerate(lines) if l.startswith(f"pub trait {trait}"))
    methods, doc = [], []
    for i in range(start + 1, len(lines)):
        line = lines[i]
        if line.startswith("}"):
            break
        if line.startswith("///"):
            doc.append(line[3:])
            continue
        if line.startswith("fn "):
            sig = line[: body_end(line)].strip()
            name = re.match(r"fn\s+(\w+)", sig).group(1)
            methods.append({"name": name, "signature": squash(sig), "raw": sig, "doc": doc, "line": i + 1})
        doc = []
    return methods


def parse_structs(text: str) -> dict[str, list[str]]:
    structs: dict[str, list[str]] = {}
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        m = re.match(r"pub struct (\w+)(<[^>]*>)?\s*\{", lines[i])
        if m:
            fields = []
            i += 1
            while not lines[i].startswith("}"):
                f = lines[i].strip()
                if f.startswith("pub "):
                    fields.append(squash(f[4:].rstrip(",")))
                i += 1
            structs[m.group(1)] = fields
        i += 1
    return structs


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out-dir", type=Path)
    args = parser.parse_args()
    out_dir = args.out_dir or find_out_dir()
    hooks_rs = (out_dir / "mod_api_generated.rs").read_text(encoding="utf-8")
    host_rs = (out_dir / "mod_api_generated_host.rs").read_text(encoding="utf-8")
    catalog = json.loads((HORIZON / "native-hooks.json").read_text(encoding="utf-8"))
    by_name = {h["name"]: h for h in catalog["hooks"]}
    # Hooks and helpers merged on main but whose API number is assigned at the next stamp
    # ("Pending API @API:name@"): tools/native-hook-fragments/*.json, read by the generator too.
    pending_helpers = 0
    for frag in sorted((HORIZON / "tools/native-hook-fragments").glob("*.json")):
        data = json.loads(frag.read_text(encoding="utf-8"))
        for h in data.get("hooks", []):
            by_name.setdefault(h["name"], dict(h, pending=frag.name))
        pending_helpers += len(data.get("helpers", []))
    commit = subprocess.run(
        ["git", "-C", str(HORIZON), "rev-parse", "--short=9", "HEAD"], capture_output=True, text=True
    ).stdout.strip()
    api = re.search(r"NATIVE_HOOKS_API: u32 = (\d+)", (HORIZON / "crates/horizon-client/src/game/native_hooks.rs").read_text(encoding="utf-8"))
    api = api.group(1) if api else "?"

    hooks = parse_trait(hooks_rs, "Hooks")
    structs = parse_structs(hooks_rs)
    # Fields a mod may change: `&mut` references, or scalars the dispatch copies back (`*x = event.x`).
    written_back: dict[str, set[str]] = {}
    for m in re.finditer(r"^pub fn dispatch_(\w+)\(.*$", hooks_rs, re.M):
        pairs = re.findall(r"\*(\w+) = event\.(\w+)", m.group(0))
        written_back[m.group(1)] = {a for a, b in pairs if a == b}
    host = parse_trait(host_rs, "Host")

    records = []
    sides: dict[str, int] = {}
    for h in hooks:
        entry = by_name.get(h["name"], {})
        side = entry.get("side") or ("unlisted" if not entry else "unspecified")
        fields = structs.get(re.search(r"event:\s*&mut (\w+)", h["signature"]).group(1), []) if "event:" in h["signature"] else []
        mutable = [f.split(":")[0] for f in fields if "&'a mut" in f] + sorted(written_back.get(h["name"], set()))
        sides[side] = sides.get(side, 0) + 1
        contract = entry.get("mod_api", {}).get("contract", "")
        out = entry.get("mod_api", {}).get("out", "")
        # The generated method returns `Flow` exactly when a mod can cancel/claim the call.
        cancellable = h["signature"].endswith("-> Flow")
        ev = re.search(r"event:\s*&mut (\w+)", h["signature"])
        ev_name = ev.group(1) if ev else None
        h.update(
            side=side,
            cancellable=cancellable,
            contract=contract,
            out=out,
            version=entry.get("version"),
            target=entry.get("target", ""),
            neoforge=entry.get("neoforge", ""),
            event=ev_name,
            event_fields=structs.get(ev_name, []) if ev_name else [],
            has_host="host:" in h["signature"] or "level:" in h["signature"],
            in_catalog=bool(entry),
            pending=entry.get("pending", ""),
            mutable_fields=mutable,
        )
        if side != "client" and h["name"] != "render":
            records.append(h)

    DOCS.mkdir(parents=True, exist_ok=True)
    (DOCS / "hooks.json").write_text(
        json.dumps(
            {
                "horizon_commit": commit,
                "native_hooks_api": int(api) if api.isdigit() else api,
                "generated_from": "OUT_DIR/mod_api_generated.rs + native-hooks.json (scripts/horizon-extract-mod-api.py)",
                "cancellable_rule": "the generated method returns Flow (Flow::Cancel sets the hook's out flag and skips later mods)",
                "side_values": "server/both/client from native-hooks.json or a pending fragment (tools/native-hook-fragments, API number assigned at the next stamp: pending_fragment names it); 'unspecified' = listed without a side; 'unlisted' = in the generated trait but in neither. Client-only hooks are omitted.",
                "mutable_fields_rule": "fields typed &'a mut, plus scalars the generated dispatch copies back (*x = event.x)",
                "catalog_cancellable": "the catalog's own `cancellable` field where present",
                "hooks": [
                    {
                        "name": r["name"],
                        "side": r["side"],
                        "cancellable": r["cancellable"],
                        "catalog_cancellable": by_name.get(r["name"], {}).get("cancellable"),
                        "contract": r["contract"],
                        "out": r["out"],
                        "version": r["version"],
                        "signature": r["signature"],
                        "has_host": r["has_host"],
                        "event": r["event"],
                        "event_fields": r["event_fields"],
                        "mutable_fields": r["mutable_fields"],
                        "pending_fragment": r["pending"],
                        "target": r["target"],
                        "neoforge": r["neoforge"],
                    }
                    for r in records
                ],
            },
            indent=1,
        )
        + "\n",
        encoding="utf-8",
    )

    no_host = [r["name"] for r in records if not r["has_host"]]
    md = [
        "# Horizon `Hooks`: server-side methods",
        "",
        f"Generated by `scripts/horizon-extract-mod-api.py` from horizon `{commit}` "
        f"(`NATIVE_HOOKS_API = {api}`): `OUT_DIR/mod_api_generated.rs` (build script of "
        "`horizon-client`, generator `crates/horizon-mod-api-gen`) and `native-hooks.json`. Do not edit by hand: rerun the script.",
        "",
        f"The trait has {len(hooks)} methods (+ `render`), catalog sides: "
        + ", ".join(f"{k} {v}" for k, v in sorted(sides.items()))
        + f". This page lists the {len(records)} that are not client-only: side `server`/`both` from the catalog, "
        "or from a pending fragment (`tools/native-hook-fragments/*.json`: merged on main, API number assigned at the next stamp; "
        "many hooks Rustic needs are there, e.g. `block_place`, `player_move`, `player_pre_login`), "
        "plus `unspecified` (listed without a side) and `unlisted` (in neither). "
        "Check an `unspecified`/`unlisted` hook's call site before relying on it server-side; "
        f"{sum(r['cancellable'] for r in records)} of them are cancellable "
        "(they return `Flow`; `Flow::Cancel` sets the flag named in *out* and skips later mods).",
        "",
        f"**{len(no_host)} of them get no `host`** (only the event): inside them a mod cannot call "
        "`Host` helpers and must defer work (e.g. to the next `server_tick`). They are marked *no host* below.",
        "",
        "Signatures are reproduced exactly (whitespace normalised). Event structs are `#[non_exhaustive]`.",
        "",
    ]
    for r in records:
        flags = [r["side"], f"v{r['version']}"]
        if r["cancellable"]:
            flags.append(f"cancellable ({r['out'] or 'flag'})")
        elif r["contract"]:
            flags.append(r["contract"] + (f" ({r['out']})" if r["out"] else ""))
        if not r["has_host"]:
            flags.append("no host")
        if r["pending"]:
            flags.append(f"pending API ({r['pending']})")
        if r["mutable_fields"]:
            flags.append("mutable: " + ", ".join(r["mutable_fields"]))
        md.append(f"## `{r['name']}`")
        md.append("")
        md.append("*" + ", ".join(flags) + "*" + (f" · NeoForge: {r['neoforge']}" if r["neoforge"] else ""))
        md.append("")
        md.append("```rust")
        md.append(r["signature"])
        if r["event"]:
            md.append(f"pub struct {r['event']} {{ " + ", ".join(r["event_fields"]) + " }")
        md.append("```")
        if r["target"]:
            md.append(f"Call site: `{r['target']}`.")
        doc = condense(r["doc"])
        if doc:
            md.append("")
            md.append(doc)
        md.append("")
    (DOCS / "hooks.md").write_text("\n".join(md) + "\n", encoding="utf-8")

    hd = [
        "# Horizon `Host`: helpers a trait mod calls",
        "",
        f"Generated by `scripts/horizon-extract-mod-api.py` from horizon `{commit}` "
        f"(`NATIVE_HOOKS_API = {api}`): `OUT_DIR/mod_api_generated_host.rs`. "
        f"{len(host)} methods: the catalog's `helpers` list ({len(catalog['helpers'])}) plus pending fragments' helpers "
        f"({pending_helpers}, `tools/native-hook-fragments/*.json`); each forwards to a "
        "`native_*` function of `game/`. Not every `native_*` helper is exposed (e.g. `native_sidebar` is mixin-only). Every default body "
        "panics (`unimplemented!(\"Host::name\")`); the game implements all of them "
        "(`OUT_DIR/mod_api_host_generated.rs`: `impl Host for Game<'_>`, each forwarding to `self.native_<name>`).",
        "",
        "Docs are condensed to their first sentences; read the generated file for the full text.",
        "",
        "| # | Method | Signature | Doc (condensed) |",
        "|---|---|---|---|",
    ]
    for k, m in enumerate(host, 1):
        sig = m["signature"].replace("|", "\\|")
        doc = condense(m["doc"], 300).replace("|", "\\|")
        hd.append(f"| {k} | `{m['name']}` | `{sig}` | {doc} |")
    (DOCS / "host.md").write_text("\n".join(hd) + "\n", encoding="utf-8")
    print(
        f"hooks: {len(hooks)} total, {len(records)} non-client, "
        f"{sum(r['cancellable'] for r in records)} cancellable, {len(no_host)} without host; "
        f"host: {len(host)} methods; sides {sides}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
