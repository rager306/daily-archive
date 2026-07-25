"""Tests for R024 M119 S05: entity-scale coverage report + REQUIREMENTS update."""

from __future__ import annotations

from pathlib import Path

import pytest

REPO_ROOT = Path("/root/daily-archive")
COVERAGE = REPO_ROOT / "data" / "r024-entity-scale-corpus-v1" / "R024-COVERAGE.md"
REQUIREMENTS = REPO_ROOT / ".gsd" / "REQUIREMENTS.md"


def test_coverage_report_exists() -> None:
    assert COVERAGE.exists()


def test_coverage_has_four_stage_comparison() -> None:
    text = COVERAGE.read_text()
    assert "M116" in text
    assert "M117" in text
    assert "M118" in text
    assert "M119" in text


def test_coverage_has_pivot_rationale() -> None:
    text = COVERAGE.read_text()
    assert "Pivot" in text or "pivot" in text
    assert "catalog exhausted" in text.lower()


def test_coverage_has_entity_scale_schema() -> None:
    text = COVERAGE.read_text()
    assert "Entity-Scale Schema" in text
    assert "10 entity types" in text or "10 types" in text


def test_coverage_has_extraction_section() -> None:
    text = COVERAGE.read_text()
    assert "Entity Extraction" in text
    assert "530" in text


def test_coverage_has_quality_metrics_section() -> None:
    text = COVERAGE.read_text()
    assert "Quality Metrics" in text
    assert "265" in text


def test_coverage_has_networkx_section() -> None:
    text = COVERAGE.read_text()
    assert "NetworkX" in text
    assert "699" in text
    assert "1427" in text


def test_coverage_has_memory_profile() -> None:
    text = COVERAGE.read_text()
    assert "Memory" in text
    assert "8.58" in text
    assert "tracemalloc" in text


def test_coverage_has_pivot_decision_tree() -> None:
    text = COVERAGE.read_text()
    assert "Decision Tree" in text or "decision tree" in text


def test_coverage_has_fail_closed_invariants() -> None:
    text = COVERAGE.read_text()
    assert "Fail-closed invariants" in text
    assert "graph_import_allowed" in text
    assert "ladybugdb" in text.lower()


def test_coverage_has_four_milestone_combined() -> None:
    text = COVERAGE.read_text()
    assert "Four-Milestone" in text or "M116 + M117 + M118 + M119" in text


def test_requirements_r024_has_m119_update() -> None:
    text = REQUIREMENTS.read_text()
    idx = text.find("### R024 ")
    assert idx >= 0
    section = text[idx : idx + 7000]
    assert "M119" in section
    assert "entity-scale" in section or "530 entities" in section


def test_requirements_r024_still_active() -> None:
    text = REQUIREMENTS.read_text()
    idx = text.find("### R024 ")
    assert idx >= 0
    section = text[idx : idx + 1500]
    assert "Status: active" in section


def test_requirements_r024_mentions_4_stages() -> None:
    text = REQUIREMENTS.read_text()
    idx = text.find("### R024 ")
    assert idx >= 0
    section = text[idx : idx + 7000]
    # 4 milestones: M116, M117, M118, M119
    for mid in ("M116", "M117", "M118", "M119"):
        assert mid in section, f"{mid} missing in R024"


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
