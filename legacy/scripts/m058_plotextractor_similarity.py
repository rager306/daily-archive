#!/usr/bin/env python3
"""Compute M058 plotextractor v2 figure-caption similarity edges."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from statistics import mean
from typing import Any

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
ARTIFACT_ROOT = ROOT / "artifacts" / "m058-plotextractor"
DEFAULT_CORPUS = ARTIFACT_ROOT / "figure-caption-corpus.json"
DEFAULT_EMBEDDINGS = ARTIFACT_ROOT / "embeddings.json"
DEFAULT_EDGES = ARTIFACT_ROOT / "edges.json"
DEFAULT_SUMMARY = ARTIFACT_ROOT / "summary.json"
DEFAULT_THRESHOLD = 0.75

SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "external_network_authorized": False,
    "llm_calls_authorized": False,
}


def load_corpus(path: Path = DEFAULT_CORPUS) -> list[dict[str, Any]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    figures = payload.get("figures") if isinstance(payload, dict) else payload
    if not isinstance(figures, list):
        raise ValueError("figure corpus must contain a figures[] list")
    return figures


def load_embeddings(path: Path = DEFAULT_EMBEDDINGS) -> dict[str, list[float]]:
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


def _caption_excerpt(figure: dict[str, Any], *, limit: int = 180) -> str:
    caption = (
        str(figure.get("caption") or figure.get("caption_text") or "").replace("\n", " ").strip()
    )
    if len(caption) <= limit:
        return caption
    return caption[: limit - 1].rstrip() + "…"


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
    if len(figure_ids) < 2:
        raise ValueError("at least two figures are required for similarity edges")
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
                    "paper_a": source["arxiv_id"],
                    "figure_a_id": source["figure_id"],
                    "figure_a_idx": source["figure_idx"],
                    "label_a": source.get("label", ""),
                    "caption_a_excerpt": _caption_excerpt(source),
                    "paper_b": target["arxiv_id"],
                    "figure_b_id": target["figure_id"],
                    "figure_b_idx": target["figure_idx"],
                    "label_b": target.get("label", ""),
                    "caption_b_excerpt": _caption_excerpt(target),
                    "similarity": round(score, 6),
                    "relation_type": relation_type,
                }
            )
    edges.sort(key=lambda edge: (-float(edge["similarity"]), edge["paper_a"], edge["paper_b"]))
    values = [float(edge["similarity"]) for edge in edges]
    inter_doc_edges = sum(1 for edge in edges if edge["relation_type"] == "inter-doc")
    intra_doc_edges = len(edges) - inter_doc_edges
    existing_summary: dict[str, Any] = {}
    if summary_path.exists():
        existing_payload = json.loads(summary_path.read_text(encoding="utf-8"))
        if isinstance(existing_payload, dict):
            existing_summary = existing_payload
    summary: dict[str, Any] = {
        **existing_summary,
        "schema_version": "m058.plotextractor.summary.v2",
        "safety_defaults": SAFETY_DEFAULTS,
        "threshold": threshold,
        "include_intra_doc": include_intra_doc,
        "sample_size": len({figure["arxiv_id"] for figure in figures}),
        "sample_arxiv_ids": sorted({figure["arxiv_id"] for figure in figures}),
        "total_figures": len(figures),
        "total_captions": sum(1 for figure in figures if figure.get("caption")),
        "total_pairs": len(figures) * (len(figures) - 1) // 2,
        "edges_total": len(edges),
        "inter_doc_edges": inter_doc_edges,
        "intra_doc_edges": intra_doc_edges,
        "similarity_stats": _similarity_stats(values),
    }
    edge_payload: dict[str, Any] = {
        "schema_version": "m058.plotextractor.edges.v1",
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
                "summary": str(args.summary),
                "edges_path": str(args.edges),
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
