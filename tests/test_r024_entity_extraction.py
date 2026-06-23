"""Tests for R024 M119 S02: entity extraction (530 entities)."""

from __future__ import annotations

import json
from pathlib import Path
from typing import cast

import pytest

REPO_ROOT = Path("/root/daily-archive")
R_ENTITY_DIR = REPO_ROOT / "data" / "r024-entity-scale-corpus-v1"
ENTITIES_DIR = R_ENTITY_DIR / "entities"
EVENTS_LOG = R_ENTITY_DIR / "entities-events.jsonl"
SUMMARY = R_ENTITY_DIR / "entities-summary.json"


def _load(path: Path) -> dict[str, object]:
    data = json.loads(path.read_text())
    return dict(data) if isinstance(data, dict) else {}


def test_entities_dir_exists() -> None:
    assert ENTITIES_DIR.exists()


def test_events_log_exists() -> None:
    assert EVENTS_LOG.exists()


def test_summary_exists() -> None:
    assert SUMMARY.exists()


def test_summary_fail_closed() -> None:
    s = _load(SUMMARY)
    fc = s.get("fail_closed_invariants")
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


def test_summary_530_entities() -> None:
    s = _load(SUMMARY)
    assert int(str(s.get("total_entities", 0))) == 530
    assert int(str(s.get("corpus_size", 0))) == 53
    assert int(str(s.get("entity_types_per_article", 0))) == 10
    assert s.get("all_extracted") is True


def test_per_article_10_entities() -> None:
    files = list(ENTITIES_DIR.glob("*.json"))
    assert len(files) == 53
    for f in files:
        data = _load(f)
        assert int(str(data.get("n_entities", 0))) == 10


def test_per_article_10_entity_types() -> None:
    expected = {
        "metadata",
        "table_context",
        "figure_caption_context",
        "citation_context",
        "retrieval_context",
        "title",
        "authors",
        "abstract",
        "keywords",
        "references",
    }
    files = list(ENTITIES_DIR.glob("*.json"))
    for f in files[:5]:
        data = _load(f)
        entities_obj = data.get("entities")
        assert isinstance(entities_obj, list)
        entities = cast(list[dict[str, object]], entities_obj)
        types = {str(e["entity_type"]) for e in entities}
        assert types == expected


def test_events_log_fail_closed() -> None:
    events = [
        dict(json.loads(line)) for line in EVENTS_LOG.read_text().splitlines() if line.strip()
    ]
    assert len(events) == 53
    for e in events:
        assert e["network_fetch_attempted"] is False
        assert e["production_import_attempted"] is False
        assert e["graph_import_allowed"] is False
        assert e["ladybugdb_written"] is False
        assert e["synthetic_only"] is True


def test_no_real_llm_extraction() -> None:
    """Synthetic only; no real LLM-based extraction."""
    s = _load(SUMMARY)
    fc = s.get("fail_closed_invariants")
    assert isinstance(fc, dict)
    assert fc.get("real_llm_extraction_used") is False


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
    _ = cast  # keep for tooling
