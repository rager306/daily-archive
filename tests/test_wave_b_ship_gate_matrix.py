"""Unit tests for Wave B ship-gate matrix builder (M260)."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.wave_b_ship_gate_matrix import (
    build_wave_b_ship_gate_matrix,
)


def test_matrix_header_path_ship_ready() -> None:
    pkg = build_wave_b_ship_gate_matrix(
        floor={"entity_f1": 1.0, "relation_f1": 1.0},
        header={"entity_f1": 0.675, "relation_f1": 0.35, "model_id": "header_priority_select"},
        baseline={"train_entity_f1": 0.925, "train_relation_f1": 0.647},
        llm={"entity_f1": 0.625, "relation_f1": 0.30, "model_id": "agnes"},
        joined_count=20,
        grounding_body_ratio=1.0,
        grounding_cand_ratio=1.0,
        human_go=True,
        wave_a_closeout_pass=True,
    )
    assert pkg.import_eligible is False
    assert pkg.dspy_optimizer_enabled is False
    assert pkg.gepa_justified is False
    assert pkg.ship_ready is True
    assert pkg.ship_blocker is None
    assert pkg.ship_path == "header_priority_constrained_select"
    assert pkg.worlds["header_constrained_select"]["entity_f1"] == 0.675
    assert pkg.relation_status["header_relation_f1"] == 0.35
    assert pkg.relation_status["free_invent"] is False
    assert "APPLIED_TO" in pkg.relation_status["allowed_relation_types"]
    assert pkg.deltas["llm_beats_header"] is False
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["relation_status"]["path"] == "header_priority_cooccurrence"


def test_matrix_llm_beats_header_can_justify_gepa_flag() -> None:
    pkg = build_wave_b_ship_gate_matrix(
        floor={"entity_f1": 1.0, "relation_f1": 1.0},
        header={"entity_f1": 0.5, "relation_f1": 0.3},
        llm={"entity_f1": 0.7, "relation_f1": 0.4},
        grounding_body_ratio=1.0,
        grounding_cand_ratio=1.0,
        human_go=True,
        wave_a_closeout_pass=True,
    )
    assert pkg.deltas["llm_beats_header"] is True
    assert pkg.gepa_justified is True
    assert pkg.ship_path == "constrained_llm_prefer_header_candidate"


def test_matrix_blocks_without_human_go() -> None:
    pkg = build_wave_b_ship_gate_matrix(
        header={"entity_f1": 0.675, "relation_f1": 0.35},
        floor={"entity_f1": 1.0, "relation_f1": 1.0},
        grounding_body_ratio=1.0,
        grounding_cand_ratio=1.0,
        human_go=False,
        wave_a_closeout_pass=True,
    )
    assert pkg.ship_ready is False
    assert pkg.ship_blocker == "human_go_false"


def test_matrix_uses_llm_compare_artifact_shape() -> None:
    compare = {
        "header": {"entity_f1": 0.675, "relation_f1": 0.35, "model_id": "header_priority_select"},
        "llm_agnes_free_compact_prompt_prefer_header": {
            "entity_f1": 0.625,
            "relation_f1": 0.3,
            "model_id": "agnes-ai-free/agnes-2.0-flash",
            "llm_kept": 1,
        },
        "delta_vs_header": {"entity_f1": -0.05, "relation_f1": -0.05},
        "joined_count": 20,
        "gepa_justified": False,
    }
    pkg = build_wave_b_ship_gate_matrix(
        floor={"entity_f1": 1.0, "relation_f1": 1.0},
        llm_compare=compare,
        grounding_body_ratio=1.0,
        grounding_cand_ratio=1.0,
        human_go=True,
        wave_a_closeout_pass=True,
    )
    assert pkg.worlds["llm_constrained_compare"]["entity_f1"] == 0.625
    assert pkg.deltas["llm_minus_header_entity_f1"] == -0.05
    assert pkg.gepa_justified is False


def test_rejects_import_true() -> None:
    with pytest.raises(ValueError):
        build_wave_b_ship_gate_matrix(
            header={"entity_f1": 0.6, "relation_f1": 0.3},
            human_go=True,
            wave_a_closeout_pass=True,
            grounding_body_ratio=1.0,
            grounding_cand_ratio=1.0,
        ).__class__(
            schema_version="x",
            worlds={},
            deltas={},
            relation_status={},
            ship_path="x",
            ship_blocker=None,
            ship_ready=True,
            gepa_justified=False,
            dspy_optimizer_enabled=False,
            diagnostics=(),
            import_eligible=True,
        )
