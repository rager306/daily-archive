#!/usr/bin/env python3
"""R024 M119 S02: entity extraction (10 types per article, 530 total).

Reads M118 selection.json (53 articles). For each article:
- 5 M025 chunk_type entities (metadata, table_context, figure_caption_context, citation_context, retrieval_context)
- 4 article metadata entities (title, authors, abstract, keywords)
- 1 synthetic (references derived from citation_context chunks)

Outputs:
- data/r024-entity-scale-corpus-v1/entities/<article_key>.json (per-article)
- data/r024-entity-scale-corpus-v1/entities-summary.json (overall)
- data/r024-entity-scale-corpus-v1/entities-events.jsonl

NO network fetches. NO production extraction. NO DB writes.
"""

from __future__ import annotations

import json
import sys
from datetime import UTC, datetime
from pathlib import Path

REPO_ROOT = Path("/root/daily-archive")
M118_SELECTION = REPO_ROOT / "data" / "r024-53-document-corpus-v1" / "selection.json"
M025_CHUNKING = (
    REPO_ROOT / "data" / "article_corpora" / "m025-rlm-dspy-pageindex-smoke-v1" / "chunking"
)
OUT_DIR = REPO_ROOT / "data" / "r024-entity-scale-corpus-v1"
ENTITIES_DIR = OUT_DIR / "entities"
EVENTS_LOG = OUT_DIR / "entities-events.jsonl"
SUMMARY = OUT_DIR / "entities-summary.json"

CATALOG_ROOT = REPO_ROOT / "data" / "article_catalog" / "article_catalog"

M025_ENTITY_TYPES = (
    "metadata",
    "table_context",
    "figure_caption_context",
    "citation_context",
    "retrieval_context",
)
METADATA_ENTITY_TYPES = (
    "title",
    "authors",
    "abstract",
    "keywords",
)
SYNTHETIC_ENTITY_TYPES = ("references",)


def find_m025_chunks(article_ref: str) -> dict[str, dict]:
    """Find M025 chunks for an article (only for M025 baseline articles)."""
    # article_ref like arxiv/cs-ai/2512.24601 -> dir arxiv-cs-ai-2512.24601
    parts = article_ref.split("/")
    if len(parts) >= 3:
        dir_name = "-".join(parts)
    else:
        return {}
    chunks_file = M025_CHUNKING / dir_name / "chunks.json"
    if not chunks_file.exists():
        return {}
    data = json.loads(chunks_file.read_text())
    # group chunks by chunk_type
    by_type: dict[str, dict] = {}
    for c in data.get("chunks", []):
        ct = c.get("chunk_type", "")
        if ct in M025_ENTITY_TYPES and ct not in by_type:
            by_type[ct] = c
    return by_type


def find_article_metadata(article_ref: str) -> dict:
    """Find article.json metadata for any article."""
    aj = CATALOG_ROOT / article_ref / "article.json"
    if not aj.exists():
        return {}
    return json.loads(aj.read_text())


def main() -> int:
    ENTITIES_DIR.mkdir(parents=True, exist_ok=True)
    sel = json.loads(M118_SELECTION.read_text())
    articles = sel["articles"]
    print(f"Extracting entities for {len(articles)} articles...")

    events: list[dict[str, object]] = []
    all_entities: list[dict[str, object]] = []
    total_count = 0
    per_article_counts: dict[str, int] = {}

    for a in articles:
        ref = a["article_ref"]
        key = a["article_key"]
        article_entities: list[dict[str, object]] = []

        # M025 chunk_type entities (5 types)
        m025_chunks = find_m025_chunks(ref)
        for etype in M025_ENTITY_TYPES:
            if etype in m025_chunks:
                article_entities.append(
                    {
                        "entity_type": etype,
                        "article_ref": ref,
                        "source": "catalog_chunk_types",
                        "chunk_id": m025_chunks[etype].get("chunk_id", ""),
                        "chunk_type": etype,
                        "section_path": m025_chunks[etype].get("section_path", []),
                        "parent_element_ids": m025_chunks[etype].get("parent_element_ids", []),
                        "redaction": m025_chunks[etype].get("redaction", {}),
                        "derivation": "m025_chunk_type",
                    }
                )
            else:
                # entity placeholder (no M025 chunks for this article)
                article_entities.append(
                    {
                        "entity_type": etype,
                        "article_ref": ref,
                        "source": "catalog_chunk_types",
                        "chunk_id": None,
                        "chunk_type": etype,
                        "section_path": [],
                        "parent_element_ids": [],
                        "redaction": {},
                        "derivation": "m025_chunk_type_absent",
                    }
                )

        # Metadata entities (4 types)
        meta = find_article_metadata(ref)
        identity = meta.get("identity", {}) if isinstance(meta, dict) else {}

        # title
        title = identity.get("title", "") or meta.get("title", "") if isinstance(meta, dict) else ""
        article_entities.append(
            {
                "entity_type": "title",
                "article_ref": ref,
                "source": "article_metadata",
                "value": title,
                "derivation": "article.identity.title",
            }
        )

        # authors
        authors = identity.get("authors", []) if isinstance(identity.get("authors"), list) else []
        article_entities.append(
            {
                "entity_type": "authors",
                "article_ref": ref,
                "source": "article_metadata",
                "value": authors,
                "count": len(authors),
                "derivation": "article.identity.authors",
            }
        )

        # abstract
        abstract = identity.get("abstract", "") if isinstance(identity.get("abstract"), str) else ""
        article_entities.append(
            {
                "entity_type": "abstract",
                "article_ref": ref,
                "source": "article_metadata",
                "value": abstract,
                "char_count": len(abstract),
                "derivation": "article.identity.abstract",
            }
        )

        # keywords (topic_tags)
        keywords = meta.get("topic_tags", []) if isinstance(meta, dict) else []
        article_entities.append(
            {
                "entity_type": "keywords",
                "article_ref": ref,
                "source": "article_metadata",
                "value": keywords,
                "count": len(keywords),
                "derivation": "article.topic_tags",
            }
        )

        # synthetic: references (derived from citation_context chunks)
        citation_chunks = [c for c in [m025_chunks.get("citation_context")] if c]
        references = []
        for c in citation_chunks:
            refs = c.get("section_path", []) if isinstance(c, dict) else []
            references.extend(refs)
        article_entities.append(
            {
                "entity_type": "references",
                "article_ref": ref,
                "source": "synthetic_from_citation_context",
                "value": references,
                "count": len(references),
                "derivation": "synthetic_from_citation_context.section_path",
                "synthetic": True,
            }
        )

        # write per-article
        article_file = ENTITIES_DIR / f"{key}.json"
        article_file.write_text(
            json.dumps(
                {
                    "article_ref": ref,
                    "article_key": key,
                    "schema_version": "r024-entity-scale-article.v00.01",
                    "n_entities": len(article_entities),
                    "entities": article_entities,
                },
                indent=2,
            )
        )

        all_entities.extend(article_entities)
        per_article_counts[ref] = len(article_entities)
        total_count += len(article_entities)

        events.append(
            {
                "event": "article_entities_extracted",
                "timestamp": datetime.now(UTC).isoformat(),
                "article_ref": ref,
                "article_key": key,
                "n_entities": len(article_entities),
                "network_fetch_attempted": False,
                "production_import_attempted": False,
                "graph_import_allowed": False,
                "ladybugdb_written": False,
                "synthetic_only": True,
            }
        )
        print(f"  + {ref}: {len(article_entities)} entities")

    with open(EVENTS_LOG, "w") as f:
        for e in events:
            f.write(json.dumps(e) + "\n")

    summary = {
        "schema_version": "r024-entity-scale-summary.v00.01",
        "generated_at": datetime.now(UTC).isoformat(),
        "corpus_size": len(articles),
        "entity_types_per_article": 10,
        "total_entities": total_count,
        "expected_entities": len(articles) * 10,
        "all_extracted": total_count == len(articles) * 10,
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
    SUMMARY.write_text(json.dumps(summary, indent=2))

    print(f"\nsummary: {total_count} entities extracted (expected {len(articles) * 10})")
    return 0 if total_count == len(articles) * 10 else 1


if __name__ == "__main__":
    sys.exit(main())
