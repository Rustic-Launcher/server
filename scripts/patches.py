#!/usr/bin/env python3
"""Rustic's patches on Horizon, in the spirit of Paper's paperweight (concept only).

Horizon lives in the `horizon/` submodule, pinned to the commit recorded by the superproject.
Our changes to it are kept as numbered `git format-patch` files in `patches/`, applied on top of
that commit on a local branch `rustic-patched`. Nothing is ever committed to Horizon's history
we push anywhere: the submodule pointer stays on the upstream commit.

  python3 scripts/patches.py apply     # reset horizon/ to the pinned commit, apply patches/*.patch
  python3 scripts/patches.py rebuild   # export the commits on rustic-patched back to patches/
  python3 scripts/patches.py check     # verify every patch applies cleanly (CI)
  python3 scripts/patches.py status    # pinned commit, branch, patches

Workflow to change a patch: `apply`, edit in horizon/, `git -C horizon commit` (amend or new
commit, one concern per commit, message `rustic: <what>`), then `rebuild`.
"""
from __future__ import annotations

import argparse
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HORIZON = ROOT / "horizon"
PATCHES = ROOT / "patches"
BRANCH = "rustic-patched"


def git(*args: str, cwd: Path = HORIZON, check: bool = True, capture: bool = True) -> str:
    result = subprocess.run(
        ["git", *args], cwd=cwd, check=False, text=True,
        stdout=subprocess.PIPE if capture else None, stderr=subprocess.PIPE if capture else None,
    )
    if check and result.returncode != 0:
        sys.exit(f"git {' '.join(args)} failed:\n{result.stderr}")
    return (result.stdout or "").strip()


def pinned_commit() -> str:
    """The Horizon commit the superproject records for the submodule."""
    line = git("ls-tree", "HEAD", "horizon", cwd=ROOT, check=False)
    if line:
        return line.split()[2]
    # Not committed yet in the superproject: use the submodule's current upstream HEAD.
    return git("rev-parse", "HEAD")


def patch_files() -> list[Path]:
    return sorted(PATCHES.glob("*.patch"))


def ensure_clean() -> None:
    if git("status", "--porcelain", "--untracked-files=no"):
        sys.exit("horizon/ has uncommitted changes: commit them (then `rebuild`) or stash them first.")


def cmd_apply(_: argparse.Namespace) -> None:
    ensure_clean()
    base = pinned_commit()
    git("checkout", "-B", BRANCH, base)
    patches = patch_files()
    if patches:
        out = subprocess.run(
            ["git", "am", "--3way", "--keep-cr", *map(str, patches)], cwd=HORIZON, text=True,
            capture_output=True,
        )
        if out.returncode != 0:
            print(out.stdout, out.stderr, sep="\n")
            sys.exit("A patch failed to apply: fix it in horizon/ (git am --continue) then `rebuild`.")
    print(f"horizon/ is on {BRANCH}: {base[:10]} + {len(patches)} patch(es)")


def cmd_rebuild(_: argparse.Namespace) -> None:
    ensure_clean()
    base = pinned_commit()
    for old in patch_files():
        old.unlink()
    PATCHES.mkdir(exist_ok=True)
    git("format-patch", "--zero-commit", "--no-signature", "--no-stat", "-N",
        "-o", str(PATCHES), f"{base}..HEAD")
    print(f"{len(patch_files())} patch(es) written to patches/ against {base[:10]}")


def cmd_check(_: argparse.Namespace) -> None:
    base = pinned_commit()
    patches = patch_files()
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp) / "wt"
        git("worktree", "add", "--detach", str(work), base)
        try:
            for patch in patches:
                result = subprocess.run(
                    ["git", "apply", "--check", "--3way", str(patch)], cwd=work, text=True,
                    capture_output=True,
                )
                if result.returncode != 0:
                    sys.exit(f"{patch.name} does not apply on {base[:10]}:\n{result.stderr}")
                subprocess.run(["git", "apply", "--3way", str(patch)], cwd=work, check=True,
                               capture_output=True)
        finally:
            git("worktree", "remove", "--force", str(work))
    print(f"OK: {len(patches)} patch(es) apply on {base[:10]}")


def cmd_status(_: argparse.Namespace) -> None:
    print("pinned :", pinned_commit())
    print("HEAD   :", git("rev-parse", "HEAD"), f"({git('rev-parse', '--abbrev-ref', 'HEAD')})")
    for patch in patch_files():
        print("patch  :", patch.name)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(required=True)
    for name, fn in [("apply", cmd_apply), ("rebuild", cmd_rebuild), ("check", cmd_check), ("status", cmd_status)]:
        sub.add_parser(name).set_defaults(func=fn)
    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
