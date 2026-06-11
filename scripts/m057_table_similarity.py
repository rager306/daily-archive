#!/usr/bin/env python3
"""Compute M057 S02 table-similarity edges from fd embeddings."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from statistics import mean
from typing import Any

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CORPUS = ROOT / "artifacts" / "m057-fd-marker" / "table-similarity" / "table-text-corpus.json"
DEFAULT_EMBEDDINGS = ROOT / "artifacts" / "m057-fd-marker" / "table-similarity" / "embeddings.json"
DEFAULT_EDGES = ROOT / "artifacts" / "m057-fd-marker" / "table-similarity" / "edges.json"
DEFAULT_SUMMARY = ROOT / "artifacts" / "m057-fd-marker" / "table-similarity" / "summary.json"
DEFAULT_THRESHOLD = 0.85

SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "external_network_authorized": False,
    "llm_calls_authorized": False,
}


def load_corpus(path: Path) -> list[dict[str, Any]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    tables = payload.get("tables") if isinstance(payload, dict) else payload
    if not isinstance(tables, list):
        raise ValueError("table corpus must contain a tables[] list")
    return tables


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
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    tables = load_corpus(corpus_path)
    embedding_map = load_embeddings(embeddings_path)
    table_by_id = {str(table["table_id"]): table for table in tables}
    table_ids = sorted(table_by_id)
    missing = [table_id for table_id in table_ids if table_id not in embedding_map]
    if missing:
        raise ValueError(f"missing embeddings for {len(missing)} tables; first missing: {missing[0]}")
    matrix = np.asarray([embedding_map[table_id] for table_id in table_ids], dtype=np.float32)
    if matrix.ndim != 2 or matrix.shape[1] != 1024:
        raise ValueError(f"expected embedding matrix with 1024 columns, got {matrix.shape}")
    norms = np.linalg.norm(matrix, axis=1)
    if np.any(norms == 0):
        raise ValueError("embedding matrix contains a zero vector")
    normalized = matrix / norms[:, None]
    similarities = normalized @ normalized.T
    upper_rows, upper_cols = np.triu_indices(len(table_ids), k=1)
    candidate_scores = similarities[upper_rows, upper_cols]
    selected = np.flatnonzero(candidate_scores > threshold)

    edges: list[dict[str, Any]] = []
    score_values: list[float] = []
    intra_doc_count = 0
    inter_doc_count = 0
    for selected_index in selected:
        left_index = int(upper_rows[selected_index])
        right_index = int(upper_cols[selected_index])
        score = round(float(candidate_scores[selected_index]), 6)
        left = table_by_id[table_ids[left_index]]
        right = table_by_id[table_ids[right_index]]
        same_paper = left["arxiv_id"] == right["arxiv_id"]
        intra_doc_count += int(same_paper)
        inter_doc_count += int(not same_paper)
        score_values.append(score)
        edges.append(
            {
                "paper_a": left["arxiv_id"],
                "table_a_idx": left["table_idx"],
                "paper_b": right["arxiv_id"],
                "table_b_idx": right["table_idx"],
                "similarity": score,
                "source_a_caption": left.get("caption", ""),
                "source_b_caption": right.get("caption", ""),
                "edge_scope": "intra-doc" if same_paper else "inter-doc",
                "evidence": "fd_cosine_similarity_0.85",
            }
        )
    edges.sort(
        key=lambda edge: (
            edge["paper_a"],
            edge["table_a_idx"],
            edge["paper_b"],
            edge["table_b_idx"],
            -edge["similarity"],
        )
    )
    summary = {
        "schema_version": "m057.table-similarity-summary.v1",
        "safety_defaults": SAFETY_DEFAULTS,
        "threshold": threshold,
        "total_tables": len(table_ids),
        "total_pairs": int(len(table_ids) * (len(table_ids) - 1) / 2),
        "edges_total": len(edges),
        "intra_doc_edges": intra_doc_count,
        "inter_doc_edges": inter_doc_count,
        "similarity_stats": _similarity_stats(score_values),
    }
    edges_payload = {
        "schema_version": "m057.table-similarity-edges.v1",
        "safety_defaults": SAFETY_DEFAULTS,
        "threshold": threshold,
        "edge_count": len(edges),
        "edges": edges,
    }
    edges_path.parent.mkdir(parents=True, exist_ok=True)
    edges_path.write_text(json.dumps(edges_payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return edges, summary


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS)
    parser.add_argument("--embeddings", type=Path, default=DEFAULT_EMBEDDINGS)
    parser.add_argument("--edges", type=Path, default=DEFAULT_EDGES)
    parser.add_argument("--summary", type=Path, default=DEFAULT_SUMMARY)
    parser.add_argument("--threshold", type=float, default=DEFAULT_THRESHOLD)
    args = parser.parse_args()
    _, summary = compute_similarity_edges(
        corpus_path=args.corpus,
        embeddings_path=args.embeddings,
        edges_path=args.edges,
        summary_path=args.summary,
        threshold=args.threshold,
    )
    sys.stdout.write(json.dumps(summary, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
