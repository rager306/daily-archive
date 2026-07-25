"""M235 S01: empty preprocess_rollup default is fail-closed and fresh per instance."""

from __future__ import annotations

from research_graph.application.corpus.preprocess_rollup import (
    empty_preprocess_rollup,
    rollup_preprocess_bodies,
)


def test_empty_preprocess_rollup_flags() -> None:
    r = empty_preprocess_rollup()
    assert r == rollup_preprocess_bodies([])
    assert r["body_count"] == 0
    assert r["drives_verdict"] is False
    assert r["import_eligible"] is False
    assert r["quality_status_counts"] == {}
    assert r["keyword_source_counts"] == {}


def test_empty_rollup_is_fresh_mutable_per_call() -> None:
    a = empty_preprocess_rollup()
    b = empty_preprocess_rollup()
    assert a is not b
    a["body_count"] = 99
    assert b["body_count"] == 0


def test_hybrid_result_default_rollup_flags() -> None:
    # Import dataclasses only; construct minimal via type defaults inspection.
    from research_graph.workflows.composition.hybrid_readiness_handoff import (
        HybridReadinessHandoffResult,
    )

    field = HybridReadinessHandoffResult.__dataclass_fields__["preprocess_rollup"]
    default = field.default_factory()  # type: ignore[misc]
    assert default["drives_verdict"] is False
    assert default["import_eligible"] is False
    assert default["body_count"] == 0


def test_non_arxiv_result_default_rollup_flags() -> None:
    from research_graph.workflows.composition.non_arxiv_html_source_proof import (
        NonArxivHtmlSourceProofResult,
    )

    field = NonArxivHtmlSourceProofResult.__dataclass_fields__["preprocess_rollup"]
    default = field.default_factory()  # type: ignore[misc]
    assert default["drives_verdict"] is False
    assert default["import_eligible"] is False
    assert default["body_count"] == 0
