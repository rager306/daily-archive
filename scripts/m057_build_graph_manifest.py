#!/usr/bin/env python3
"""Build the M057 S04 combined content graph manifest.

The manifest is diagnostic-only. It normalizes citation, table-similarity, and
figure-similarity evidence into one edge schema while keeping the five safety
switches explicitly false.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from statistics import mean
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CITATION_EDGES = ROOT / "artifacts" / "m056-bfs-graph" / "candidate-edges.json"
DEFAULT_TABLE_EDGES = ROOT / "artifacts" / "m057-fd-marker" / "table-similarity" / "edges.json"
DEFAULT_FIGURE_EDGES = ROOT / "artifacts" / "m057-fd-marker" / "figure-links" / "edges.json"
DEFAULT_COMBINED_EDGES = ROOT / "artifacts" / "m057-fd-marker" / "combined-edges.json"
DEFAULT_LAYER_SUMMARY = ROOT / "artifacts" / "m057-fd-marker" / "per-layer-summary.json"

SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "external_network_authorized": False,
    "llm_calls_authorized": False,
}


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def stable_evidence_id(layer: str, ordinal: int, source_paper_id: str, target_paper_id: str) -> str:
    return f"m057:{layer}:{ordinal:05d}:{source_paper_id}->{target_paper_id}"


def normalize_citation_edges(payload: dict[str, Any]) -> list[dict[str, Any]]:
    normalized: list[dict[str, Any]] = []
    for ordinal, edge in enumerate(payload.get("edges", []), start=1):
        source_paper_id = str(edge["paper_a"])
        target_paper_id = str(edge["paper_b"])
        normalized.append(
            {
                "source_paper_id": source_paper_id,
                "source_artifact_type": "paper",
                "source_artifact_idx": 0,
                "target_paper_id": target_paper_id,
                "target_artifact_type": "paper",
                "target_artifact_idx": 0,
                "similarity_score": float(edge.get("citation_count", 1)),
                "evidence_layer": "citation",
                "evidence_id": stable_evidence_id("citation", ordinal, source_paper_id, target_paper_id),
            }
        )
    return normalized


def normalize_table_edges(payload: dict[str, Any]) -> list[dict[str, Any]]:
    normalized: list[dict[str, Any]] = []
    for ordinal, edge in enumerate(payload.get("edges", []), start=1):
        source_paper_id = str(edge["paper_a"])
        target_paper_id = str(edge["paper_b"])
        normalized.append(
            {
                "source_paper_id": source_paper_id,
                "source_artifact_type": "table",
                "source_artifact_idx": int(edge["table_a_idx"]),
                "target_paper_id": target_paper_id,
                "target_artifact_type": "table",
                "target_artifact_idx": int(edge["table_b_idx"]),
                "similarity_score": float(edge["similarity"]),
                "evidence_layer": "table_similarity",
                "evidence_id": stable_evidence_id("table", ordinal, source_paper_id, target_paper_id),
            }
        )
    return normalized


def normalize_figure_edges(payload: dict[str, Any]) -> list[dict[str, Any]]:
    normalized: list[dict[str, Any]] = []
    for ordinal, edge in enumerate(payload.get("edges", []), start=1):
        source_paper_id = str(edge["source_arxiv_id"])
        target_paper_id = str(edge["target_arxiv_id"])
        normalized.append(
            {
                "source_paper_id": source_paper_id,
                "source_artifact_type": "figure",
                "source_artifact_idx": int(edge["source_figure_idx"]),
                "target_paper_id": target_paper_id,
                "target_artifact_type": "figure",
                "target_artifact_idx": int(edge["target_figure_idx"]),
                "similarity_score": float(edge["similarity"]),
                "evidence_layer": "figure_similarity",
                "evidence_id": stable_evidence_id("figure", ordinal, source_paper_id, target_paper_id),
            }
        )
    return normalized


def summarize_layer(edges: list[dict[str, Any]]) -> dict[str, Any]:
    scores = [float(edge["similarity_score"]) for edge in edges]
    return {
        "count": len(edges),
        "mean_similarity": round(mean(scores), 6) if scores else 0.0,
        "distinct_source_papers": len({edge["source_paper_id"] for edge in edges}),
        "distinct_target_papers": len({edge["target_paper_id"] for edge in edges}),
    }


def build_manifest(
    citation_edges_path: Path = DEFAULT_CITATION_EDGES,
    table_edges_path: Path = DEFAULT_TABLE_EDGES,
    figure_edges_path: Path = DEFAULT_FIGURE_EDGES,
) -> tuple[dict[str, Any], dict[str, Any]]:
    citation_edges = normalize_citation_edges(load_json(citation_edges_path))
    table_edges = normalize_table_edges(load_json(table_edges_path))
    figure_edges = normalize_figure_edges(load_json(figure_edges_path))

    layers = {
        "citation": citation_edges,
        "table_similarity": table_edges,
        "figure_similarity": figure_edges,
    }
    combined_edges = citation_edges + table_edges + figure_edges

    manifest = {
        "schema_version": "m057.combined-edges.v1",
        "diagnostic_only": True,
        "base_url": "http://127.0.0.1:8000",
        "safety_defaults": SAFETY_DEFAULTS,
        "edge_count": len(combined_edges),
        "edges": combined_edges,
    }
    summary = {
        "schema_version": "m057.per-layer-summary.v1",
        "diagnostic_only": True,
        "base_url": "http://127.0.0.1:8000",
        "safety_defaults": SAFETY_DEFAULTS,
        "total_edges": len(combined_edges),
        "layers": {layer: summarize_layer(edges) for layer, edges in layers.items()},
    }
    return manifest, summary


def run(
    citation_edges_path: Path = DEFAULT_CITATION_EDGES,
    table_edges_path: Path = DEFAULT_TABLE_EDGES,
    figure_edges_path: Path = DEFAULT_FIGURE_EDGES,
    combined_edges_path: Path = DEFAULT_COMBINED_EDGES,
    layer_summary_path: Path = DEFAULT_LAYER_SUMMARY,
) -> tuple[dict[str, Any], dict[str, Any]]:
    manifest, summary = build_manifest(citation_edges_path, table_edges_path, figure_edges_path)
    write_json(combined_edges_path, manifest)
    write_json(layer_summary_path, summary)
    return manifest, summary


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--citation-edges", type=Path, default=DEFAULT_CITATION_EDGES)
    parser.add_argument("--table-edges", type=Path, default=DEFAULT_TABLE_EDGES)
    parser.add_argument("--figure-edges", type=Path, default=DEFAULT_FIGURE_EDGES)
    parser.add_argument("--combined-edges", type=Path, default=DEFAULT_COMBINED_EDGES)
    parser.add_argument("--layer-summary", type=Path, default=DEFAULT_LAYER_SUMMARY)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    _, summary = run(
        citation_edges_path=args.citation_edges,
        table_edges_path=args.table_edges,
        figure_edges_path=args.figure_edges,
        combined_edges_path=args.combined_edges,
        layer_summary_path=args.layer_summary,
    )
    print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
