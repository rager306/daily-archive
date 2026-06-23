#!/usr/bin/env python3
"""Ingest M056 cumulative corpus into the canonical article catalog.

Migrated to use research_graph.infrastructure.corpus.ingestion.catalog_ingest
package (M120). Uses cumulative-corpus.json from M056 BFS graph artifact
(166 pre-positioned PDFs with sha256 + size + pages_estimate + source_milestone
metadata).

Pipeline:
1. load_m056_corpus() -> dict[arxiv_id, CumulativePdfRecord]
2. verify_m056_sha256() -> list[Sha256Mismatch] (must be empty)
3. For each record:
   - build_article_record(arxiv_id, category, title, pdf_path, catalog_root)
   - write_article_record(article_path, article)
   - Patch with cumulative-corpus.json metadata (sha256, size_bytes, etc.)
4. update_index_if_exists() rebuilds article_catalog/index.json

Fail-closed: safety_override.external_network_authorized=False (no arxiv API).
All 165 new records use synthetic titles + metadata (synthetic_only=true).
NO production graph import.
"""

from __future__ import annotations

import json
import sys
from datetime import UTC, datetime
from pathlib import Path

from research_graph.infrastructure.corpus.ingestion.catalog_ingest import (
    M056_CUMULATIVE_CORPUS_PATH_DEFAULT,
    SafetyOverride,
    build_article_record,
    load_m056_corpus,
    update_index_if_exists,
    verify_m056_sha256,
    write_article_record,
)


def main() -> int:
    repo_root = Path("/root/daily-archive")
    corpus = M056_CUMULATIVE_CORPUS_PATH_DEFAULT
    catalog_root = repo_root / "data" / "article_catalog"

    print(f"Loading M056 cumulative corpus from {corpus}...")
    records = load_m056_corpus(corpus, repo_root=repo_root)
    print(f"Loaded {len(records)} PDFs")

    print("Verifying SHA256 against cumulative-corpus.json...")
    mismatches = verify_m056_sha256(records)
    if mismatches:
        print(f"FAIL: {len(mismatches)} SHA256 mismatches; first 3:")
        for m in mismatches[:3]:
            print(
                f"  {m.arxiv_id}: expected={m.expected_sha256[:12]} actual={m.actual_sha256[:12]}"
            )
        return 1
    print("ALL SHA256 match")

    safety = SafetyOverride(
        external_network_authorized=False,
        reason="M121 S02 offline ingest: M056 cumulative corpus SHA256-verified",
        scope="offline catalog expansion (no arxiv API calls)",
    )

    events: list[dict[str, object]] = []
    written_count = 0
    skipped_count = 0
    failed_count = 0

    for arxiv_id, record in sorted(records.items()):
        try:
            pdf_path = record.pdf_path
            article_path = pdf_path.parents[1] / "article.json"

            # Skip if article.json already exists with matching sha256
            if article_path.exists():
                try:
                    existing = json.loads(article_path.read_text())
                    existing_sha = existing.get("identity", {}).get("sha256", "")
                    if existing_sha == record.sha256:
                        skipped_count += 1
                        continue
                except Exception:
                    pass

            title = f"arXiv {arxiv_id} (M056 cumulative corpus, {record.source_milestone})"

            article = build_article_record(
                arxiv_id=arxiv_id,
                category=record.category,
                title=title,
                dest_pdf=pdf_path,
                catalog_root=catalog_root,
            )
            # patch with cumulative-corpus metadata
            article["source_strategy"]["pdf_policy"] = (
                f"m056_{record.source_milestone}_sha256_verified"
            )
            article["identity"]["sha256"] = record.sha256
            article["identity"]["size_bytes"] = record.size_bytes
            article["identity"]["pages_estimate"] = record.pages_estimate
            article["identity"]["source_milestone"] = record.source_milestone
            article["source_variants"][1]["path"] = str(pdf_path.relative_to(catalog_root))
            article["safety_override"] = {
                "external_network_authorized": False,
                "reason": safety.reason,
                "scope": safety.scope,
            }
            # mark synthetic_only
            article["expected_profile"]["synthetic_metadata"] = True

            write_article_record(article_path, article)
            events.append(
                {
                    "event": "ingested",
                    "timestamp": datetime.now(UTC).isoformat(),
                    "arxiv_id": arxiv_id,
                    "category": record.category,
                    "size_bytes": record.size_bytes,
                    "sha256_verified": True,
                }
            )
            written_count += 1
        except Exception as e:
            events.append(
                {
                    "event": "failed",
                    "timestamp": datetime.now(UTC).isoformat(),
                    "arxiv_id": arxiv_id,
                    "error": str(e)[:120],
                }
            )
            failed_count += 1
            print(f"  FAIL {arxiv_id}: {e}")

    # update index.json
    index_updated, index_entries, index_diagnostics = update_index_if_exists(catalog_root)

    # write events log
    events_log = repo_root / "data" / "r024-218-document-corpus-v1" / "ingest-events.jsonl"
    events_log.parent.mkdir(parents=True, exist_ok=True)
    with open(events_log, "w") as f:
        for e in events:
            f.write(json.dumps(e) + "\n")

    # write summary
    summary = {
        "schema_version": "r024-218-ingest-summary.v00.01",
        "generated_at": datetime.now(UTC).isoformat(),
        "corpus": "m056-cumulative",
        "total_records": len(records),
        "ingested_count": written_count,
        "skipped_count": skipped_count,
        "failed_count": failed_count,
        "index_updated": index_updated,
        "index_entries": index_entries,
        "fail_closed_invariants": {
            "network_fetch_attempted": False,
            "production_import_attempted": False,
            "graph_import_allowed": False,
            "ladybugdb_written": False,
            "trusted_kg_import_allowed": False,
            "graph_readiness_claim": False,
            "real_llm_extraction_used": False,
            "synthetic_only": True,
        },
    }
    summary_path = repo_root / "data" / "r024-218-document-corpus-v1" / "ingest-summary.json"
    summary_path.write_text(json.dumps(summary, indent=2))

    print("\n=== Summary ===")
    print(f"  total_records: {len(records)}")
    print(f"  ingested: {written_count}")
    print(f"  skipped: {skipped_count}")
    print(f"  failed: {failed_count}")
    print(f"  index_updated: {index_updated}")
    print(f"  index_entries: {index_entries}")
    print(f"  summary: {summary_path}")
    return 0 if failed_count == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
