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
from typing import Any, cast

from research_graph.infrastructure.corpus.ingestion.catalog_ingest import (
    M056_CUMULATIVE_CORPUS_PATH_DEFAULT,
    CumulativePdfRecord,
    SafetyOverride,
    build_article_record,
    load_m056_corpus,
    update_index_if_exists,
    verify_m056_sha256,
    write_article_record,
)


def _article_claims_offline_m056(article: dict[str, Any], expected_sha256: str) -> bool:
    identity = article.get("identity", {})
    if not isinstance(identity, dict):
        return False
    if identity.get("sha256") != expected_sha256:
        return False
    if identity.get("source_kind") != "m056_cumulative_corpus_local_pdf":
        return False
    variants = article.get("source_variants", [])
    if not isinstance(variants, list):
        return False
    return all(
        isinstance(variant, dict) and variant.get("network_fetch_attempted") is False
        for variant in variants
    )


def _patch_m056_offline_article(
    *,
    article: dict[str, Any],
    record: CumulativePdfRecord,
    pdf_path: Path,
    catalog_root: Path,
    safety: SafetyOverride,
) -> dict[str, Any]:
    """Patch build_article_record output with M056 offline-corpus metadata."""
    arxiv_id = record.arxiv_id
    source_strategy = cast(dict[str, Any], article["source_strategy"])
    identity = cast(dict[str, Any], article["identity"])
    source_variants = cast(list[dict[str, Any]], article["source_variants"])
    expected_profile = cast(dict[str, Any], article["expected_profile"])
    safety_flags = cast(dict[str, Any], article["safety_flags"])

    source_strategy["primary_source_variant_id"] = f"{arxiv_id}:source:m056-cumulative-corpus"
    source_strategy["metadata_order"] = ["m056_cumulative_corpus_json"]
    source_strategy["pdf_policy"] = f"m056_{record.source_milestone}_sha256_verified"
    source_strategy["fallback_policy"] = (
        "use local PDF from M056 cumulative corpus only; no network, graph writes, "
        "or production import is authorized"
    )

    identity["source_kind"] = "m056_cumulative_corpus_local_pdf"
    identity["sha256"] = record.sha256
    identity["size_bytes"] = record.size_bytes
    identity["pages_estimate"] = record.pages_estimate
    identity["source_milestone"] = record.source_milestone

    source_variants[0].update(
        {
            "variant_id": f"{arxiv_id}:source:m056-cumulative-corpus",
            "source_role": "m056_cumulative_corpus_json",
            "source_origin": "local_artifact",
            "path": str(M056_CUMULATIVE_CORPUS_PATH_DEFAULT),
            "url": None,
            "capture_status": "captured_local",
            "capture_policy": "local_m056_cumulative_corpus_json_no_network",
            "loader_outcome": "loaded_metadata_from_cumulative_corpus_json",
            "network_fetch_attempted": False,
        }
    )
    source_variants[1].update(
        {
            "path": str(pdf_path.relative_to(catalog_root)),
            "source_origin": "m056_local_acquisition",
            "capture_policy": "local_copy_from_m056_cumulative_corpus_no_additional_pdf_download",
            "network_fetch_attempted": False,
        }
    )

    article["safety_override"] = {
        "external_network_authorized": False,
        "reason": safety.reason,
        "scope": safety.scope,
    }
    safety_flags["network_fetch_required_for_pipeline_phase"] = False
    expected_profile["synthetic_metadata"] = True
    return article


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

            # Skip only if article.json already exists and fully reflects the
            # offline M056 corpus contract. Older S02 records with matching
            # SHA256 but stale network_fetch_attempted=true must be rewritten.
            if article_path.exists():
                try:
                    existing = json.loads(article_path.read_text())
                    if _article_claims_offline_m056(existing, record.sha256):
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
            article = _patch_m056_offline_article(
                article=article,
                record=record,
                pdf_path=pdf_path,
                catalog_root=catalog_root,
                safety=safety,
            )

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
