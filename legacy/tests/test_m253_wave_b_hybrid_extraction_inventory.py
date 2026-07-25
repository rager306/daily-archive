"""M253: hybrid-body extraction candidate inventory."""

from __future__ import annotations

from pathlib import Path

from research_graph.application.corpus.wave_b_hybrid_extraction_inventory import (
    WaveBHybridExtractionInventoryPackage,
    inventory_hybrid_extraction_candidates,
)


_BODY = """# Graph Neural Networks for Structured Learning

## Abstract
Graph neural networks process graph-structured data using iterative message
passing between neighboring nodes with enough scholarly prose for inventory.
"""


def _body(root: Path, paper_id: str, text: str = _BODY) -> None:
    p = root / paper_id / "body" / f"{paper_id}.hybrid.body.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")


def test_inventory_counts_unique_bodies(tmp_path: Path) -> None:
    r1 = tmp_path / "r1"
    r2 = tmp_path / "r2"
    _body(r1, "p1")
    _body(r2, "p1")
    _body(r1, "p2")
    _body(r1, "empty", text="\n")

    pkg = inventory_hybrid_extraction_candidates(
        body_roots=(r1, r2),
        sample_limit=10,
    )
    assert pkg.import_eligible is False
    assert pkg.dspy_optimizer_enabled is False
    assert pkg.candidate_count == 3
    assert pkg.empty_count == 1
    assert pkg.total_words > 0
    ids = {c.paper_id for c in pkg.candidates}
    assert "p1" in ids and "p2" in ids
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["wave"] == "B"


def test_rejects_import_true() -> None:
    import pytest

    with pytest.raises(ValueError):
        WaveBHybridExtractionInventoryPackage(
            schema_version="x",
            candidate_count=0,
            empty_count=0,
            total_chars=0,
            total_words=0,
            candidates=(),
            diagnostics=(),
            import_eligible=True,
        )
