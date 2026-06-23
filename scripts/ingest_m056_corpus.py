#!/usr/bin/env python3
"""Ingest M056 cumulative corpus into the canonical article catalog.

Thin compatibility wrapper around the M122 catalog ingest application use case.
The M056 cumulative corpus has its own offline contract: all PDFs are already
pre-positioned in the canonical catalog layout, SHA256 must match
cumulative-corpus.json before writes, article records use synthetic metadata,
and no network, graph, production import, or LLM activity is authorized.
"""

from __future__ import annotations

import sys
from pathlib import Path

from research_graph.application.corpus.catalog_ingest import (
    CatalogIngestRequest,
    CatalogIngestUseCase,
)
from research_graph.infrastructure.corpus.ingestion.catalog_adapters import (
    M056CumulativeCorpusSourceAssetStore,
    M056FilesystemCatalogRepository,
    M056OfflineMetadataProvider,
    Sha256ChecksumVerifier,
    write_m056_ingest_events,
    write_m056_ingest_summary,
)
from research_graph.infrastructure.corpus.ingestion.catalog_ingest import (
    M056_CUMULATIVE_CORPUS_PATH_DEFAULT,
    SafetyOverride,
)


def main() -> int:
    repo_root = Path(__file__).resolve().parents[1]
    corpus = M056_CUMULATIVE_CORPUS_PATH_DEFAULT
    catalog_root = repo_root / "data" / "article_catalog"

    print(f"Loading M056 cumulative corpus from {corpus}...")
    source_assets = M056CumulativeCorpusSourceAssetStore(corpus, repo_root=repo_root)
    records = source_assets.records
    print(f"Loaded {len(records)} PDFs")

    print("Verifying SHA256 against cumulative-corpus.json...")
    mismatches = source_assets.sha256_mismatches()
    if mismatches:
        print(f"FAIL: {len(mismatches)} SHA256 mismatches; first 3:")
        for mismatch in mismatches[:3]:
            print(
                f"  {mismatch.arxiv_id}: "
                f"expected={mismatch.expected_sha256[:12]} "
                f"actual={mismatch.actual_sha256[:12]}"
            )
        return 1
    print("ALL SHA256 match")

    safety = SafetyOverride(
        external_network_authorized=False,
        reason="M121 S02 offline ingest: M056 cumulative corpus SHA256-verified",
        scope="offline catalog expansion (no arxiv API calls)",
    )

    result = CatalogIngestUseCase(
        source_assets=source_assets,
        metadata_provider=M056OfflineMetadataProvider(records),
        checksum_verifier=Sha256ChecksumVerifier(),
        catalog_repository=M056FilesystemCatalogRepository(
            catalog_root,
            records=records,
            safety_override=safety,
        ),
    ).run(CatalogIngestRequest(update_index=True))

    events_log = repo_root / "data" / "r024-218-document-corpus-v1" / "ingest-events.jsonl"
    write_m056_ingest_events(events_log, result)

    summary_path = repo_root / "data" / "r024-218-document-corpus-v1" / "ingest-summary.json"
    write_m056_ingest_summary(summary_path, result)

    ingested_count = sum(
        count for status, count in result.status_counts.items() if status != "skipped"
    )
    skipped_count = result.status_counts.get("skipped", 0)
    failed_count = len(result.failures)

    print("\n=== Summary ===")
    print(f"  total_records: {result.unique_article_ids}")
    print(f"  ingested: {ingested_count}")
    print(f"  skipped: {skipped_count}")
    print(f"  failed: {failed_count}")
    print(f"  index_updated: {result.index_updated}")
    print(f"  index_entries: {result.index_entries}")
    print(f"  summary: {summary_path}")
    return 0 if result.succeeded else 1


if __name__ == "__main__":
    sys.exit(main())
