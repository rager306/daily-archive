#!/usr/bin/env python3
"""Canonical article catalog ingest CLI.

Thin compatibility wrapper around the M122 catalog ingest application use case.
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

from research_graph.application.corpus.catalog_ingest import (
    CatalogIngestRequest,
    CatalogIngestResult,
    CatalogIngestUseCase,
)
from research_graph.infrastructure.corpus.ingestion.catalog_adapters import (
    ArxivCatalogMetadataProvider,
    FilesystemCatalogRepository,
    M061SourceAssetStore,
    Sha256ChecksumVerifier,
)
from research_graph.infrastructure.corpus.ingestion.catalog_ingest import (
    CATALOG_ROOT_DEFAULT,
    M061_ROOT_DEFAULT,
    REPORT_PATH_DEFAULT,
    SAFETY_OVERRIDE_M061_INGEST,
    ApiMetrics,
    IngestRecord,
    IngestResult,
    SafetyOverride,
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


def _legacy_result(
    result: CatalogIngestResult,
    *,
    api_metrics: ApiMetrics,
) -> IngestResult:
    """Convert application result to the legacy report shape.

    `render_report` is still the report compatibility surface for this CLI.
    S02 moves orchestration to the application use case while preserving that
    user-facing markdown report until a later report writer migration replaces
    it deliberately.
    """

    records = [
        IngestRecord(
            arxiv_id=record.article_id,
            anchor_ids=record.anchor_ids,
            source_pdf=Path(record.source_asset_path),
            dest_pdf=Path(record.catalog_asset_path),
            category=record.category,
            title=record.title,
            status=record.status.value,
            fallback=record.fallback,
            source_sha256=record.source_sha256,
            dest_sha256=record.catalog_sha256,
            message=record.message,
        )
        for record in result.records
    ]
    return IngestResult(
        records=records,
        selected_total=result.selected_total,
        discovered_pdf_total=result.discovered_pdf_total,
        unique_arxiv_ids=result.unique_article_ids,
        before_catalog_pdf_count=result.before_catalog_pdf_count,
        after_catalog_pdf_count=result.after_catalog_pdf_count,
        api_metrics=api_metrics,
        index_updated=result.index_updated,
        index_entries=result.index_entries,
        index_diagnostics=[],
    )


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)

    safety = SAFETY_OVERRIDE_M061_INGEST
    if args.no_network:
        safety = SafetyOverride(
            external_network_authorized=False,
            reason="--no-network CLI flag",
            scope="offline ingest run",
        )

    api_metrics = ApiMetrics()
    metadata_provider = ArxivCatalogMetadataProvider(
        safety_override=safety,
        metrics=api_metrics,
    )
    result = CatalogIngestUseCase(
        source_assets=M061SourceAssetStore(args.m061_root),
        metadata_provider=metadata_provider,
        checksum_verifier=Sha256ChecksumVerifier(),
        catalog_repository=FilesystemCatalogRepository(args.catalog_root),
    ).run(CatalogIngestRequest(update_index=not args.no_index))

    report_result = _legacy_result(result, api_metrics=api_metrics)
    render_report(report_result, repo_root=Path.cwd(), report_path=args.report_path)

    status_counts = Counter(record.status.value for record in result.records)
    print(f"processed_pdf_copies={result.discovered_pdf_total}")
    print(f"unique_arxiv_ids={result.unique_article_ids}")
    print(
        f"ingested={status_counts.get('ingested', 0) + status_counts.get('updated', 0) + status_counts.get('metadata_created', 0)}"
    )
    print(f"skipped={status_counts.get('skipped', 0)}")
    print(f"fallback={result.fallback_count}")
    print(f"arxiv_api_requests={api_metrics.requests_made}")
    print(f"arxiv_api_429s={api_metrics.rate_limit_429s}")
    print(f"catalog_pdf_count={result.before_catalog_pdf_count}->{result.after_catalog_pdf_count}")
    print(f"report={args.report_path}")
    return 0 if result.succeeded else 1


if __name__ == "__main__":
    raise SystemExit(main())
