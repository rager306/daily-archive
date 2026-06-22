"""Tests for R024 S05: 53-document final coverage report + REQUIREMENTS update."""

from __future__ import annotations

from pathlib import Path

import pytest

REPO_ROOT = Path("/root/daily-archive")
COVERAGE = REPO_ROOT / "data" / "r024-53-document-corpus-v1" / "R024-COVERAGE.md"
REQUIREMENTS = REPO_ROOT / ".gsd" / "REQUIREMENTS.md"


def test_coverage_report_exists() -> None:
    assert COVERAGE.exists()


def test_coverage_has_three_stage_comparison() -> None:
    text = COVERAGE.read_text()
    assert "10-document" in text
    assert "20-document" in text
    assert "53-document" in text
    assert "M116" in text
    assert "M117" in text
    assert "M118" in text


def test_coverage_has_executive_summary() -> None:
    text = COVERAGE.read_text()
    assert "Executive Summary" in text


def test_coverage_has_corpus_selection_section() -> None:
    text = COVERAGE.read_text()
    assert "Corpus Selection" in text
    assert "Baseline (M117)" in text


def test_coverage_has_pdf_conversion_documented() -> None:
    text = COVERAGE.read_text()
    assert "PDF" in text
    assert "pymupdf" in text


def test_coverage_has_parser_chunking_section() -> None:
    text = COVERAGE.read_text()
    assert "Parser" in text and "Chunking" in text
    assert "53/53" in text


def test_coverage_has_quality_metrics_section() -> None:
    text = COVERAGE.read_text()
    assert "Quality Metrics" in text


def test_coverage_has_networkx_probe_section() -> None:
    text = COVERAGE.read_text()
    assert "NetworkX" in text
    assert "431" in text
    assert "629" in text


def test_coverage_has_memory_profile() -> None:
    text = COVERAGE.read_text()
    assert "Memory" in text
    assert "8.27 MB" in text or "8.27" in text
    assert "tracemalloc" in text


def test_coverage_has_production_recommendations() -> None:
    text = COVERAGE.read_text()
    assert "Production" in text
    assert "gate" in text.lower()


def test_coverage_has_fail_closed_invariants() -> None:
    text = COVERAGE.read_text()
    assert "Fail-closed invariants" in text
    assert "graph_import_allowed" in text
    assert "false" in text
    assert "ladybugdb" in text.lower() and "false" in text.lower()


def test_coverage_has_three_milestone_combined_stats() -> None:
    text = COVERAGE.read_text()
    assert "Three-Milestone" in text or "M116 + M117 + M118" in text


def test_requirements_r024_has_m118_update() -> None:
    text = REQUIREMENTS.read_text()
    idx = text.find("### R024 ")
    assert idx >= 0
    section = text[idx : idx + 6000]
    assert "M118" in section
    assert "53-document" in section or "53-doc" in section


def test_requirements_r024_still_active() -> None:
    text = REQUIREMENTS.read_text()
    idx = text.find("### R024 ")
    assert idx >= 0
    section = text[idx : idx + 1500]
    assert "Status: active" in section


def test_requirements_r024_mentions_networkx_and_memory() -> None:
    text = REQUIREMENTS.read_text()
    idx = text.find("### R024 ")
    assert idx >= 0
    section = text[idx : idx + 6000]
    assert "NetworkX" in section
    assert "8.27" in section or "memory" in section.lower()


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
