"""M279: GT isolation wired into offline GEPA reflective spike."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.wave_b_gepa_constrained_spike import (
    offline_reflective_spike,
    partition_cases_for_gepa,
)


def _case(cid: str, body: str = "method applies to task with dataset metric.") -> dict:
    return {
        "case_id": cid,
        "paper_id": cid,
        "body_text": body + " " + ("extra scholarly text. " * 20),
        "gold_entities": [],
        "gold_relations": [],
    }


def test_partition_excludes_held_out_from_train() -> None:
    cases = [_case(f"p{i}") for i in range(6)]
    part = partition_cases_for_gepa(
        cases,
        held_out_case_ids=["p0", "p1"],
        train_ratio=0.67,
        split_seed=0,
    )
    train_ids = {str(c.get("case_id")) for c in part["train"]}
    assert "p0" not in train_ids
    assert "p1" not in train_ids
    assert part["isolation"].ok is True
    assert part["isolation"].import_eligible is False


def test_partition_fails_if_held_out_forced_into_train_context() -> None:
    cases = [_case("a"), _case("b"), _case("c")]
    with pytest.raises(ValueError, match="gt_isolation"):
        partition_cases_for_gepa(
            cases,
            held_out_case_ids=["a"],
            train_ratio=0.67,
            split_seed=0,
            # simulate leakage by also putting held-out into explicit train_ids override
            force_train_ids=["a", "b"],
        )


def test_offline_spike_diagnostics_include_isolation() -> None:
    cases = [_case(f"c{i}") for i in range(4)]
    pkg = offline_reflective_spike(
        cases=cases,
        held_out_case_ids=["c3"],
        max_iterations=1,
        train_ratio=0.5,
    )
    assert pkg.import_eligible is False
    diag = " ".join(pkg.diagnostics)
    assert "gt_isolation_ok:true" in diag
    assert "held_out:1" in diag or "held_out_count:1" in diag
    train_ids = set(pkg.train_case_ids)
    assert "c3" not in train_ids
    assert pkg.gt_isolation_ok is True
    assert "c3" in pkg.held_out_case_ids
    assert "held_out_in_train" not in diag
