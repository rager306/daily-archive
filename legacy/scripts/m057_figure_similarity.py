#!/usr/bin/env python3
"""Compute M057 S03 figure-caption similarity edges from fd embeddings."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from statistics import mean
from typing import Any

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CORPUS = (
    ROOT / "artifacts" / "m057-fd-marker" / "figure-links" / "figure-caption-corpus.json"
)
DEFAULT_EMBEDDINGS = ROOT / "artifacts" / "m057-fd-marker" / "figure-links" / "embeddings.json"
DEFAULT_EDGES = ROOT / "artifacts" / "m057-fd-marker" / "figure-links" / "edges.json"
DEFAULT_SUMMARY = ROOT / "artifacts" / "m057-fd-marker" / "figure-links" / "summary.json"
DEFAULT_THRESHOLD = 0.80

SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "external_network_authorized": False,
    "llm_calls_authorized": False,
}


def load_corpus(path: Path) -> list[dict[str, Any]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    figures = payload.get("figures") if isinstance(payload, dict) else payload
    if not isinstance(figures, list):
        raise ValueError("figure corpus must contain a figures[] list")
    return figures


def load_embeddings(path: Path) -> dict[str, list[float]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    embeddings = payload.get("embeddings") if isinstance(payload, dict) else payload
    if not isinstance(embeddings, dict):
        raise ValueError("embeddings artifact must contain an embeddings object")
    return embeddings


def _similarity_stats(values: list[float]) -> dict[str, float | None]:
    if not values:
        return {"min": None, "max": None, "mean": None}
    return {
        "min": round(min(values), 6),
        "max": round(max(values), 6),
        "mean": round(mean(values), 6),
    }


def compute_similarity_edges(
    *,
    corpus_path: Path = DEFAULT_CORPUS,
    embeddings_path: Path = DEFAULT_EMBEDDINGS,
    edges_path: Path = DEFAULT_EDGES,
    summary_path: Path = DEFAULT_SUMMARY,
    threshold: float = DEFAULT_THRESHOLD,
    include_intra_doc: bool = False,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    figures = load_corpus(corpus_path)
    embedding_map = load_embeddings(embeddings_path)
    figure_by_id = {str(figure["figure_id"]): figure for figure in figures}
    figure_ids = sorted(figure_by_id)
    missing = [figure_id for figure_id in figure_ids if figure_id not in embedding_map]
    if missing:
        raise ValueError(
            f"missing embeddings for {len(missing)} figures; first missing: {missing[0]}"
        )
    matrix = np.asarray([embedding_map[figure_id] for figure_id in figure_ids], dtype=np.float32)
    norms = np.linalg.norm(matrix, axis=1)
    if np.any(norms == 0):
        raise ValueError("zero-length embedding encountered")
    normalized = matrix / norms[:, None]
    similarities = normalized @ normalized.T
    edges: list[dict[str, Any]] = []
    for row_index in range(len(figure_ids)):
        for col_index in range(row_index + 1, len(figure_ids)):
            score = float(similarities[row_index, col_index])
            if score <= threshold:
                continue
            source = figure_by_id[figure_ids[row_index]]
            target = figure_by_id[figure_ids[col_index]]
            relation_type = "intra-doc" if source["arxiv_id"] == target["arxiv_id"] else "inter-doc"
            if relation_type == "intra-doc" and not include_intra_doc:
                continue
            edges.append(
                {
                    "source_figure_id": source["figure_id"],
                    "target_figure_id": target["figure_id"],
                    "source_arxiv_id": source["arxiv_id"],
                    "target_arxiv_id": target["arxiv_id"],
                    "source_figure_idx": source["figure_idx"],
                    "target_figure_idx": target["figure_idx"],
                    "similarity": round(score, 6),
                    "relation_type": relation_type,
                }
            )
    edges.sort(
        key=lambda edge: (-edge["similarity"], edge["source_figure_id"], edge["target_figure_id"])
    )
    values = [float(edge["similarity"]) for edge in edges]
    inter_doc_edges = sum(1 for edge in edges if edge["relation_type"] == "inter-doc")
    intra_doc_edges = len(edges) - inter_doc_edges
    summary: dict[str, Any] = {
        "schema_version": "m057.figure-similarity-summary.v1",
        "safety_defaults": SAFETY_DEFAULTS,
        "threshold": threshold,
        "include_intra_doc": include_intra_doc,
        "total_figures": len(figures),
        "total_pairs": len(figures) * (len(figures) - 1) // 2,
        "edges_total": len(edges),
        "inter_doc_edges": inter_doc_edges,
        "intra_doc_edges": intra_doc_edges,
        "similarity_stats": _similarity_stats(values),
    }
    edge_payload: dict[str, Any] = {
        "schema_version": "m057.figure-similarity-edges.v1",
        "safety_defaults": SAFETY_DEFAULTS,
        "threshold": threshold,
        "include_intra_doc": include_intra_doc,
        "edges": edges,
    }
    edges_path.parent.mkdir(parents=True, exist_ok=True)
    edges_path.write_text(
        json.dumps(edge_payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return edges, summary


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS)
    parser.add_argument("--embeddings", type=Path, default=DEFAULT_EMBEDDINGS)
    parser.add_argument("--edges", type=Path, default=DEFAULT_EDGES)
    parser.add_argument("--summary", type=Path, default=DEFAULT_SUMMARY)
    parser.add_argument("--threshold", type=float, default=DEFAULT_THRESHOLD)
    parser.add_argument("--include-intra-doc", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    edges, summary = compute_similarity_edges(
        corpus_path=args.corpus,
        embeddings_path=args.embeddings,
        edges_path=args.edges,
        summary_path=args.summary,
        threshold=args.threshold,
        include_intra_doc=args.include_intra_doc,
    )
    print(
        json.dumps(
            {
                "edges": len(edges),
                "inter_doc_edges": summary["inter_doc_edges"],
                "intra_doc_edges": summary["intra_doc_edges"],
                "summary": str(args.summary),
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
