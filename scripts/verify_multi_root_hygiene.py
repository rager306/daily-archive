#!/usr/bin/env python3
"""Multi-root hybrid body hygiene plan/apply (M267).

Default: plan only. Optional --apply-hardlinks for identical copies.
Never import. Never auto-remove content-bearing files.

Usage::

    uv run python scripts/verify_multi_root_hygiene.py
    uv run python scripts/verify_multi_root_hygiene.py --apply-hardlinks
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from research_graph.application.corpus.multi_root_hygiene import (
    apply_multi_root_hardlinks,
    plan_multi_root_hygiene,
)
from research_graph.workflows.composition.etl_body_coverage import DEFAULT_BODY_ROOTS

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = Path("artifacts/etl/multi-root-hygiene-plan.json")


def _r(repo: Path, p: Path) -> Path:
    return p if p.is_absolute() else (repo / p).resolve()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Plan multi-root hybrid hygiene (identical=hardlink candidate). "
            "Default plan-only. Import always false."
        )
    )
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--body-root",
        action="append",
        default=None,
        help="Body root (repeatable); default DEFAULT_BODY_ROOTS",
    )
    parser.add_argument(
        "--apply-hardlinks",
        action="store_true",
        help="Replace identical duplicates with hardlinks to primary",
    )
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    repo = Path(args.repo_root)

    raw = tuple(args.body_root) if args.body_root else DEFAULT_BODY_ROOTS
    roots = [_r(repo, Path(p)) for p in raw]
    plan = plan_multi_root_hygiene(roots)
    if args.apply_hardlinks:
        plan = apply_multi_root_hardlinks(plan, apply_hardlinks=True)

    payload = plan.to_dict()
    payload["import_eligible"] = False
    payload["graph_writes_allowed"] = False
    out = _r(repo, Path(args.output))
    out.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        sys.stdout.write(
            "multi-root-hygiene | "
            f"multi: {payload['multi_root_paper_id_count']} | "
            f"identical: {payload['identical_content_count']} | "
            f"divergent: {payload['divergent_content_count']} | "
            f"actions: {len(payload.get('actions') or [])} | "
            f"hardlinks: {payload['applied_hardlinks']} | "
            "import_eligible: false\n"
        )
        sys.stdout.write(f"  report: {out}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
