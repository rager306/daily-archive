"""Tests for R024 M119 S01: 10-entity-type schema + pivot rationale."""

from __future__ import annotations

import json
from pathlib import Path
from typing import cast

import pytest

REPO_ROOT = Path("/root/daily-archive")
R_ENTITY_DIR = REPO_ROOT / "data" / "r024-entity-scale-corpus-v1"
SCHEMA_JSON = R_ENTITY_DIR / "entity-schema.json"
SCHEMA_MD = R_ENTITY_DIR / "entity-schema.md"


def _load(path: Path) -> dict[str, object]:
    data = json.loads(path.read_text())
    return dict(data) if isinstance(data, dict) else {}


def test_schema_json_exists() -> None:
    assert SCHEMA_JSON.exists()


def test_schema_md_exists() -> None:
    assert SCHEMA_MD.exists()


def test_schema_has_10_entity_types() -> None:
    s = _load(SCHEMA_JSON)
    types = s.get("entity_types")
    assert isinstance(types, list)
    assert len(types) == 10


def test_schema_entity_type_names() -> None:
    s = _load(SCHEMA_JSON)
    types_obj = s.get("entity_types")
    assert isinstance(types_obj, list)
    types = cast(list[dict[str, object]], types_obj)
    names = [str(t["entity_type"]) for t in types]
    expected = {
        "metadata",
        "table_context",
        "figure_caption_context",
        "citation_context",
        "retrieval_context",
        "title",
        "authors",
        "abstract",
        "references",
        "keywords",
    }
    assert set(names) == expected


def test_schema_pivot_rationale() -> None:
    s = _load(SCHEMA_JSON)
    pivot = s.get("pivot_rationale")
    assert isinstance(pivot, dict)
    assert pivot.get("catalog_exhausted") is True
    assert pivot.get("catalog_limit") == 55
    assert pivot.get("M118_baseline_entities") == 265
    assert pivot.get("M119_target_entities") == 530
    assert pivot.get("scale_factor") == 2.0


def test_schema_fail_closed() -> None:
    s = _load(SCHEMA_JSON)
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


def test_schema_counts() -> None:
    s = _load(SCHEMA_JSON)
    counts = s.get("counts")
    assert isinstance(counts, dict)
    assert counts.get("total_entity_types") == 10
    assert counts.get("from_catalog_chunk_types") == 5
    assert counts.get("from_article_metadata") == 4
    assert counts.get("synthetic_from_metadata_plus_chunks") == 1


def test_schema_md_has_pivot_rationale() -> None:
    text = SCHEMA_MD.read_text()
    assert "Pivot Rationale" in text
    assert "catalog exhausted" in text.lower()
    assert "53 articles" in text or "55 articles" in text


def test_schema_md_has_entity_table() -> None:
    text = SCHEMA_MD.read_text()
    assert "Entity Type" in text
    assert "Source" in text
    assert "Derivation" in text


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
