"""Tests for R024 S05: coverage report + REQUIREMENTS update."""

from __future__ import annotations

from pathlib import Path
from typing import cast

import pytest

REPO_ROOT = Path("/root/daily-archive")
COVERAGE = REPO_ROOT / "data" / "r024-10-document-corpus-v1" / "R024-COVERAGE.md"
REQUIREMENTS = REPO_ROOT / ".gsd" / "REQUIREMENTS.md"


def test_coverage_report_exists() -> None:
    assert COVERAGE.exists(), "R024-COVERAGE.md missing"


def test_coverage_has_executive_summary() -> None:
    text = COVERAGE.read_text()
    assert "Executive Summary" in text
    assert "10-document" in text


def test_coverage_has_corpus_selection_section() -> None:
    text = COVERAGE.read_text()
    assert "Corpus Selection" in text
    assert "Baseline (M025)" in text
    assert "Extension" in text


def test_coverage_has_parser_chunking_section() -> None:
    text = COVERAGE.read_text()
    assert "Parser + Chunking" in text
    assert "10/10" in text


def test_coverage_has_quality_metrics_section() -> None:
    text = COVERAGE.read_text()
    assert "Quality Metrics" in text
    assert "25" in text
    assert "20" in text


def test_coverage_has_networkx_probe_section() -> None:
    text = COVERAGE.read_text()
    assert "NetworkX" in text
    assert "31" in text
    assert "30" in text


def test_coverage_has_fail_closed_invariants() -> None:
    text = COVERAGE.read_text()
    assert "Fail-closed invariants" in text
    assert "graph_import_allowed=false" in text
    assert "ladybugdb_written=false" in text
    assert "falkordb_written=false" in text


def test_coverage_has_recommendations() -> None:
    text = COVERAGE.read_text()
    assert "Recommendations" in text
    assert "20-doc" in text
    assert "one-week" in text


def test_coverage_has_risk_notes() -> None:
    text = COVERAGE.read_text()
    assert "Risk" in text or "Blocker" in text
    assert "no regressions" in text.lower() or "preserved" in text.lower()


def test_requirements_r024_has_m116_update() -> None:
    text = REQUIREMENTS.read_text()
    # find R024 section
    idx = text.find("### R024 ")
    assert idx >= 0, "R024 section missing"
    section = text[idx : idx + 4000]
    assert "M116" in section, "M116 reference missing in R024"
    assert "10-document" in section or "10-document corpus" in section


def test_requirements_r024_still_active() -> None:
    text = REQUIREMENTS.read_text()
    idx = text.find("### R024 ")
    assert idx >= 0, "R024 section missing"
    section = text[idx : idx + 1500]
    assert "Status: active" in section


def test_requirements_r024_mentions_networkx() -> None:
    text = REQUIREMENTS.read_text()
    idx = text.find("### R024 ")
    assert idx >= 0, "R024 section missing"
    section = text[idx : idx + 5000]
    assert "NetworkX" in section, "NetworkX reference missing in R024"


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
    _ = cast  # keep import for tooling
