"""M243 S01: preprocess fleet metrics on unique hybrid bodies (import-blocked)."""

from __future__ import annotations

from pathlib import Path

from research_graph.application.corpus.etl_preprocess_fleet_audit import (
    audit_preprocess_fleet,
    discover_unique_hybrid_bodies,
)


_BODY = """# Graph Neural Networks

## Abstract
Graph neural networks process graph-structured data using message passing.

## Method
We evaluate citation graphs and molecular graphs for prediction tasks.

## Results
Enough scholarly prose for quality scoring and language detection.
"""


def _write_body(root: Path, paper_id: str, text: str = _BODY) -> Path:
    p = root / paper_id / "body" / f"{paper_id}.hybrid.body.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")
    return p


def test_discover_dedupes_multi_root(tmp_path: Path) -> None:
    r1 = tmp_path / "r1"
    r2 = tmp_path / "r2"
    _write_body(r1, "p1")
    _write_body(r2, "p1")
    _write_body(r1, "p2")
    found = discover_unique_hybrid_bodies((r1, r2))
    assert len(found) == 2
    ids = {x.paper_id for x in found}
    assert ids == {"p1", "p2"}
    # first root wins for p1
    p1 = next(x for x in found if x.paper_id == "p1")
    assert "r1" in str(p1.path)


def test_fleet_aggregates_quality_and_language(tmp_path: Path) -> None:
    r1 = tmp_path / "bodies"
    _write_body(r1, "p1")
    _write_body(r1, "p2")
    _write_body(r1, "short", text="hi\n")

    pkg = audit_preprocess_fleet(body_roots=(r1,))
    assert pkg.import_eligible is False
    assert pkg.graph_writes_allowed is False
    assert pkg.body_count == 3
    assert pkg.error_count == 0
    assert sum(pkg.quality_status_counts.values()) == 3
    assert sum(pkg.language_counts.values()) == 3
    assert "token_frequency" in pkg.keyword_source_counts
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["body_count"] == 3
    assert "quality_status_counts" in d
    assert len(d["samples"]) <= 3


def test_empty_roots_zero(tmp_path: Path) -> None:
    pkg = audit_preprocess_fleet(body_roots=(tmp_path / "missing",))
    assert pkg.body_count == 0
    assert pkg.quality_status_counts == {}
    assert pkg.import_eligible is False


def test_rejects_import_true() -> None:
    import pytest
    from research_graph.application.corpus.etl_preprocess_fleet_audit import (
        EtlPreprocessFleetPackage,
    )

    with pytest.raises(ValueError):
        EtlPreprocessFleetPackage(
            schema_version="x",
            body_count=0,
            error_count=0,
            quality_status_counts={},
            language_counts={},
            keyword_source_counts={},
            samples=(),
            diagnostics=(),
            import_eligible=True,
        )
