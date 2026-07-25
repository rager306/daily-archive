"""Tests for R024 S03: 20-document quality metrics + comparison (10 vs 20)."""

from __future__ import annotations

import json
from pathlib import Path
from typing import cast

import pytest

REPO_ROOT = Path("/root/daily-archive")
R020_DIR = REPO_ROOT / "data" / "r024-20-document-corpus-v1"
METRICS = R020_DIR / "quality-metrics.json"
COMPARISON = R020_DIR / "quality-comparison-10-vs-20.md"


def _load(path: Path) -> dict[str, object]:
    data = json.loads(path.read_text())
    return dict(data) if isinstance(data, dict) else {}


def test_metrics_exists() -> None:
    assert METRICS.exists()


def test_comparison_exists() -> None:
    assert COMPARISON.exists()


def test_metrics_fail_closed_invariants() -> None:
    m = _load(METRICS)
    fc = m.get("fail_closed_invariants")
    assert isinstance(fc, dict)
    for key in (
        "network_fetch_attempted",
        "production_import_attempted",
        "graph_import_allowed",
        "ladybugdb_written",
        "trusted_kg_import_allowed",
        "graph_readiness_claim",
    ):
        assert fc.get(key) is False, f"{key} must be False"


def test_metrics_m117_20_articles() -> None:
    m = _load(METRICS)
    assert int(str(m.get("corpus_size_m117", 0))) == 20


def test_metrics_chunk_counts_positive() -> None:
    m = _load(METRICS)
    assert int(str(m.get("m117_total_chunks", 0))) > 0
    assert int(str(m.get("m116_total_chunks", 0))) > 0


def test_metrics_m117_per_article() -> None:
    m = _load(METRICS)
    m117 = m.get("m117")
    assert isinstance(m117, dict)
    assert len(m117) == 20
    for ref, data in m117.items():
        d = cast(dict[str, object], data) if isinstance(data, dict) else {}
        assert int(str(d.get("chunk_count", 0))) > 0, f"{ref} chunks=0"
        assert d.get("graph_import_allowed") is False
        assert d.get("ladybugdb_written") is False


def test_comparison_md_has_fail_closed() -> None:
    text = COMPARISON.read_text()
    assert "Fail-Closed Invariants" in text
    assert "network_fetch_attempted" in text


def test_comparison_md_has_per_article_table() -> None:
    text = COMPARISON.read_text()
    assert "Per-Article Chunk Counts" in text
    assert "M117" in text or "M116" in text


def test_comparison_md_has_summary() -> None:
    text = COMPARISON.read_text()
    assert "Summary" in text
    assert "Scale factor" in text or "scale_factor" in text


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
