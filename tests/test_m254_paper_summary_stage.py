"""M254 S04: optional fail-closed paper summary stage package."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.paper_summary_stage import (
    PaperSummaryStagePackage,
    build_paper_summary_stage,
)
from research_graph.domain.universal_kb.contracts import SafetyFlags


def test_build_stage_from_fields_fail_closed() -> None:
    pkg = build_paper_summary_stage(
        paper_id="1207.4167",
        title="PSR",
        abstract="Predictive state representations.",
        headline="PSRs model dynamical systems.",
        what_it_does="They use predictive tests. Theory is developed.",
        why_it_matters="Compact alternative to HMMs.",
        analogy="Think of it like forecasting weather from patterns.",
        binding_id="paper-summary-generate-default",
        model_name="agnes-ai/agnes-2.0-flash",
        role="default",
    )
    assert pkg.import_eligible is False
    assert pkg.graph_writes_allowed is False
    assert pkg.stage_status == "ready_for_review"
    assert pkg.paper_id == "1207.4167"
    assert pkg.headline.startswith("PSRs")
    assert pkg.safety_flags == SafetyFlags()
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["graph_writes_allowed"] is False
    assert d["binding_id"] == "paper-summary-generate-default"
    assert d["model_name"] == "agnes-ai/agnes-2.0-flash"


def test_incomplete_fields_pending() -> None:
    pkg = build_paper_summary_stage(
        paper_id="x",
        title="T",
        abstract="A",
        headline="",
        what_it_does="",
        why_it_matters="",
        analogy="",
        binding_id="paper-summary-generate-default",
        model_name="agnes-ai/agnes-2.0-flash",
        role="default",
        error="not_generated",
    )
    assert pkg.stage_status == "pending"
    assert pkg.import_eligible is False
    assert pkg.error == "not_generated"


def test_rejects_import_true() -> None:
    with pytest.raises(ValueError, match="import"):
        PaperSummaryStagePackage(
            schema_version="paper-summary-stage.v1",
            paper_id="x",
            title="t",
            abstract="a",
            headline="h",
            what_it_does="w",
            why_it_matters="y",
            analogy="Think of it like z",
            binding_id="b",
            model_name="m",
            role="default",
            stage_status="ready_for_review",
            safety_flags=SafetyFlags(),
            diagnostics=(),
            import_eligible=True,
            graph_writes_allowed=False,
            error=None,
        )


def test_rejects_graph_writes() -> None:
    with pytest.raises(ValueError, match="import|write"):
        PaperSummaryStagePackage(
            schema_version="paper-summary-stage.v1",
            paper_id="x",
            title="t",
            abstract="a",
            headline="h",
            what_it_does="w",
            why_it_matters="y",
            analogy="Think of it like z",
            binding_id="b",
            model_name="m",
            role="default",
            stage_status="ready_for_review",
            safety_flags=SafetyFlags(),
            diagnostics=(),
            import_eligible=False,
            graph_writes_allowed=True,
            error=None,
        )


def test_optional_composition_wire_is_off_by_default() -> None:
    from research_graph.workflows.composition import paper_summary_stage as wire

    assert wire.DEFAULT_SUMMARY_STAGE_ENABLED is False
    assert wire.should_run_summary_stage(enabled=None) is False
    assert wire.should_run_summary_stage(enabled=False) is False
    assert wire.should_run_summary_stage(enabled=True) is True
