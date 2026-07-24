"""TDD: controlled hybrid expand batch gate."""

from __future__ import annotations

from research_graph.application.corpus.hybrid_expand_batch_gate import (
    evaluate_expand_batch_gate,
)


def test_allow_when_ready_sidecars_and_limit() -> None:
    pkg = evaluate_expand_batch_gate(
        preflight_signal="ready_to_batch",
        ready_count=15,
        limit=10,
        enable_live_hybrid=True,
        grobid_available=True,
        odl_available=True,
    )
    assert pkg.allow_live_batch is True
    assert pkg.gate_signal == "allow_limited_batch"
    assert pkg.effective_limit == 10
    assert pkg.import_eligible is False


def test_block_without_grobid() -> None:
    pkg = evaluate_expand_batch_gate(
        preflight_signal="ready_to_batch",
        ready_count=15,
        limit=10,
        enable_live_hybrid=True,
        grobid_available=False,
        odl_available=True,
    )
    assert pkg.allow_live_batch is False
    assert "grobid_unavailable" in pkg.reasons


def test_block_without_live_flag_or_limit() -> None:
    pkg = evaluate_expand_batch_gate(
        preflight_signal="ready_to_batch",
        ready_count=15,
        limit=0,
        enable_live_hybrid=False,
        grobid_available=True,
        odl_available=True,
    )
    assert pkg.allow_live_batch is False
    assert pkg.effective_limit == 0


def test_caps_limit_by_ready_and_max() -> None:
    pkg = evaluate_expand_batch_gate(
        preflight_signal="repair",
        ready_count=5,
        limit=20,
        enable_live_hybrid=True,
        grobid_available=True,
        odl_available=True,
        max_limit=20,
    )
    assert pkg.allow_live_batch is True
    assert pkg.effective_limit == 5
