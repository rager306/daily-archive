"""M255 S01: deterministic hybrid-body statistical extraction package."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.wave_b_hybrid_statistical_extraction import (
    HybridStatisticalExtractionPackage,
    build_hybrid_statistical_extraction,
    build_hybrid_statistical_fleet,
)


SAMPLE = """
Predictive state representations provide a framework for modeling dynamical systems.
The representation uses tests and predictions rather than latent states of a hidden
Markov model. Predictive state representations compact dynamical systems from
observations. The framework proves theoretical properties of the representation.
"""


def test_build_produces_keywords_and_candidates() -> None:
    pkg = build_hybrid_statistical_extraction(
        paper_id="1207.4167",
        body_text=SAMPLE,
        body_path="/tmp/1207.4167.hybrid.body.md",
    )
    assert pkg.import_eligible is False
    assert pkg.graph_writes_allowed is False
    assert pkg.dspy_optimizer_enabled is False
    assert pkg.llm_used is False
    assert pkg.keyword_source == "token_frequency"
    assert pkg.keyword_count >= 3
    assert pkg.word_count > 20
    assert pkg.extraction_status == "statistical_ready"
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["llm_used"] is False
    assert d["dspy_optimizer_enabled"] is False
    assert len(d["keywords"]) == pkg.keyword_count
    # candidate relations only from co-occurrence pairs
    assert pkg.candidate_relation_count == len(pkg.candidate_relations)
    for rel in pkg.candidate_relations:
        assert rel["relation_type"] == "RELATED_TO"
        assert rel["import_eligible"] is False


def test_empty_body_pending() -> None:
    pkg = build_hybrid_statistical_extraction(
        paper_id="x",
        body_text="   ",
        body_path=None,
    )
    assert pkg.extraction_status == "empty_body"
    assert pkg.keyword_count == 0
    assert pkg.candidate_relation_count == 0
    assert pkg.import_eligible is False


def test_rejects_import_true() -> None:
    with pytest.raises(ValueError, match="import"):
        HybridStatisticalExtractionPackage(
            schema_version="m255-wave-b-hybrid-statistical-extraction.v1",
            paper_id="x",
            body_path=None,
            word_count=0,
            keyword_count=0,
            cooc_pair_count=0,
            candidate_relation_count=0,
            keywords=(),
            candidate_relations=(),
            keyword_source="token_frequency",
            extraction_status="empty_body",
            diagnostics=(),
            llm_used=False,
            dspy_optimizer_enabled=False,
            import_eligible=True,
            graph_writes_allowed=False,
        )


def test_rejects_dspy_true() -> None:
    with pytest.raises(ValueError, match="DSPy|dspy"):
        HybridStatisticalExtractionPackage(
            schema_version="m255-wave-b-hybrid-statistical-extraction.v1",
            paper_id="x",
            body_path=None,
            word_count=0,
            keyword_count=0,
            cooc_pair_count=0,
            candidate_relation_count=0,
            keywords=(),
            candidate_relations=(),
            keyword_source="token_frequency",
            extraction_status="empty_body",
            diagnostics=(),
            llm_used=False,
            dspy_optimizer_enabled=True,
            import_eligible=False,
            graph_writes_allowed=False,
        )


def test_fleet_aggregate_fail_closed() -> None:
    a = build_hybrid_statistical_extraction(
        paper_id="a", body_text=SAMPLE, body_path=None
    )
    b = build_hybrid_statistical_extraction(
        paper_id="b", body_text="   ", body_path=None
    )
    fleet = build_hybrid_statistical_fleet(
        packages=(a, b),
        wave_b_gate_open=True,
        human_go=True,
    )
    assert fleet.paper_count == 2
    assert fleet.empty_count == 1
    assert fleet.statistical_ready_count == 1
    assert fleet.import_eligible is False
    assert fleet.dspy_optimizer_enabled is False
    assert fleet.llm_used is False
    assert fleet.fleet_status == "sampled"
    d = fleet.to_dict()
    assert d["import_eligible"] is False


def test_fleet_blocked_when_gate_closed() -> None:
    a = build_hybrid_statistical_extraction(
        paper_id="a", body_text=SAMPLE, body_path=None
    )
    fleet = build_hybrid_statistical_fleet(
        packages=(a,),
        wave_b_gate_open=False,
        human_go=False,
    )
    assert fleet.fleet_status == "blocked_gate"
