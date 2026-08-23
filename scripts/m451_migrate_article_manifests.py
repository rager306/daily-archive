#!/usr/bin/env python3
"""M451 W4: migrate M450 manifest.json (stage ledger) -> manifest.yaml (ADR-068).

The article manifest schema is DEFINED by data/schemas/article_schema.yaml —
this script only transforms legacy data into that shape. Mapping rules:

  stage -> lifecycle event   (parsed->parsed, context->structured,
                              extracted->extracted, enriched->enriched,
                              summaries->summarized)
  graph stages (embedded/materialized/verified) are NOT article lifecycle
  stages — they belong to the graph pipeline ledger, not the article.
  lifecycle is DERIVED from the last event, never hand-set.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

import yaml

STAGE_TO_LIFECYCLE = {
    "parsed": "parsed",
    "context": "structured",
    "extracted": "extracted",
    "enriched": "enriched",
    "summaries": "summarized",
}

LIFECYCLE_ORDER = ["raw", "parsed", "structured", "extracted", "enriched", "summarized", "validated"]

STAGE_ACTORS = {
    "parsed": "grobid",
    "context": "document-context-projection",
    "extracted": "rule-based-extractor",
    "enriched": "openalex-crossref-enricher",
    "summaries": "revision-summarizer",
}

# layer_id -> (dir, file, stage_key, derived_from) — mirrors article_schema.yaml
LAYER_SOURCES = {
    "parsed": ("parsed", "article.json", "parsed", ["source"]),
    "context": ("context", "projection.json", "context", ["parsed"]),
    "extracted": ("extracted", "entities.json", "extracted", ["parsed"]),
}


def migrate_article(article_dir: Path) -> str:
    legacy = json.loads((article_dir / "manifest.json").read_text())

    # 1. lifecycle events from completed stages (graph stages excluded)
    events = []
    for stage_key, lifecycle in STAGE_TO_LIFECYCLE.items():
        entry = legacy.get("stages", {}).get(stage_key, {})
        if entry.get("status") != "complete" or "at" not in entry:
            continue
        events.append({
            "stage": lifecycle,
            "recorded_at": entry["at"],
            "actor": STAGE_ACTORS[stage_key],
            "evidence_sha256": entry.get("output_sha256"),
        })
    events.sort(key=lambda e: e["recorded_at"])

    lifecycle = events[-1]["stage"] if events else "raw"

    # 2. artifact graph from layers actually on disk
    artifacts = {}
    for layer_id, (subdir, filename, stage_key, derived_from) in LAYER_SOURCES.items():
        file_path = article_dir / subdir / filename
        if not file_path.exists():
            continue
        entry = legacy.get("stages", {}).get(stage_key, {})
        stats: dict = {}
        if "sections_count" in entry:
            stats["sections"] = entry["sections_count"]
        if "ast_nodes" in entry:
            stats["ast_nodes"] = entry["ast_nodes"]
        if "entity_count" in entry:
            stats["entities"] = entry["entity_count"]
            if entry.get("entity_types"):
                stats["by_type"] = entry["entity_types"]
        artifacts[layer_id] = {
            "file": f"{subdir}/{filename}",
            "derived_from": derived_from,
            "sha256": entry.get("output_sha256"),
            **({"stats": stats} if stats else {}),
        }

    manifest = {
        "schema_version": "article-manifest.v1",
        "paper_id": legacy["paper_id"],
        "source": legacy.get("source", {}),
        "lifecycle": lifecycle,
        "lifecycle_events": events,
        "artifacts": artifacts,
    }

    out = article_dir / "manifest.yaml"
    out.write_text(yaml.safe_dump(manifest, sort_keys=False, allow_unicode=True))
    return str(out)


def main(argv: list[str]) -> int:
    root = Path(argv[1]).resolve() if len(argv) > 1 else Path("data/processed")
    count, skipped = 0, 0
    for article_dir in sorted(root.iterdir()):
        if not article_dir.is_dir() or not (article_dir / "manifest.json").exists():
            continue
        if (article_dir / "manifest.yaml").exists():
            skipped += 1  # append-only: never overwrite an existing v1 manifest
            continue
        migrate_article(article_dir)
        count += 1
    print(f"migrated: {count}, skipped (already migrated): {skipped}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
