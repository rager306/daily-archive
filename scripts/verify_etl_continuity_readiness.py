#!/usr/bin/env python3
"""Wave A operator script: continuity readiness report (M244).

Composes catalog↔hybrid body coverage and preprocess fleet metrics into one
fail-closed report with readiness_signal (blocked|repair|ready_for_review).

readiness_signal is NOT import authorization. Exit 0 after report generation.

Usage::

    uv run python scripts/verify_etl_continuity_readiness.py
    uv run python scripts/verify_etl_continuity_readiness.py --json
    uv run python scripts/verify_etl_continuity_readiness.py --output /tmp/continuity.json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from research_graph.application.corpus.etl_continuity_readiness import (
    build_continuity_readiness,
)
from research_graph.workflows.composition.etl_body_coverage import (
    DEFAULT_BODY_ROOTS,
    DEFAULT_CATALOG_INDEX,
    DEFAULT_CATALOG_ROOT,
)

ROOT = Path(__file__).resolve().parents[1]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Wave A continuity readiness: coverage + preprocess fleet. "
            "Import always false. Exit 0 after report generation."
        )
    )
    parser.add_argument(
        "--catalog-index",
        type=Path,
        default=DEFAULT_CATALOG_INDEX,
        help="Path to article catalog index.json",
    )
    parser.add_argument(
        "--catalog-root",
        type=Path,
        default=DEFAULT_CATALOG_ROOT,
        help="Catalog root for resolving article_path",
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
        help="Max sample rows in nested packages",
    )
    args = parser.parse_args(argv)

    repo = Path(args.repo_root)

    def _resolve(p: Path) -> Path:
        path = Path(p)
        return path if path.is_absolute() else (repo / path).resolve()

    catalog_index = _resolve(args.catalog_index)
    catalog_root = _resolve(args.catalog_root)
    raw_roots = tuple(args.body_root) if args.body_root else DEFAULT_BODY_ROOTS
    body_roots = tuple(_resolve(r) for r in raw_roots)

    package = build_continuity_readiness(
        catalog_index_path=catalog_index,
        catalog_root=catalog_root,
        body_roots=body_roots,
        sample_limit=args.sample_limit,
    )
    payload = package.to_dict()
    payload["import_eligible"] = False
    payload["graph_writes_allowed"] = False

    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding="utf-8")

    cov = package.coverage
    pre = package.preprocess
    if args.json:
        sys.stdout.write(text)
    else:
        q = (
            ",".join(f"{k}={v}" for k, v in pre.quality_status_counts.items())
            or "none"
        )
        sys.stdout.write(
            "etl-continuity-readiness | "
            f"signal: {package.readiness_signal} | "
            f"articles: {cov.article_count} | "
            f"hybrid_found: {cov.hybrid_body_found} | "
            f"hybrid_fraction: {cov.hybrid_body_fraction} | "
            f"preprocess_bodies: {pre.body_count} | "
            f"preprocess_errors: {pre.error_count} | "
            f"quality: {q} | "
            "import_eligible: false\n"
        )
        if cov.gaps:
            sys.stdout.write("  coverage_gaps: " + ", ".join(cov.gaps) + "\n")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
