"""Tests for R024 M119 S03: entity quality metrics + comparison (5→10 entity types)."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

REPO_ROOT = Path("/root/daily-archive")
R_ENTITY_DIR = REPO_ROOT / "data" / "r024-entity-scale-corpus-v1"
METRICS = R_ENTITY_DIR / "quality-metrics.json"
COMPARISON = R_ENTITY_DIR / "comparison-5-entities-vs-10-entities.md"


def _load(path: Path) -> dict[str, object]:
    data = json.loads(path.read_text())
    return dict(data) if isinstance(data, dict) else {}


def test_metrics_exists() -> None:
    assert METRICS.exists()


def test_comparison_exists() -> None:
    assert COMPARISON.exists()


def test_metrics_fail_closed() -> None:
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
        "real_llm_extraction_used",
    ):
        assert fc.get(key) is False, f"{key} must be False"
    assert fc.get("synthetic_only") is True


def test_metrics_530_entities() -> None:
    m = _load(METRICS)
    assert int(str(m.get("m119_total_entities", 0))) == 530
    assert int(str(m.get("m118_total_entities", 0))) == 265
    assert int(str(m.get("m119_entity_types_count", 0))) == 10
    assert int(str(m.get("m118_entity_types_count", 0))) == 5


def test_metrics_2x_scale() -> None:
    m = _load(METRICS)
    cmp = m.get("comparison")
    assert isinstance(cmp, dict)
    assert float(str(cmp.get("scale_factor_entities", 0))) == 2.0
    assert float(str(cmp.get("scale_factor_types", 0))) == 2.0


def test_metrics_10_entity_types() -> None:
    m = _load(METRICS)
    by_type = m.get("m119_entities_by_type")
    assert isinstance(by_type, dict)
    assert len(by_type) == 10
    # each type has 53 instances
    for t, n in by_type.items():
        assert int(str(n)) == 53, f"{t}: expected 53, got {n}"


def test_comparison_md_has_fail_closed() -> None:
    text = COMPARISON.read_text()
    assert "Fail-closed invariants" in text or "Fail-Closed Invariants" in text
    assert "synthetic_only" in text


def test_comparison_md_has_distribution() -> None:
    text = COMPARISON.read_text()
    assert "M119 Entity Types Distribution" in text
    assert "metadata" in text
    assert "title" in text
    assert "references" in text


def test_comparison_md_has_summary() -> None:
    text = COMPARISON.read_text()
    assert "Summary" in text
    assert "scale" in text.lower()


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
