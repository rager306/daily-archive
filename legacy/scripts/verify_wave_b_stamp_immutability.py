#!/usr/bin/env python3
"""Wave B stamp immutability operator (M257 S05).

Reads durable human_go stamp, attempts write without force_rewrite, asserts
authorized_at unchanged. Never rewrites stamp on the success path.

Usage::

    uv run python scripts/verify_wave_b_stamp_immutability.py
    uv run python scripts/verify_wave_b_stamp_immutability.py --json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from research_graph.application.corpus.wave_b_extraction_baseline import (
    DEFAULT_HUMAN_GO_STAMP,
    read_human_go_stamp,
    write_human_go_stamp,
)

ROOT = Path(__file__).resolve().parents[1]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Verify Wave B human_go stamp refuses mutation without force_rewrite. "
            "Import always false."
        )
    )
    parser.add_argument(
        "--stamp",
        type=Path,
        default=None,
        help=f"Stamp path (default: {DEFAULT_HUMAN_GO_STAMP})",
    )
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--json", action="store_true")
    parser.add_argument(
        "--strict",
        action="store_true",
        help="Exit 1 when stamp missing/invalid or guard fails",
    )
    args = parser.parse_args(argv)

    repo = Path(args.repo_root)
    raw = args.stamp if args.stamp is not None else DEFAULT_HUMAN_GO_STAMP
    stamp_path = raw if Path(raw).is_absolute() else (repo / raw).resolve()

    before = read_human_go_stamp(stamp_path)
    if before is None:
        payload = {
            "stamp_guard": "missing_or_invalid",
            "stamp_path": str(stamp_path),
            "stamp_present": stamp_path.is_file(),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": "No valid human_go stamp; immutability N/A until stamp exists",
        }
        text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
        if args.json:
            sys.stdout.write(text)
        else:
            sys.stdout.write(
                "wave-b-stamp-immutability | stamp_guard: missing_or_invalid | "
                f"path: {stamp_path.name} | import_eligible: false\n"
            )
        return 1 if args.strict else 0

    before_at = before.get("authorized_at")
    before_ref = before.get("decision_ref")
    after_write = write_human_go_stamp(
        stamp_path,
        authorized_by="stamp-immutability-operator",
        decision_ref="D999-should-not-apply",
        note="immutability probe; must not rewrite",
        force_rewrite=False,
    )
    after = read_human_go_stamp(stamp_path)
    ok = (
        after is not None
        and after.get("authorized_at") == before_at
        and after_write.get("authorized_at") == before_at
        and after.get("decision_ref") == before_ref
        and after.get("import_eligible") is not True
    )
    payload = {
        "stamp_guard": "ok" if ok else "failed",
        "stamp_path": str(stamp_path),
        "stamp_present": True,
        "authorized_at": before_at,
        "decision_ref": before_ref,
        "authorized_at_unchanged": bool(
            after and after.get("authorized_at") == before_at
        ),
        "decision_ref_unchanged": bool(
            after and after.get("decision_ref") == before_ref
        ),
        "import_eligible": False,
        "graph_writes_allowed": False,
        "note": (
            "write_human_go_stamp(force_rewrite=False) must return existing stamp "
            "without authorized_at bump"
        ),
    }
    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.json:
        sys.stdout.write(text)
    else:
        sys.stdout.write(
            "wave-b-stamp-immutability | "
            f"stamp_guard: {payload['stamp_guard']} | "
            f"authorized_at_unchanged: {str(payload['authorized_at_unchanged']).lower()} | "
            f"decision_ref: {before_ref} | "
            "import_eligible: false\n"
        )
    if args.strict and not ok:
        return 1
    return 0 if ok else (1 if args.strict else 0)


if __name__ == "__main__":
    raise SystemExit(main())
