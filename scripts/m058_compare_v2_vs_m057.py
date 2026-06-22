#!/usr/bin/env python3
"""Compare M058 plotextractor v2 figure links against M057 regex baseline."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
M058_ROOT = ROOT / "artifacts" / "m058-plotextractor"
DEFAULT_M058_SUMMARY = M058_ROOT / "summary.json"
DEFAULT_M058_EDGES = M058_ROOT / "edges.json"
DEFAULT_M058_CORPUS = M058_ROOT / "figure-caption-corpus.json"
DEFAULT_M057_SUMMARY = ROOT / "artifacts" / "m057-fd-marker" / "figure-links" / "summary.json"
DEFAULT_M057_EDGES = ROOT / "artifacts" / "m057-fd-marker" / "figure-links" / "edges.json"
DEFAULT_M057_CORPUS = (
    ROOT / "artifacts" / "m057-fd-marker" / "figure-links" / "figure-caption-corpus.json"
)
DEFAULT_OUTPUT_JSON = M058_ROOT / "v2-vs-m057.json"
DEFAULT_OUTPUT_MD = M058_ROOT / "v2-vs-m057.md"
DEFAULT_DECISION = M058_ROOT / "s01-decision.md"

SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "external_network_authorized": False,
    "llm_calls_authorized": False,
}


def _load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _mean_caption_length(figures: list[dict[str, Any]], caption_key: str) -> float:
    captions = [str(figure.get(caption_key) or figure.get("caption") or "") for figure in figures]
    captions = [caption for caption in captions if caption.strip()]
    if not captions:
        return 0.0
    return round(sum(len(caption) for caption in captions) / len(captions), 3)


def _edge_pairs(edge_payload: dict[str, Any], *, version: str) -> set[tuple[str, str]]:
    pairs: set[tuple[str, str]] = set()
    for edge in edge_payload.get("edges", []):
        if version == "m058":
            a = str(edge.get("figure_a_id"))
            b = str(edge.get("figure_b_id"))
        else:
            a = str(edge.get("source_figure_id"))
            b = str(edge.get("target_figure_id"))
        pairs.add(tuple(sorted((a, b))))
    return pairs


def _winner(v2: float, v1: float, *, higher_is_better: bool = True) -> str:
    if v2 == v1:
        return "tie"
    if higher_is_better:
        return "v2" if v2 > v1 else "m057"
    return "v2" if v2 < v1 else "m057"


def compare_v2_vs_m057(
    *,
    m058_summary_path: Path = DEFAULT_M058_SUMMARY,
    m058_edges_path: Path = DEFAULT_M058_EDGES,
    m058_corpus_path: Path = DEFAULT_M058_CORPUS,
    m057_summary_path: Path = DEFAULT_M057_SUMMARY,
    m057_edges_path: Path = DEFAULT_M057_EDGES,
    m057_corpus_path: Path = DEFAULT_M057_CORPUS,
    output_json_path: Path = DEFAULT_OUTPUT_JSON,
    output_md_path: Path = DEFAULT_OUTPUT_MD,
    decision_path: Path = DEFAULT_DECISION,
) -> dict[str, Any]:
    m058_summary = _load_json(m058_summary_path)
    m058_edges = _load_json(m058_edges_path)
    m058_corpus = _load_json(m058_corpus_path)
    m057_summary = _load_json(m057_summary_path)
    m057_edges = _load_json(m057_edges_path)
    m057_corpus = _load_json(m057_corpus_path)

    m058_figures = m058_corpus.get("figures", [])
    m057_figures = m057_corpus.get("figures", [])
    m058_ids = {figure.get("arxiv_id") for figure in m058_figures}
    m057_ids = {figure.get("arxiv_id") for figure in m057_figures}
    overlap_ids = sorted(str(item) for item in (m058_ids & m057_ids) if item)
    m058_pairs = _edge_pairs(m058_edges, version="m058")
    m057_pairs = _edge_pairs(m057_edges, version="m057")
    overlap_pairs = sorted(m058_pairs & m057_pairs)

    metrics = {
        "pdfs_processed": {
            "v2": int(m058_summary.get("sample_size", 0)),
            "m057": len(m057_ids),
            "winner": "m057",
            "note": "M057 covers the full prior corpus; M058 is a required 5-PDF pilot.",
        },
        "pilot_caption_coverage": {
            "v2": round(
                float(m058_summary.get("total_captions", 0))
                / max(float(m058_summary.get("total_figures", 0)), 1.0),
                6,
            ),
            "m057": round(
                sum(1 for figure in m057_figures if figure.get("caption"))
                / max(len(m057_figures), 1),
                6,
            ),
        },
        "caption_richness_mean_chars": {
            "v2": _mean_caption_length(m058_figures, "caption"),
            "m057": _mean_caption_length(m057_figures, "caption"),
        },
        "edges_total": {
            "v2": int(m058_summary.get("edges_total", 0)),
            "m057": int(m057_summary.get("edges_total", 0)),
        },
        "inter_doc_edges": {
            "v2": int(m058_summary.get("inter_doc_edges", 0)),
            "m057": int(m057_summary.get("inter_doc_edges", 0)),
        },
        "similarity_mean": {
            "v2": float((m058_summary.get("similarity_stats") or {}).get("mean") or 0.0),
            "m057": float((m057_summary.get("similarity_stats") or {}).get("mean") or 0.0),
        },
        "label_availability": {
            "v2": round(
                sum(1 for figure in m058_figures if figure.get("label"))
                / max(len(m058_figures), 1),
                6,
            ),
            "m057": 0.0,
        },
        "image_path_availability": {
            "v2": round(
                sum(1 for figure in m058_figures if figure.get("image_path"))
                / max(len(m058_figures), 1),
                6,
            ),
            "m057": 0.0,
        },
    }
    for _name, item in metrics.items():
        if "winner" not in item:
            item["winner"] = _winner(float(item["v2"]), float(item["m057"]))

    go = (
        int(m058_summary.get("sample_size", 0)) == 5
        and int(m058_summary.get("total_figures", 0)) >= 5
        and int(m058_summary.get("total_captions", 0)) >= 5
        and metrics["label_availability"]["v2"] > 0
        and metrics["image_path_availability"]["v2"] > 0
    )
    decision = "go" if go else "no-go"
    rationale = (
        "S02 is authorized to proceed because plotextractor v2 produced TeX-derived captions, labels, and image paths for the 5-PDF pilot."
        if go
        else "S02 is disabled until plotextractor v2 produces enough TeX-derived captions, labels, and image paths for the 5-PDF pilot."
    )
    payload: dict[str, Any] = {
        "schema_version": "m058.plotextractor.v2-vs-m057.v1",
        "safety_defaults": SAFETY_DEFAULTS,
        "overlap_arxiv_ids": overlap_ids,
        "overlap_edge_pairs": overlap_pairs,
        "metrics": metrics,
        "decision_for_s02": decision,
        "decision_rationale": rationale,
    }
    output_json_path.parent.mkdir(parents=True, exist_ok=True)
    output_json_path.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    md = render_markdown(payload)
    output_md_path.write_text(md, encoding="utf-8")
    decision_path.write_text(render_decision_markdown(payload), encoding="utf-8")
    return payload


def render_markdown(payload: dict[str, Any]) -> str:
    lines = [
        "# M058 S01: plotextractor v2 vs M057 figure-caption baseline",
        "",
        "Safety defaults: graph writes, production import, fact promotion, external network promotion, and LLM calls are disabled by default.",
        "",
        "## Metric comparison",
        "",
        "| Metric | v2 | M057 | Better |",
        "|---|---:|---:|---|",
    ]
    for name, metric in payload["metrics"].items():
        lines.append(f"| {name} | {metric['v2']} | {metric['m057']} | {metric['winner']} |")
    lines.extend(
        [
            "",
            "## Overlap",
            "",
            f"- Overlap arXiv IDs: {', '.join(payload['overlap_arxiv_ids']) or 'none'}",
            f"- Overlap edge pairs: {len(payload['overlap_edge_pairs'])}",
            "",
            "## Decision",
            "",
            f"- Decision for S02: **{payload['decision_for_s02']}**",
            f"- Rationale: {payload['decision_rationale']}",
            "",
        ]
    )
    return "\n".join(lines)


def render_decision_markdown(payload: dict[str, Any]) -> str:
    return "\n".join(
        [
            "# M058 S01 Decision: S02 Marker stage 1",
            "",
            f"Decision: **{payload['decision_for_s02']}**",
            "",
            payload["decision_rationale"],
            "",
            "## Evidence",
            "",
            f"- v2 figures: {payload['metrics']['edges_total']['v2']} similarity edges over the pilot corpus.",
            f"- Label availability: {payload['metrics']['label_availability']['v2']}.",
            f"- Image path availability: {payload['metrics']['image_path_availability']['v2']}.",
            "- Graph writes, production import, fact promotion, external network promotion, and LLM calls are disabled by default.",
            "",
        ]
    )


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--m058-summary", type=Path, default=DEFAULT_M058_SUMMARY)
    parser.add_argument("--m058-edges", type=Path, default=DEFAULT_M058_EDGES)
    parser.add_argument("--m058-corpus", type=Path, default=DEFAULT_M058_CORPUS)
    parser.add_argument("--m057-summary", type=Path, default=DEFAULT_M057_SUMMARY)
    parser.add_argument("--m057-edges", type=Path, default=DEFAULT_M057_EDGES)
    parser.add_argument("--m057-corpus", type=Path, default=DEFAULT_M057_CORPUS)
    parser.add_argument("--output-json", type=Path, default=DEFAULT_OUTPUT_JSON)
    parser.add_argument("--output-md", type=Path, default=DEFAULT_OUTPUT_MD)
    parser.add_argument("--decision", type=Path, default=DEFAULT_DECISION)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    payload = compare_v2_vs_m057(
        m058_summary_path=args.m058_summary,
        m058_edges_path=args.m058_edges,
        m058_corpus_path=args.m058_corpus,
        m057_summary_path=args.m057_summary,
        m057_edges_path=args.m057_edges,
        m057_corpus_path=args.m057_corpus,
        output_json_path=args.output_json,
        output_md_path=args.output_md,
        decision_path=args.decision,
    )
    print(
        json.dumps(
            {"decision_for_s02": payload["decision_for_s02"], "output": str(args.output_json)},
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
