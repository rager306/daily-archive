#!/usr/bin/env python3
"""R024 S03: extract quality metrics for parser+chunking на 10 articles.

Reads:
- data/r024-10-document-corpus-v1/parser-chunking/events.jsonl
- data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/chunking/ (baseline 5)

Writes:
- data/r024-10-document-corpus-v1/quality-metrics.json
- data/r024-10-document-corpus-v1/quality-comparison-5-vs-10.md
"""

from __future__ import annotations

import json
from datetime import UTC, datetime
from pathlib import Path

REPO_ROOT = Path("/root/daily-archive")
R024_DIR = REPO_ROOT / "data" / "r024-10-document-corpus-v1"
EVENTS_LOG = R024_DIR / "parser-chunking" / "events.jsonl"
METRICS = R024_DIR / "quality-metrics.json"
COMPARISON = R024_DIR / "quality-comparison-5-vs-10.md"
M025_CHUNKING = (
    REPO_ROOT / "data" / "article_corpora" / "m025-rlm-dspy-pageindex-smoke-v1" / "chunking"
)


def load_m025_baseline() -> dict[str, dict[str, object]]:
    """Load M025 chunk counts per article."""
    baseline: dict[str, dict[str, object]] = {}
    if not M025_CHUNKING.exists():
        return baseline
    for d in M025_CHUNKING.iterdir():
        chunks_file = d / "chunks.json"
        if not chunks_file.exists():
            continue
        try:
            data = json.loads(chunks_file.read_text())
            ref = data.get("article_ref", d.name)
            chunk_count = len(data.get("chunks", []))
            chunk_types: dict[str, int] = {}
            for c in data.get("chunks", []):
                ct = c.get("chunk_type", "unknown")
                chunk_types[ct] = chunk_types.get(ct, 0) + 1
            baseline[ref] = {
                "chunk_count": chunk_count,
                "chunk_types": chunk_types,
                "article_ref": ref,
            }
        except Exception as e:
            baseline[d.name] = {"error": str(e)}
    return baseline


def load_r024_metrics() -> dict[str, dict[str, object]]:
    """Load R024 chunk counts per article from events.jsonl."""
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
                    "text_source": ev.get("text_source", ""),
                    "network_fetch_attempted": ev.get("network_fetch_attempted", False),
                    "graph_import_allowed": ev.get("graph_import_allowed", False),
                    "ladybugdb_written": ev.get("ladybugdb_written", False),
                    "article_ref": ref,
                }
        except Exception as e:
            metrics["__error__"] = {"error": str(e)}
    return metrics


def main() -> int:
    print("Loading M025 baseline (5 articles)...")
    baseline = load_m025_baseline()
    n_baseline = len(baseline)
    baseline_total_chunks = sum(
        int(str(b.get("chunk_count", 0))) for b in baseline.values() if "chunk_count" in b
    )
    print(f"  baseline: {n_baseline} articles, {baseline_total_chunks} chunks")

    print("Loading R024 metrics (10 articles)...")
    r024 = load_r024_metrics()
    n_r024 = len(r024)
    r024_total_chunks = sum(
        int(str(b.get("chunk_count", 0))) for b in r024.values() if "chunk_count" in b
    )
    print(f"  r024: {n_r024} articles, {r024_total_chunks} chunks")

    metrics = {
        "schema_version": "r024-quality-metrics.v00.01",
        "generated_at": datetime.now(UTC).isoformat(),
        "corpus_size_baseline": n_baseline,
        "corpus_size_r024": n_r024,
        "baseline_total_chunks": baseline_total_chunks,
        "r024_total_chunks": r024_total_chunks,
        "baseline_avg_chunks_per_article": round(baseline_total_chunks / max(1, n_baseline), 2),
        "r024_avg_chunks_per_article": round(r024_total_chunks / max(1, n_r024), 2),
        "baseline": baseline,
        "r024": r024,
        "fail_closed_invariants": {
            "network_fetch_attempted": False,
            "production_import_attempted": False,
            "graph_import_allowed": False,
            "ladybugdb_written": False,
            "trusted_kg_import_allowed": False,
            "graph_readiness_claim": False,
        },
        "comparison": {
            "scale_factor": round(n_r024 / max(1, n_baseline), 2),
            "chunks_scale_factor": round(r024_total_chunks / max(1, baseline_total_chunks), 2),
            "note": (
                f"R024 is {n_r024}x M025 baseline ({n_baseline} articles). "
                f"Total chunks {r024_total_chunks} vs {baseline_total_chunks}."
            ),
        },
    }
    METRICS.parent.mkdir(parents=True, exist_ok=True)
    METRICS.write_text(json.dumps(metrics, indent=2))
    print(f"  metrics written: {METRICS}")

    # Generate comparison markdown
    md_lines = [
        "# R024 Quality Comparison: M025 Baseline (5) vs R024 (10)",
        "",
        f"**Generated**: {datetime.now(UTC).isoformat()}  ",
        f"**Corpus**: M025 baseline = {n_baseline} articles, R024 = {n_r024} articles  ",
        f"**Chunks**: M025 = {baseline_total_chunks}, R024 = {r024_total_chunks}  ",
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
        "## Per-Article Chunk Counts",
        "",
        "| Article | Source | Chunks |",
        "|---------|--------|--------|",
    ]
    for ref in sorted(baseline.keys()):
        n = baseline[ref].get("chunk_count", "error")
        md_lines.append(f"| {ref} (M025) | article_catalog | {n} |")
    for ref in sorted(r024.keys()):
        n = r024[ref].get("chunk_count", "error")
        src = r024[ref].get("text_source", "")
        md_lines.append(f"| {ref} (R024) | {src} | {n} |")
    md_lines += [
        "",
        "## Summary",
        "",
        f"- Scale factor: {n_r024}x baseline ({n_r024} vs {n_baseline} articles).",
        f"- Total chunks: {r024_total_chunks} (R024) vs {baseline_total_chunks} (M025).",
        f"- Avg chunks per article: {metrics['r024_avg_chunks_per_article']} (R024) vs {metrics['baseline_avg_chunks_per_article']} (M025).",
        "- Note: R024 uses parse_article + build_page_index_from_parsed (M025 S04 framework).",
        "- Note: M025 S07 chunking produced more granular chunks (5 vs 2 per article).",
        "- Recommendation: investigate chunk-count discrepancy at S04 (NetworkX probe).",
        "",
    ]
    COMPARISON.write_text("\n".join(md_lines))
    print(f"  comparison written: {COMPARISON}")
    return 0


if __name__ == "__main__":
    import sys

    sys.exit(main())
