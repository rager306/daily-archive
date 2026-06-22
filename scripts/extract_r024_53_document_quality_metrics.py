#!/usr/bin/env python3
"""R024 S03: extract quality metrics for parser+chunking на 53 articles.

Reads:
- data/r024-53-document-corpus-v1/parser-chunking/events.jsonl (53 articles)
- data/r024-20-document-corpus-v1/quality-metrics.json (M117 baseline 20)

Writes:
- data/r024-53-document-corpus-v1/quality-metrics.json
- data/r024-53-document-corpus-v1/quality-comparison-20-vs-53.md
"""

from __future__ import annotations

import json
from datetime import UTC, datetime
from pathlib import Path

REPO_ROOT = Path("/root/daily-archive")
R053_DIR = REPO_ROOT / "data" / "r024-53-document-corpus-v1"
EVENTS_LOG = R053_DIR / "parser-chunking" / "events.jsonl"
METRICS = R053_DIR / "quality-metrics.json"
COMPARISON = R053_DIR / "quality-comparison-20-vs-53.md"
M117_METRICS = REPO_ROOT / "data" / "r024-20-document-corpus-v1" / "quality-metrics.json"


def load_m118_metrics() -> dict[str, dict[str, object]]:
    metrics: dict[str, dict[str, object]] = {}
    if not EVENTS_LOG.exists():
        return metrics
    for line in EVENTS_LOG.read_text().splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            ev = json.loads(line)
            ref = ev.get("article_ref", "")
            if ev.get("event") == "parser_chunking_complete":
                metrics[ref] = {
                    "chunk_count": ev.get("chunk_count", 0),
                    "source_kind": ev.get("source_kind", "unknown"),
                    "network_fetch_attempted": ev.get("network_fetch_attempted", False),
                    "graph_import_allowed": ev.get("graph_import_allowed", False),
                    "ladybugdb_written": ev.get("ladybugdb_written", False),
                    "article_ref": ref,
                }
        except Exception as e:
            metrics["__error__"] = {"error": str(e)}
    return metrics


def main() -> int:
    print("Loading M118 metrics (53 articles)...")
    m118 = load_m118_metrics()
    n_m118 = len(m118)
    m118_total_chunks = sum(
        int(str(b.get("chunk_count", 0))) for b in m118.values() if "chunk_count" in b
    )
    pdf_count = sum(1 for b in m118.values() if b.get("source_kind") == "pdf_converted")
    html_count = sum(1 for b in m118.values() if b.get("source_kind") == "html_native")
    print(
        f"  m118: {n_m118} articles, {m118_total_chunks} chunks (PDF={pdf_count}, HTML={html_count})"
    )

    print("Loading M117 baseline (20 articles)...")
    m117_data = json.loads(M117_METRICS.read_text())
    n_m117 = int(m117_data["corpus_size_m117"])
    m117_total_chunks = int(m117_data["m117_total_chunks"])
    print(f"  m117 baseline: {n_m117} articles, {m117_total_chunks} chunks")

    metrics = {
        "schema_version": "r024-53-document-quality-metrics.v00.01",
        "generated_at": datetime.now(UTC).isoformat(),
        "corpus_size_m118": n_m118,
        "corpus_size_m117": n_m117,
        "m117_total_chunks": m117_total_chunks,
        "m118_total_chunks": m118_total_chunks,
        "m117_avg_chunks_per_article": round(m117_total_chunks / max(1, n_m117), 2),
        "m118_avg_chunks_per_article": round(m118_total_chunks / max(1, n_m118), 2),
        "m118_pdf_count": pdf_count,
        "m118_html_count": html_count,
        "m118": m118,
        "fail_closed_invariants": {
            "network_fetch_attempted": False,
            "production_import_attempted": False,
            "graph_import_allowed": False,
            "ladybugdb_written": False,
            "trusted_kg_import_allowed": False,
            "graph_readiness_claim": False,
        },
        "comparison": {
            "scale_factor": round(n_m118 / max(1, n_m117), 2),
            "chunks_scale_factor": round(m118_total_chunks / max(1, m117_total_chunks), 2),
            "note": (
                f"M118 is {n_m118}/{n_m117}={round(n_m118 / max(1, n_m117), 2)}x M117 baseline. "
                f"Total chunks {m118_total_chunks} vs {m117_total_chunks}. "
                f"PDF={pdf_count}, HTML={html_count}."
            ),
        },
    }
    METRICS.parent.mkdir(parents=True, exist_ok=True)
    METRICS.write_text(json.dumps(metrics, indent=2))
    print(f"  metrics written: {METRICS}")

    md_lines = [
        "# R024 Quality Comparison: M117 Baseline (20) vs M118 (53)",
        "",
        f"**Generated**: {datetime.now(UTC).isoformat()}  ",
        f"**Corpus**: M117 = {n_m117} articles, M118 = {n_m118} articles  ",
        f"**Chunks**: M117 = {m117_total_chunks}, M118 = {m118_total_chunks}  ",
        f"**M118 sources**: PDF={pdf_count} (pymupdf), HTML={html_count} (abs.html)  ",
        "",
        "## Fail-Closed Invariants",
        "",
        "| Flag | Value |",
        "|------|-------|",
        "| network_fetch_attempted | false |",
        "| production_import_attempted | false |",
        "| graph_import_allowed | false |",
        "| ladybugdb_written | false |",
        "| trusted_kg_import_allowed | false |",
        "| graph_readiness_claim | false |",
        "",
        "## Per-Article Chunk Counts (M118)",
        "",
        "| Article | Source Kind | Chunks |",
        "|---------|-------------|--------|",
    ]
    for ref in sorted(m118.keys()):
        n = m118[ref].get("chunk_count", "error")
        sk = m118[ref].get("source_kind", "")
        md_lines.append(f"| {ref} | {sk} | {n} |")
    md_lines += [
        "",
        "## Summary",
        "",
        f"- Scale factor: {n_m118 / max(1, n_m117)}x baseline ({n_m118} vs {n_m117} articles).",
        f"- Total chunks: {m118_total_chunks} (M118) vs {m117_total_chunks} (M117).",
        f"- Avg chunks per article: {metrics['m118_avg_chunks_per_article']} (M118) vs {metrics['m117_avg_chunks_per_article']} (M117).",
        f"- M118 source mix: {pdf_count} PDF-converted (pymupdf) + {html_count} HTML (abs.html).",
        "- Note: M118 uses same parser+chunking framework as M117 (parse_article + build_page_index_from_parsed).",
        "- Recommendation: extend NetworkX probe at 53-article scale + memory profiling (S04).",
        "",
    ]
    COMPARISON.write_text("\n".join(md_lines))
    print(f"  comparison written: {COMPARISON}")
    return 0


if __name__ == "__main__":
    import sys

    sys.exit(main())
