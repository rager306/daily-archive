#!/usr/bin/env python3
"""Wave A operator script: catalog vs hybrid body coverage (M241).

Read-only report. Always exits 0 when the audit runs (coverage gaps are data
signals, not script failures). Never authorizes import.

Usage::

    uv run python scripts/verify_etl_body_coverage.py
    uv run python scripts/verify_etl_body_coverage.py --json
    uv run python scripts/verify_etl_body_coverage.py --output /tmp/coverage.json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from research_graph.workflows.composition.etl_body_coverage import (
    DEFAULT_BODY_ROOTS,
    DEFAULT_CATALOG_INDEX,
    DEFAULT_CATALOG_ROOT,
    EtlBodyCoverageRequest,
    run_etl_body_coverage_audit,
)

ROOT = Path(__file__).resolve().parents[1]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Wave A ETL body coverage audit: catalog index vs hybrid.body.md. "
            "Import always false. Exit 0 after successful report generation."
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
    parser.add_argument("--json", action="store_true", help="Print full JSON report")
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Optional path to write JSON report",
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=ROOT,
        help="Repository root for relative paths",
    )
    args = parser.parse_args(argv)

    body_roots = (
        tuple(args.body_root) if args.body_root else DEFAULT_BODY_ROOTS
    )
    result = run_etl_body_coverage_audit(
        EtlBodyCoverageRequest(
            catalog_index_path=args.catalog_index,
            catalog_root=args.catalog_root,
            body_roots=body_roots,
            repo_root=args.repo_root,
        )
    )
    payload = result.to_dict()
    # Hard guarantee for operators reading stdout/files.
    payload["import_eligible"] = False
    payload["graph_writes_allowed"] = False

    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding="utf-8")

    pkg = result.package
    if args.json:
        sys.stdout.write(text)
    else:
        sys.stdout.write(
            "etl-body-coverage | "
            f"articles: {pkg.article_count} | "
            f"hybrid_found: {pkg.hybrid_body_found} | "
            f"hybrid_missing: {pkg.hybrid_body_missing} | "
            f"hybrid_fraction: {pkg.hybrid_body_fraction} | "
            f"hybrid_unique_ids: {pkg.hybrid_body_unique_paper_ids} | "
            f"hybrid_files: {pkg.hybrid_body_artifact_files} | "
            f"article_json_found: {pkg.article_json_found} | "
            f"body_roots: {len(result.body_roots_used)} | "
            f"gaps: {len(pkg.gaps)} | "
            "import_eligible: false\n"
        )
        if pkg.by_source_code:
            parts = [f"{k}={v}" for k, v in sorted(pkg.by_source_code.items())]
            sys.stdout.write("  by_source: " + ", ".join(parts) + "\n")
        if pkg.hybrid_body_files_by_root:
            parts = [
                f"{Path(k).name}={v}"
                for k, v in sorted(pkg.hybrid_body_files_by_root.items())
            ]
            sys.stdout.write("  files_by_root: " + ", ".join(parts) + "\n")
        if getattr(pkg, "multi_root_paper_id_count", 0):
            sys.stdout.write(
                "  multi_root: "
                f"ids={pkg.multi_root_paper_id_count} "
                f"identical={pkg.multi_root_identical_content_count} "
                f"divergent={pkg.multi_root_divergent_content_count}\n"
            )
        if pkg.gaps:
            sys.stdout.write("  gaps: " + ", ".join(pkg.gaps) + "\n")

    # Report generation success always 0; data gaps are not script failures.
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
