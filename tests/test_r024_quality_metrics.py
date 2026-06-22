"""Tests for R024 S03: quality metrics + comparison report (5 vs 10 articles)."""

from __future__ import annotations

import json
from pathlib import Path
from typing import cast

import pytest

REPO_ROOT = Path("/root/daily-archive")
R024_DIR = REPO_ROOT / "data" / "r024-10-document-corpus-v1"
METRICS = R024_DIR / "quality-metrics.json"
COMPARISON = R024_DIR / "quality-comparison-5-vs-10.md"


def _load(path: Path) -> dict[str, object]:
    data = json.loads(path.read_text())
    return dict(data) if isinstance(data, dict) else {}


def test_metrics_exists() -> None:
    assert METRICS.exists(), "quality-metrics.json missing"


def test_comparison_exists() -> None:
    assert COMPARISON.exists(), "quality-comparison-5-vs-10.md missing"


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


def test_metrics_baseline_5_articles() -> None:
    m = _load(METRICS)
    assert m.get("corpus_size_baseline") == 5


def test_metrics_r024_10_articles() -> None:
    m = _load(METRICS)
    assert m.get("corpus_size_r024") == 10


def test_metrics_chunk_counts_positive() -> None:
    m = _load(METRICS)
    assert int(str(m.get("baseline_total_chunks", 0))) > 0
    assert int(str(m.get("r024_total_chunks", 0))) > 0


def test_metrics_r024_per_article() -> None:
    m = _load(METRICS)
    r024 = m.get("r024")
    assert isinstance(r024, dict)
    assert len(r024) == 10
    for ref, data in r024.items():
        d: dict[str, object] = cast(dict[str, object], data)
        assert int(str(d.get("chunk_count", 0))) > 0, f"{ref} chunks=0"
        assert d.get("graph_import_allowed") is False
        assert d.get("ladybugdb_written") is False


def test_comparison_md_has_fail_closed() -> None:
    text = COMPARISON.read_text()
    assert "Fail-Closed Invariants" in text
    assert "network_fetch_attempted" in text
    assert "false" in text.lower()


def test_comparison_md_has_per_article_table() -> None:
    text = COMPARISON.read_text()
    assert "Per-Article Chunk Counts" in text
    assert "(M025)" in text
    assert "(R024)" in text


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
