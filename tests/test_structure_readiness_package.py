"""Unit tests for structure readiness package (M262)."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.structure_readiness_package import (
    build_structure_readiness_package,
    extract_structure_layer,
)


def test_structure_ready_when_seams_present_and_hybrid() -> None:
    pkg = build_structure_readiness_package(
        structure_layer={
            "layer": "structure",
            "health": "present",
            "present_seams": ["a.py", "b.py"],
            "missing_seams": [],
            "gaps": [],
        },
        pipeline_overall="partial",
        hybrid_found=64,
        hybrid_fraction=0.2783,
        closeout_signal="wave_a_closed",
        citation_verdict="ready_for_human_review",
    )
    assert pkg.import_eligible is False
    assert pkg.structure_signal == "ready_for_structure_review"
    assert pkg.hybrid_found == 64
    assert pkg.closeout_signal == "wave_a_closed"
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["graph_writes_allowed"] is False


def test_structure_partial_on_known_gaps() -> None:
    pkg = build_structure_readiness_package(
        structure_layer={
            "health": "partial",
            "present_seams": ["a.py"],
            "missing_seams": [],
            "gaps": ["real_corpus_chunk_quality_not_continuously_gated"],
        },
        hybrid_found=64,
        closeout_signal="wave_a_closed",
    )
    assert pkg.structure_signal == "partial"
    assert any(a.startswith("structure_gaps:") for a in pkg.alerts)


def test_structure_blocked_when_no_seams() -> None:
    pkg = build_structure_readiness_package(
        structure_layer={
            "health": "blocked",
            "present_seams": [],
            "missing_seams": ["missing.py"],
            "gaps": [],
        }
    )
    assert pkg.structure_signal == "blocked"


def test_extract_structure_layer_from_audit_dict() -> None:
    layer = extract_structure_layer(
        {
            "layers": [
                {"layer": "source", "health": "present"},
                {
                    "layer": "structure",
                    "health": "partial",
                    "present_seams": ["x"],
                    "missing_seams": [],
                    "gaps": ["g"],
                },
            ]
        }
    )
    assert layer["layer"] == "structure"
    assert layer["health"] == "partial"


def test_rejects_import_true() -> None:
    with pytest.raises(ValueError):
        build_structure_readiness_package(
            structure_layer={"health": "present", "present_seams": ["a"], "missing_seams": [], "gaps": []}
        ).__class__(
            schema_version="x",
            structure_signal="ready_for_structure_review",
            structure_layer_health="present",
            structure_present_seams=("a",),
            structure_missing_seams=(),
            structure_gaps=(),
            hybrid_found=1,
            hybrid_fraction=0.1,
            closeout_signal="wave_a_closed",
            citation_verdict=None,
            pipeline_overall="partial",
            diagnostics=(),
            alerts=(),
            import_eligible=True,
        )
