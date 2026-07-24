"""M277 E2.2: canary corpus design plan (design only)."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from research_graph.application.corpus.canary_corpus_design import (
    SCHEMA_VERSION,
    assign_canary_paper_ids,
    build_canary_corpus_design,
)


def test_build_default_at_least_60_slots() -> None:
    design = build_canary_corpus_design()
    assert design.schema_version == SCHEMA_VERSION
    assert design.target_size >= 60
    assert len(design.slots) >= 60
    assert design.import_eligible is False
    assert design.graph_writes_allowed is False
    strata = {s.stratum for s in design.slots}
    assert "heavy_tables" in strata
    assert "formulas_equations" in strata
    assert "ocr_or_scanned" in strata
    payload = design.to_dict()
    assert payload["annotation_status"] == "design_only"
    assert payload["import_eligible"] is False


def test_rejects_small_target() -> None:
    with pytest.raises(ValueError, match="60"):
        build_canary_corpus_design(target_size=10)


def test_partial_assignment() -> None:
    design = build_canary_corpus_design(
        target_size=60,
        assigned=[("c001", "2507.00001")],
    )
    by_id = {s.slot_id: s for s in design.slots}
    assert by_id["c001"].paper_id == "2507.00001"
    assert sum(1 for s in design.slots if s.paper_id) == 1


def test_write_artifact_fixture(tmp_path: Path) -> None:
    design = build_canary_corpus_design(target_size=60)
    path = tmp_path / "canary-corpus-design.v1.json"
    path.write_text(
        json.dumps(design.to_dict(), indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    loaded = json.loads(path.read_text(encoding="utf-8"))
    assert loaded["target_size"] >= 60
    assert len(loaded["slots"]) >= 60


def test_assign_canary_paper_ids_full_and_held_out() -> None:
    design = build_canary_corpus_design(target_size=60)
    pool = [f"paper.{i:04d}" for i in range(70)]
    assigned, held, freeze = assign_canary_paper_ids(
        design, pool, held_out_count=12, held_out_seed=7
    )
    assert assigned.assigned_count == 60
    assert assigned.annotation_status == "ids_assigned_labels_pending"
    assert assigned.import_eligible is False
    assert len(held) >= 1
    assert freeze["frozen"] is True
    assert freeze["import_eligible"] is False
    # held-out subset of assigned
    assigned_ids = {s.paper_id for s in assigned.slots if s.paper_id}
    assert set(held).issubset(assigned_ids)
    # deterministic
    assigned2, held2, _ = assign_canary_paper_ids(
        design, pool, held_out_count=12, held_out_seed=7
    )
    assert held == held2
    assert [s.paper_id for s in assigned.slots] == [s.paper_id for s in assigned2.slots]


def test_assign_requires_enough_ids() -> None:
    design = build_canary_corpus_design(target_size=60)
    with pytest.raises(ValueError, match="need at least 60"):
        assign_canary_paper_ids(design, [f"p{i}" for i in range(10)])
