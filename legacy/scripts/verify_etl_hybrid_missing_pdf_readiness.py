#!/usr/bin/env python3
"""Wave A operator: hybrid-missing catalog papers vs local PDF readiness.

Read-only expand queue signal. Never starts hybrid batch. Never import.

Usage::

    uv run python scripts/verify_etl_hybrid_missing_pdf_readiness.py
    uv run python scripts/verify_etl_hybrid_missing_pdf_readiness.py --json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from research_graph.application.corpus.etl_hybrid_missing_pdf_readiness import (
    audit_hybrid_missing_pdf_readiness,
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
            "Wave A: among catalog articles without hybrid body, count local PDFs. "
            "Import always false. Exit 0 after successful report."
        )
    )
    parser.add_argument("--catalog-index", type=Path, default=DEFAULT_CATALOG_INDEX)
    parser.add_argument("--catalog-root", type=Path, default=DEFAULT_CATALOG_ROOT)
    parser.add_argument(
        "--body-root",
        action="append",
        type=Path,
        default=None,
        help="Hybrid body root (repeatable). Default: known m213 runs-live* roots.",
    )
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--sample-limit", type=int, default=12)
    args = parser.parse_args(argv)

    repo = Path(args.repo_root)
    catalog_index = (
        args.catalog_index
        if args.catalog_index.is_absolute()
        else (repo / args.catalog_index)
    )
    catalog_root = (
        args.catalog_root
        if args.catalog_root.is_absolute()
        else (repo / args.catalog_root)
    )
    if args.body_root:
        body_roots = tuple(
            r if r.is_absolute() else (repo / r) for r in args.body_root
        )
    else:
        body_roots = tuple(
            (r if Path(r).is_absolute() else (repo / r)) for r in DEFAULT_BODY_ROOTS
        )
    pkg = audit_hybrid_missing_pdf_readiness(
        catalog_index_path=catalog_index,
        catalog_root=catalog_root,
        body_roots=body_roots,
        sample_limit=int(args.sample_limit),
    )
    payload = pkg.to_dict()
    payload["import_eligible"] = False
    payload["graph_writes_allowed"] = False
    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        out = args.output if args.output.is_absolute() else (repo / args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")
    if args.json:
        sys.stdout.write(text)
    else:
        sys.stdout.write(
            "etl-hybrid-missing-pdf-readiness | "
            f"articles: {pkg.article_count} | "
            f"hybrid_found: {pkg.hybrid_found_count} | "
            f"hybrid_missing: {pkg.hybrid_missing_count} | "
            f"missing_with_pdf: {pkg.missing_with_local_pdf_count} | "
            f"missing_without_pdf: {pkg.missing_without_local_pdf_count} | "
            f"expand_ready_frac: {pkg.expand_ready_fraction_of_missing} | "
            "import_eligible: false\n"
        )
        if pkg.expand_ready_sample:
            sample = ", ".join(s.paper_id for s in pkg.expand_ready_sample[:5])
            sys.stdout.write(f"  expand_ready_sample: {sample}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

