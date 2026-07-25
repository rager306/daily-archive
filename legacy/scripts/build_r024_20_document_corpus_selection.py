#!/usr/bin/env python3
"""R024 20-doc corpus selection (M117 S01).

Builds 20-article corpus from M116 baseline (10) + 10 new candidates.
Reuses M116 selection framework.
"""

from __future__ import annotations

import json
from datetime import UTC, datetime
from pathlib import Path

REPO_ROOT = Path("/root/daily-archive")
M116_SELECTION = REPO_ROOT / "data" / "r024-10-document-corpus-v1" / "selection.json"
OUT_DIR = REPO_ROOT / "data" / "r024-20-document-corpus-v1"
OUT_SELECTION = OUT_DIR / "selection.json"
OUT_EVENTS = OUT_DIR / "selection-events.jsonl"
OUT_SUMMARY = OUT_DIR / "selection-summary.json"

# 10 new candidates from M117 S01 (diverse topics, all with local sources)
NEW_CANDIDATES = [
    {
        "article_ref": "arxiv/cs-cl/2605.18211",
        "article_key": "2605.18211",
        "source_code": "arxiv",
        "seed_url": "https://arxiv.org/abs/2605.18211",
        "selection_role": "graph_seq2seq_extension",
        "selection_source": "m117-r024-20-document-corpus-v1",
        "topic_tags": ["graph-neural-networks", "seq2seq", "knowledge-graph"],
    },
    {
        "article_ref": "arxiv/cs-cv/1804.02767",
        "article_key": "1804.02767",
        "source_code": "arxiv",
        "seed_url": "https://arxiv.org/abs/1804.02767",
        "selection_role": "yolov3_baseline",
        "selection_source": "m117-r024-20-document-corpus-v1",
        "topic_tags": ["computer-vision", "object-detection", "yolo"],
    },
    {
        "article_ref": "arxiv/cs-lg/2111.00396",
        "article_key": "2111.00396",
        "source_code": "arxiv",
        "seed_url": "https://arxiv.org/abs/2111.00396",
        "selection_role": "structured_state_space",
        "selection_source": "m117-r024-20-document-corpus-v1",
        "topic_tags": ["long-sequences", "state-space-models"],
    },
    {
        "article_ref": "arxiv/mixed-source/2603.04448",
        "article_key": "2603.04448",
        "source_code": "arxiv",
        "seed_url": "https://arxiv.org/abs/2603.04448",
        "selection_role": "skill_net_ai_skills",
        "selection_source": "m117-r024-20-document-corpus-v1",
        "topic_tags": ["ai-skills", "evaluation"],
    },
    {
        "article_ref": "arxiv/cs-cl/2511.20639",
        "article_key": "2511.20639",
        "source_code": "arxiv",
        "seed_url": "https://arxiv.org/abs/2511.20639",
        "selection_role": "multi_agent_latent",
        "selection_source": "m117-r024-20-document-corpus-v1",
        "topic_tags": ["multi-agent", "latent-collaboration"],
    },
    {
        "article_ref": "arxiv/cs-lg/2203.14465",
        "article_key": "2203.14465",
        "source_code": "arxiv",
        "seed_url": "https://arxiv.org/abs/2203.14465",
        "selection_role": "star_reasoning_bootstrapping",
        "selection_source": "m117-r024-20-document-corpus-v1",
        "topic_tags": ["reasoning", "bootstrapping"],
    },
    {
        "article_ref": "arxiv/mixed-source/2605.21401",
        "article_key": "2605.21401",
        "source_code": "arxiv",
        "seed_url": "https://arxiv.org/abs/2605.21401",
        "selection_role": "llm_safety_empirical",
        "selection_source": "m117-r024-20-document-corpus-v1",
        "topic_tags": ["llm-safety", "empirical-study"],
    },
    {
        "article_ref": "arxiv/mixed-source/2605.25522",
        "article_key": "2605.25522",
        "source_code": "arxiv",
        "seed_url": "https://arxiv.org/abs/2605.25522",
        "selection_role": "graph_ann_search",
        "selection_source": "m117-r024-20-document-corpus-v1",
        "topic_tags": ["approximate-nearest-neighbor", "graph-index"],
    },
    {
        "article_ref": "arxiv/mixed-source/2605.20897",
        "article_key": "2605.20897",
        "source_code": "arxiv",
        "seed_url": "https://arxiv.org/abs/2605.20897",
        "selection_role": "graph_fairness_connectivity",
        "selection_source": "m117-r024-20-document-corpus-v1",
        "topic_tags": ["graph-fairness", "connectivity"],
    },
    {
        "article_ref": "arxiv/mixed-source/2604.18478",
        "article_key": "2604.18478",
        "source_code": "arxiv",
        "seed_url": "https://arxiv.org/abs/2604.18478",
        "selection_role": "worlddb_graph_memory",
        "selection_source": "m117-r024-20-document-corpus-v1",
        "topic_tags": ["vector-graph", "world-model", "ontology"],
    },
]


def main() -> int:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    sel_m116 = json.loads(M116_SELECTION.read_text())
    m116_articles = sel_m116["articles"]

    # Combine: 10 from M116 + 10 new
    all_articles = []
    for a in m116_articles:
        ca = dict(a)
        ca["selection_role"] = "m116_baseline"
        ca["selection_source"] = "m116-r024-10-document-corpus-v1"
        all_articles.append(ca)
    for a in NEW_CANDIDATES:
        all_articles.append(a)

    # Verify uniqueness
    keys = [a["article_key"] for a in all_articles]
    if len(set(keys)) != len(keys):
        dups = [k for k in keys if keys.count(k) > 1]
        print(f"DUPLICATE keys: {dups}")
        return 1
    if len(all_articles) != 20:
        print(f"Expected 20, got {len(all_articles)}")
        return 1

    selection = {
        "schema_version": "article-corpus-selection.v00.02",
        "selection_id": "m117-r024-20-document-corpus-v1",
        "catalog_schema_version": "article-catalog.v00.01",
        "article_schema_version": "article.v00.01",
        "purpose": "R024 second-stage 20-document corpus validation (M117).",
        "baseline_corpus": "m116-r024-10-document-corpus-v1",
        "network_policy": "test_phase_must_not_fetch",
        "graph_import_allowed": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
        "selection_counts": {
            "baseline_m116": 10,
            "extension_m117": 10,
            "total": 20,
        },
        "articles": all_articles,
    }
    OUT_SELECTION.write_text(json.dumps(selection, indent=2))
    print(f"selection.json: {len(all_articles)} articles (10 baseline + 10 extension)")

    # Events
    events: list[dict[str, object]] = [
        {
            "event": "selection_start",
            "timestamp": datetime.now(UTC).isoformat(),
            "milestone": "M117-hoqwxd",
            "slice": "S01",
            "schema_version": "r024-20-document-selection-event.v00.01",
        }
    ]
    for a in all_articles:
        events.append(
            {
                "event": "article_selected",
                "timestamp": datetime.now(UTC).isoformat(),
                "article_ref": a["article_ref"],
                "article_key": a["article_key"],
                "selection_role": a.get("selection_role", ""),
                "selection_source": a.get("selection_source", ""),
                "network_fetch_attempted": False,
                "graph_import_allowed": False,
                "ladybugdb_written": False,
            }
        )
    events.append(
        {
            "event": "selection_complete",
            "timestamp": datetime.now(UTC).isoformat(),
            "total_articles": len(all_articles),
            "schema_version": "r024-20-document-selection-event.v00.01",
        }
    )
    with open(OUT_EVENTS, "w") as f:
        for e in events:
            f.write(json.dumps(e) + "\n")
    print(f"events.jsonl: {len(events)} events")

    # Summary
    summary = {
        "schema_version": "r024-20-document-selection-summary.v00.01",
        "generated_at": datetime.now(UTC).isoformat(),
        "total_articles": len(all_articles),
        "baseline_m116_count": 10,
        "extension_m117_count": 10,
        "unique_keys": len(set(keys)),
        "all_local_sources": True,
        "network_fetch_attempted": False,
        "graph_import_allowed": False,
        "ladybugdb_written": False,
    }
    OUT_SUMMARY.write_text(json.dumps(summary, indent=2))
    print("summary.json: 20 articles unique, all local sources")
    return 0


if __name__ == "__main__":
    import sys

    sys.exit(main())
