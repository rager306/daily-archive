#!/usr/bin/env python3
"""Wave A operator script: preprocess fleet metrics on hybrid bodies (M243).

Runs scholarly preprocess_summary_for_body over unique hybrid.body.md files
(first body root wins per paper_id). Never enables YAKE by default.
Never authorizes import. Exit 0 after successful report generation.

Usage::

    uv run python scripts/verify_etl_preprocess_fleet.py
    uv run python scripts/verify_etl_preprocess_fleet.py --json
    uv run python scripts/verify_etl_preprocess_fleet.py --output /tmp/fleet.json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from research_graph.application.corpus.etl_preprocess_fleet_audit import (
    audit_preprocess_fleet,
)
from research_graph.workflows.composition.etl_body_coverage import DEFAULT_BODY_ROOTS

ROOT = Path(__file__).resolve().parents[1]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Wave A preprocess fleet audit on unique hybrid bodies. "
            "Import always false. Exit 0 after report generation."
        )
    )
    parser.add_argument(
        "--body-root",
        action="append",
        type=Path,
        default=None,
        help="Hybrid body root (repeatable). Default: known m213 runs-live* roots.",
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=ROOT,
        help="Repository root for relative paths",
    )
    parser.add_argument("--json", action="store_true", help="Print full JSON report")
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Optional path to write JSON report",
    )
    parser.add_argument(
        "--sample-limit",
        type=int,
        default=12,
        help="Max sample rows in report",
    )
    args = parser.parse_args(argv)

    repo = Path(args.repo_root)
    raw_roots = tuple(args.body_root) if args.body_root else DEFAULT_BODY_ROOTS
    roots: list[Path] = []
    for r in raw_roots:
        p = Path(r)
        if not p.is_absolute():
            p = (repo / p).resolve()
        roots.append(p)

    package = audit_preprocess_fleet(
        body_roots=roots,
        sample_limit=args.sample_limit,
    )
    payload = package.to_dict()
    payload["import_eligible"] = False
    payload["graph_writes_allowed"] = False
    payload["body_roots"] = [str(r) for r in roots if r.is_dir()]

    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        q = ",".join(f"{k}={v}" for k, v in package.quality_status_counts.items()) or "none"
        lang = ",".join(f"{k}={v}" for k, v in package.language_counts.items()) or "none"
        kw = ",".join(f"{k}={v}" for k, v in package.keyword_source_counts.items()) or "none"
        sys.stdout.write(
            "etl-preprocess-fleet | "
            f"bodies: {package.body_count} | "
            f"errors: {package.error_count} | "
            f"quality: {q} | "
            f"language: {lang} | "
            f"keyword_source: {kw} | "
            "yake: false | "
            "import_eligible: false\n"
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
