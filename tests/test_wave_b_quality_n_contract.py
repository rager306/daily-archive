"""Tests for Wave B same-n quality contract (M271)."""

from __future__ import annotations

from research_graph.application.corpus.wave_b_quality_n_contract import (
    evaluate_quality_n_contract,
    extract_joined_count,
)
from research_graph.application.corpus.wave_b_ship_gate_matrix import (
    build_wave_b_ship_gate_matrix,
)


def test_n_contract_all_match() -> None:
    c = evaluate_quality_n_contract(
        header_n=23, llm_n=23, gepa_n=23, grounding_n=23, matrix_n=23, canonical=23
    )
    assert c.all_match is True
    assert c.canonical_joined_count == 23
    assert c.import_eligible is False


def test_n_contract_mismatch() -> None:
    c = evaluate_quality_n_contract(
        header_n=23, llm_n=20, gepa_n=23, grounding_n=20, matrix_n=23, canonical=23
    )
    assert c.all_match is False
    assert any("llm:20" in m for m in c.mismatches)


def test_extract_joined_count_nested() -> None:
    assert extract_joined_count({"joined_count": 23}) == 23
    assert extract_joined_count({"worlds": {"context": {"joined_count": 20}}}) == 20
    assert extract_joined_count({"metrics": {"case_count": 15}}) == 15


def test_matrix_gepa_same_n_blocks_promote() -> None:
    """GEPA dual F1 win on different n cannot promote (M271)."""
    pkg = build_wave_b_ship_gate_matrix(
        floor={"entity_f1": 1.0, "relation_f1": 1.0},
        header={"entity_f1": 0.5, "relation_f1": 0.26},
        offline_gepa={
            "entity_f1": 0.75,
            "relation_f1": 0.40,
            "train_entity_f1": 0.7,
            "val_entity_f1": 0.55,
            "joined_count": 20,  # stale vs live 23
            "promote_ready": True,
        },
        joined_count=23,
        grounding_body_ratio=1.0,
        grounding_cand_ratio=1.0,
        human_go=True,
        wave_a_closeout_pass=True,
    )
    assert pkg.deltas["gepa_beats_header"] is False
    assert pkg.deltas["gepa_n_matches"] is False
    assert pkg.ship_path == "header_priority_constrained_select"
    assert pkg.worlds["context"]["gepa_n_matches"] is False
