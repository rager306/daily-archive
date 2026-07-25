"""M282: structure readiness demotes weak IR (gate pass, ir_hard=0)."""

from __future__ import annotations

from research_graph.application.corpus.structure_readiness_package import (
    build_structure_readiness_package,
)


def test_weak_ir_demotes_ready_to_partial() -> None:
    pkg = build_structure_readiness_package(
        structure_layer={
            "health": "present",
            "present_seams": ["chunk_quality"],
            "missing_seams": [],
            "gaps": [],
        },
        hybrid_found=10,
        hybrid_fraction=0.4,
        closeout_signal="wave_a_closed",
        chunk_quality_gate={
            "gate_signal": "pass",
            "continuity_gap_cleared": True,
            "ir_hard_count": 0,
            "newline_demoted_count": 12,
        },
    )
    assert pkg.import_eligible is False
    assert pkg.weak_structure_ir is True
    assert pkg.structure_signal == "partial"
    assert any("weak_structure_ir" in a for a in pkg.alerts)


def test_ir_hard_positive_can_be_ready() -> None:
    pkg = build_structure_readiness_package(
        structure_layer={
            "health": "present",
            "present_seams": ["chunk_quality"],
            "missing_seams": [],
            "gaps": [],
        },
        hybrid_found=10,
        chunk_quality_gate={
            "gate_signal": "pass",
            "continuity_gap_cleared": True,
            "ir_hard_count": 8,
            "newline_demoted_count": 0,
        },
    )
    assert pkg.weak_structure_ir is False
    assert pkg.structure_signal == "ready_for_structure_review"
    assert pkg.ir_hard_count == 8
