"""Tests for R024 S05: 20-document coverage report + REQUIREMENTS update."""

from __future__ import annotations

from pathlib import Path
from typing import cast

import pytest

REPO_ROOT = Path("/root/daily-archive")
COVERAGE = REPO_ROOT / "data" / "r024-20-document-corpus-v1" / "R024-COVERAGE.md"
REQUIREMENTS = REPO_ROOT / ".gsd" / "REQUIREMENTS.md"


def test_coverage_report_exists() -> None:
    assert COVERAGE.exists()


def test_coverage_has_executive_summary() -> None:
    text = COVERAGE.read_text()
    assert "Executive Summary" in text
    assert "20-document" in text


def test_coverage_has_corpus_selection_section() -> None:
    text = COVERAGE.read_text()
    assert "Corpus Selection" in text
    assert "Baseline (M116)" in text
    assert "Extension" in text


def test_coverage_has_parser_chunking_section() -> None:
    text = COVERAGE.read_text()
    assert "Parser + Chunking" in text
    assert "20/20" in text


def test_coverage_has_quality_metrics_section() -> None:
    text = COVERAGE.read_text()
    assert "Quality Metrics" in text


def test_coverage_has_networkx_probe_section() -> None:
    text = COVERAGE.read_text()
    assert "NetworkX" in text or "Networkx" in text
    assert "161" in text
    assert "188" in text


def test_coverage_has_extended_features() -> None:
    """20-doc has entities + relations not in M116."""
    text = COVERAGE.read_text()
    assert "entity" in text.lower()
    assert "article_cites_article" in text
    assert "coarse_topic_code" in text


def test_coverage_has_fail_closed_invariants() -> None:
    text = COVERAGE.read_text()
    assert "Fail-closed invariants" in text
    assert "graph_import_allowed=false" in text
    assert "ladybugdb_written=false" in text


def test_coverage_has_recommendations() -> None:
    text = COVERAGE.read_text()
    assert "Recommendations" in text
    assert "one-week" in text


def test_coverage_has_risk_notes() -> None:
    text = COVERAGE.read_text()
    assert "Risk" in text or "Blocker" in text


def test_coverage_has_combined_stats() -> None:
    text = COVERAGE.read_text()
    assert "Combined Stats" in text or "M116+M117" in text


def test_requirements_r024_has_m117_update() -> None:
    text = REQUIREMENTS.read_text()
    idx = text.find("### R024 ")
    assert idx >= 0
    section = text[idx : idx + 5000]
    assert "M117" in section
    assert "20-document" in section or "20-document corpus" in section


def test_requirements_r024_still_active() -> None:
    text = REQUIREMENTS.read_text()
    idx = text.find("### R024 ")
    assert idx >= 0
    section = text[idx : idx + 1500]
    assert "Status: active" in section


def test_requirements_r024_mentions_networkx() -> None:
    text = REQUIREMENTS.read_text()
    idx = text.find("### R024 ")
    assert idx >= 0
    section = text[idx : idx + 5000]
    assert "NetworkX" in section


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
    _ = cast  # keep for tooling
