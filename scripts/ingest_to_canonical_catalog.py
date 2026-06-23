#!/usr/bin/env python3
"""Canonical article catalog ingest CLI.

Thin wrapper around :mod:`research_graph.infrastructure.corpus.ingestion.catalog_ingest`.
Replaces the legacy ``scripts/m061_ingest_to_canonical_catalog.py`` script.

Behavior matches the M061 S04 CLI:
- Reads M061 anchor-2hop artifacts under ``--m061-root``
- Copies PDFs into canonical catalog under ``--catalog-root``
- Creates ``article.json`` records with safety_flags
- Optionally updates ``article_catalog/index.json`` (skip with ``--no-index``)
- Renders Markdown report at ``artifacts/m061-2hop/s04-ingest-report.md``

External network access (arxiv API for category/title lookup) requires
explicit ``SafetyOverride.external_network_authorized=True``; default is
fail-closed (all fallback metadata, no network call).
"""

from __future__ import annotations

import argparse
import sys
from collections import Counter
from pathlib import Path

from research_graph.infrastructure.corpus.ingestion.catalog_ingest import (
    CATALOG_ROOT_DEFAULT,
    M061_ROOT_DEFAULT,
    REPORT_PATH_DEFAULT,
    SAFETY_OVERRIDE_M061_INGEST,
    IngestOptions,
    SafetyOverride,
    ingest_catalog,
    render_report,
)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Ingest M061-acquired PDFs into the canonical article catalog.",
    )
    parser.add_argument(
        "--m061-root",
        type=Path,
        default=M061_ROOT_DEFAULT,
        help="M061 2-hop artifacts root (default: %(default)s)",
    )
    parser.add_argument(
        "--catalog-root",
        type=Path,
        default=CATALOG_ROOT_DEFAULT,
        help="Canonical article catalog root (default: %(default)s)",
    )
    parser.add_argument(
        "--report-path",
        type=Path,
        default=REPORT_PATH_DEFAULT,
        help="Markdown report output path (default: %(default)s)",
    )
    parser.add_argument(
        "--no-index",
        action="store_true",
        help="Do not update article_catalog/index.json after ingestion",
    )
    parser.add_argument(
        "--no-network",
        action="store_true",
        help="Disable external network (arxiv API) calls; use fallback metadata only",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)

    safety = SAFETY_OVERRIDE_M061_INGEST
    if args.no_network:
        safety = SafetyOverride(
            external_network_authorized=False,
            reason="--no-network CLI flag",
            scope="offline ingest run",
        )

    options = IngestOptions(
        m061_root=args.m061_root,
        arxiv_root=args.catalog_root / "article_catalog" / "arxiv",
        safety_override=safety,
        update_index=not args.no_index,
    )
    result = ingest_catalog(options)

    render_report(result, repo_root=Path.cwd(), report_path=args.report_path)

    status_counts = Counter(record.status for record in result.records)
    fallback_count = sum(1 for record in result.records if record.fallback)
    print(f"processed_pdf_copies={result.discovered_pdf_total}")
    print(f"unique_arxiv_ids={result.unique_arxiv_ids}")
    print(
        f"ingested={status_counts.get('ingested', 0) + status_counts.get('updated', 0) + status_counts.get('metadata_created', 0)}"
    )
    print(f"skipped={status_counts.get('skipped', 0)}")
    print(f"fallback={fallback_count}")
    print(f"arxiv_api_requests={result.api_metrics.requests_made}")
    print(f"arxiv_api_429s={result.api_metrics.rate_limit_429s}")
    print(f"catalog_pdf_count={result.before_catalog_pdf_count}->{result.after_catalog_pdf_count}")
    print(f"report={args.report_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
