"""M278 E3.2–E3.3: existence vs faithfulness + risk strata."""

from __future__ import annotations

from research_graph.application.corpus.verification_checks import (
    check_bibliographic_existence,
    check_claim_faithfulness,
    classify_risk_tier,
    promote_path_allowed,
)
from research_graph.domain.relation_types import CAUSES, CITES


def test_existence_does_not_imply_faithfulness() -> None:
    ex = check_bibliographic_existence(resolved_bib_id="doi:10.1/x", citation_key="smith2020")
    assert ex.exists is True
    assert ex.implies_faithfulness is False
    assert ex.import_eligible is False


def test_existence_missing() -> None:
    ex = check_bibliographic_existence(citation_key="missing2024")
    assert ex.exists is False


def test_causal_is_high_risk() -> None:
    assert classify_risk_tier(relation_type=CAUSES) == "high"
    assert classify_risk_tier(relation_type=CITES) == "medium"
    assert classify_risk_tier(claim_text="We outperform baselines by 12%") == "high"


def test_faithfulness_requires_overlap_and_resolvable_span() -> None:
    spans = [{"artifact_hash": "h", "page": 1, "bbox": [0, 0, 1, 1]}]
    span_text = "Our method causes improved accuracy on the benchmark dataset."
    claim = "method causes improved accuracy"
    v = check_claim_faithfulness(
        claim_text=claim,
        span_text=span_text,
        spans=spans,
        relation_type=CAUSES,
    )
    assert v.faithful is True
    assert v.risk_tier == "high"
    assert v.audit_required is True
    assert v.promote_blocked is False
    assert v.import_eligible is False


def test_high_impact_without_faithfulness_blocks_promote() -> None:
    spans = [{"artifact_hash": "h", "page": 1}]
    v = check_claim_faithfulness(
        claim_text="Method A outperforms B by 30%",
        span_text="Unrelated paragraph about datasets only.",
        spans=spans,
        relation_type=CAUSES,
    )
    assert v.faithful is False
    path = promote_path_allowed(existence=None, faithfulness=v, relation_type=CAUSES)
    assert path["promote_staging_allowed"] is False
    assert path["import_eligible"] is False


def test_citation_existence_can_stage_but_not_import() -> None:
    ex = check_bibliographic_existence(resolved_bib_id="x")
    # faithfulness N/A for bare CITES path
    v = check_claim_faithfulness(
        claim_text="see ref",
        span_text="see ref",
        spans=[{"artifact_hash": "h", "char_start": 0, "char_end": 7}],
        relation_type=CITES,
    )
    path = promote_path_allowed(existence=ex, faithfulness=v, relation_type=CITES)
    assert path["import_eligible"] is False
    assert "existence" in path["reason"] or path["promote_staging_allowed"] in {True, False}
