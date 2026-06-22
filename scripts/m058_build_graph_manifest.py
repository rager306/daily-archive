#!/usr/bin/env python3
"""Build the M058 S05 combined pilot graph manifest.

This manifest is diagnostic-only. It combines M056 citation edges, M057 table
and figure similarity edges, and the M058 plotextractor v2 figure layer into a
single normalized schema. The five safety switches are intentionally explicit
and false.
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
DEFAULT_FIGURE_V1_EDGES = ROOT / "artifacts" / "m057-fd-marker" / "figure-links" / "edges.json"
DEFAULT_FIGURE_V2_EDGES = ROOT / "artifacts" / "m058-plotextractor" / "edges.json"
DEFAULT_OUTPUT_DIR = ROOT / "artifacts" / "m058-pilot"
DEFAULT_COMBINED_EDGES = DEFAULT_OUTPUT_DIR / "combined-edges.json"
DEFAULT_LAYER_SUMMARY = DEFAULT_OUTPUT_DIR / "per-layer-summary.json"

LOOPBACK_BIND_HOST = "127.0.0.1"
SAFETY_DEFAULTS: dict[str, bool] = {
    "external_network_authorized": False,
    "fact_promotion_authorized": False,
    "graph_writes_authorized": False,
    "llm_calls_authorized": False,
    "production_import_authorized": False,
}

LAYER_ORDER = ("citation", "table_similarity", "figure_similarity_v1", "figure_similarity_v2")


def load_edges(path: Path) -> list[dict[str, Any]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if isinstance(payload, dict):
        edges = payload.get("edges")
        if not isinstance(edges, list):
            raise ValueError(f"{path} does not contain an edges list")
        return edges
    if isinstance(payload, list):
        return payload
    raise ValueError(f"{path} must be a JSON object or list")


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def _as_int_or_none(value: Any) -> int | None:
    return value if isinstance(value, int) else None


def _as_float_or_none(value: Any) -> float | None:
    if isinstance(value, int | float):
        return round(float(value), 6)
    return None


def _stable_evidence_id(
    layer: str, ordinal: int, source_paper_id: str, target_paper_id: str
) -> str:
    return f"m058:{layer}:{ordinal:05d}:{source_paper_id}->{target_paper_id}"


def normalize_edge(edge: dict[str, Any], *, layer: str, ordinal: int) -> dict[str, Any]:
    if layer == "citation":
        source_paper_id = str(edge["paper_a"])
        target_paper_id = str(edge["paper_b"])
        source_artifact_type = "paper"
        target_artifact_type = "paper"
        source_artifact_idx = None
        target_artifact_idx = None
        similarity_score = None
        relation_type = edge.get("edge_type", "cites")
        source_artifact_id = source_paper_id
        target_artifact_id = target_paper_id
    elif layer == "table_similarity":
        source_paper_id = str(edge["paper_a"])
        target_paper_id = str(edge["paper_b"])
        source_artifact_type = "table"
        target_artifact_type = "table"
        source_artifact_idx = _as_int_or_none(edge.get("table_a_idx"))
        target_artifact_idx = _as_int_or_none(edge.get("table_b_idx"))
        similarity_score = _as_float_or_none(edge.get("similarity"))
        relation_type = edge.get("edge_scope", "similarity")
        source_artifact_id = f"{source_paper_id}::table::{source_artifact_idx}"
        target_artifact_id = f"{target_paper_id}::table::{target_artifact_idx}"
    elif layer == "figure_similarity_v1":
        source_paper_id = str(edge["source_arxiv_id"])
        target_paper_id = str(edge["target_arxiv_id"])
        source_artifact_type = "figure_caption_regex"
        target_artifact_type = "figure_caption_regex"
        source_artifact_idx = _as_int_or_none(edge.get("source_figure_idx"))
        target_artifact_idx = _as_int_or_none(edge.get("target_figure_idx"))
        similarity_score = _as_float_or_none(edge.get("similarity"))
        relation_type = edge.get("relation_type", "similarity")
        source_artifact_id = str(
            edge.get("source_figure_id", f"{source_paper_id}::{source_artifact_idx}")
        )
        target_artifact_id = str(
            edge.get("target_figure_id", f"{target_paper_id}::{target_artifact_idx}")
        )
    elif layer == "figure_similarity_v2":
        source_paper_id = str(edge["paper_a"])
        target_paper_id = str(edge["paper_b"])
        source_artifact_type = "figure_caption_tex"
        target_artifact_type = "figure_caption_tex"
        source_artifact_idx = _as_int_or_none(edge.get("figure_a_idx"))
        target_artifact_idx = _as_int_or_none(edge.get("figure_b_idx"))
        similarity_score = _as_float_or_none(edge.get("similarity"))
        relation_type = edge.get("relation_type", "similarity")
        source_artifact_id = str(
            edge.get("figure_a_id", f"{source_paper_id}::{source_artifact_idx}")
        )
        target_artifact_id = str(
            edge.get("figure_b_id", f"{target_paper_id}::{target_artifact_idx}")
        )
    else:
        raise ValueError(f"Unknown evidence layer: {layer}")

    return {
        "evidence_id": _stable_evidence_id(layer, ordinal, source_paper_id, target_paper_id),
        "evidence_layer": layer,
        "relation_type": relation_type,
        "similarity_score": similarity_score,
        "source_artifact_id": source_artifact_id,
        "source_artifact_idx": source_artifact_idx,
        "source_artifact_type": source_artifact_type,
        "source_paper_id": source_paper_id,
        "target_artifact_id": target_artifact_id,
        "target_artifact_idx": target_artifact_idx,
        "target_artifact_type": target_artifact_type,
        "target_paper_id": target_paper_id,
    }


def summarize_layer(layer: str, edges: list[dict[str, Any]]) -> dict[str, Any]:
    similarity_scores = [
        edge["similarity_score"] for edge in edges if edge["similarity_score"] is not None
    ]
    return {
        "count": len(edges),
        "distinct_source_papers": len({edge["source_paper_id"] for edge in edges}),
        "distinct_target_papers": len({edge["target_paper_id"] for edge in edges}),
        "evidence_layer": layer,
        "mean_similarity": round(mean(similarity_scores), 6) if similarity_scores else None,
        "similarity_edge_count": len(similarity_scores),
    }


def build_graph_manifest(
    *,
    citation_edges_path: Path = DEFAULT_CITATION_EDGES,
    table_edges_path: Path = DEFAULT_TABLE_EDGES,
    figure_v1_edges_path: Path = DEFAULT_FIGURE_V1_EDGES,
    figure_v2_edges_path: Path = DEFAULT_FIGURE_V2_EDGES,
    combined_edges_path: Path = DEFAULT_COMBINED_EDGES,
    layer_summary_path: Path = DEFAULT_LAYER_SUMMARY,
) -> tuple[dict[str, Any], dict[str, Any]]:
    raw_sources = {
        "citation": load_edges(citation_edges_path),
        "table_similarity": load_edges(table_edges_path),
        "figure_similarity_v1": load_edges(figure_v1_edges_path),
        "figure_similarity_v2": load_edges(figure_v2_edges_path),
    }

    normalized_by_layer: dict[str, list[dict[str, Any]]] = {}
    normalized_edges: list[dict[str, Any]] = []
    for layer in LAYER_ORDER:
        layer_edges = [
            normalize_edge(edge, layer=layer, ordinal=ordinal)
            for ordinal, edge in enumerate(raw_sources[layer], start=1)
        ]
        normalized_by_layer[layer] = layer_edges
        normalized_edges.extend(layer_edges)

    layer_summaries = [summarize_layer(layer, normalized_by_layer[layer]) for layer in LAYER_ORDER]
    layer_summary = {
        "layer_count": len(LAYER_ORDER),
        "layers": layer_summaries,
        "loopback_bind_host": LOOPBACK_BIND_HOST,
        "safety_defaults": SAFETY_DEFAULTS,
        "schema_version": "m058.combined-graph.per-layer-summary.v1",
        "source_files": {
            "citation": str(
                citation_edges_path.relative_to(ROOT)
                if citation_edges_path.is_relative_to(ROOT)
                else citation_edges_path
            ),
            "figure_similarity_v1": str(
                figure_v1_edges_path.relative_to(ROOT)
                if figure_v1_edges_path.is_relative_to(ROOT)
                else figure_v1_edges_path
            ),
            "figure_similarity_v2": str(
                figure_v2_edges_path.relative_to(ROOT)
                if figure_v2_edges_path.is_relative_to(ROOT)
                else figure_v2_edges_path
            ),
            "table_similarity": str(
                table_edges_path.relative_to(ROOT)
                if table_edges_path.is_relative_to(ROOT)
                else table_edges_path
            ),
        },
        "total_edges": len(normalized_edges),
    }
    combined_manifest = {
        "edge_count": len(normalized_edges),
        "edges": normalized_edges,
        "layer_order": list(LAYER_ORDER),
        "loopback_bind_host": LOOPBACK_BIND_HOST,
        "safety_defaults": SAFETY_DEFAULTS,
        "schema_version": "m058.combined-graph.edges.v1",
    }

    write_json(combined_edges_path, combined_manifest)
    write_json(layer_summary_path, layer_summary)
    return combined_manifest, layer_summary


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--citation-edges", type=Path, default=DEFAULT_CITATION_EDGES)
    parser.add_argument("--table-edges", type=Path, default=DEFAULT_TABLE_EDGES)
    parser.add_argument("--figure-v1-edges", type=Path, default=DEFAULT_FIGURE_V1_EDGES)
    parser.add_argument("--figure-v2-edges", type=Path, default=DEFAULT_FIGURE_V2_EDGES)
    parser.add_argument("--combined-edges", type=Path, default=DEFAULT_COMBINED_EDGES)
    parser.add_argument("--layer-summary", type=Path, default=DEFAULT_LAYER_SUMMARY)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    combined_manifest, layer_summary = build_graph_manifest(
        citation_edges_path=args.citation_edges,
        table_edges_path=args.table_edges,
        figure_v1_edges_path=args.figure_v1_edges,
        figure_v2_edges_path=args.figure_v2_edges,
        combined_edges_path=args.combined_edges,
        layer_summary_path=args.layer_summary,
    )
    print(
        json.dumps(
            {
                "combined_edges": str(args.combined_edges),
                "edge_count": combined_manifest["edge_count"],
                "layer_count": layer_summary["layer_count"],
                "layer_summary": str(args.layer_summary),
                "safety_defaults": SAFETY_DEFAULTS,
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
