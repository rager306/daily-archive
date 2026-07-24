"""M278 E3.4–E3.5: audit cache key + GT isolation."""

from __future__ import annotations

from research_graph.application.corpus.audit_cache import build_audit_cache_key
from research_graph.application.corpus.gt_isolation import (
    assert_context_isolated,
    check_gt_isolation,
    freeze_canary_split,
)


def test_audit_cache_key_changes_with_constraints() -> None:
    payload = {"relation_type": "CAUSES", "text": "x causes y"}
    spans = [{"page": 1, "artifact_hash": "a"}]
    k1 = build_audit_cache_key(
        payload=payload, spans=spans, constraints_hash="c1", judge_id="j"
    )
    k2 = build_audit_cache_key(
        payload=payload, spans=spans, constraints_hash="c2", judge_id="j"
    )
    k1b = build_audit_cache_key(
        payload=payload, spans=spans, constraints_hash="c1", judge_id="j"
    )
    assert k1 == k1b
    assert k1 != k2
    assert len(k1) == 64


def test_gt_isolation_blocks_held_out_in_train() -> None:
    v = check_gt_isolation(
        context_paper_ids=["p1", "p2", "hold1"],
        held_out_ids=["hold1", "hold2"],
        role="train",
    )
    assert v.ok is False
    assert any("held_out_in_train:hold1" in x for x in v.violations)
    assert v.import_eligible is False


def test_gt_isolation_allows_held_out_in_eval() -> None:
    v = check_gt_isolation(
        context_paper_ids=["hold1"],
        held_out_ids=["hold1"],
        role="eval",
    )
    assert v.ok is True


def test_gold_marker_in_prompt_blocked() -> None:
    v = check_gt_isolation(
        context_paper_ids=["p1"],
        held_out_ids=["hold1"],
        role="prompt",
        context_blob="system: gold label for hold1 is OUTPERFORMS",
        gold_markers=["gold label for hold1"],
    )
    assert v.ok is False
    assert any("gold_marker_in_prompt" in x for x in v.violations)


def test_freeze_canary_split() -> None:
    split = freeze_canary_split(
        ["a", "b", "c", "d"],
        held_out_ids=["c", "d"],
    )
    assert split["frozen"] is True
    assert split["held_out_count"] == 2
    assert split["import_eligible"] is False


def test_assert_context_isolated() -> None:
    v = assert_context_isolated(
        {"paper_ids": ["x"], "prompt_text": "ok"},
        held_out_ids=["y"],
        role="prompt",
    )
    assert v.ok is True
