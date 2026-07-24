"""M279 E4: EvidenceAssertion staging + silent upgrade + promote boundary."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.assertion_lifecycle import (
    detect_silent_upgrades,
    promotion_boundary_check,
)
from research_graph.application.corpus.evidence_assertion_build import (
    build_evidence_assertion,
)
from research_graph.domain.evidence_assertion import SCHEMA_VERSION, EvidenceAssertion
from research_graph.domain.relation_types import CAUSES


def test_build_supported_assertion() -> None:
    spans = [{"artifact_hash": "h", "page": 1, "bbox": [0, 0, 1, 1]}]
    ea = build_evidence_assertion(
        paper_id="p1",
        subject="MethodA",
        predicate=CAUSES,
        object="AccuracyGain",
        claim_text="MethodA causes accuracy gain on the benchmark",
        span_text="MethodA causes accuracy gain on the benchmark dataset results.",
        spans=spans,
        confidence=0.9,
    )
    assert ea.schema_version == SCHEMA_VERSION
    assert ea.import_eligible is False
    assert ea.resolvable is True
    assert ea.epistemic_status in {"supported", "contested", "candidate"}
    assert ea.risk_tier == "high"
    payload = ea.to_dict()
    assert payload["import_eligible"] is False
    assert "identity_key" in payload


def test_assertion_rejects_import() -> None:
    with pytest.raises(ValueError, match="import"):
        EvidenceAssertion(
            schema_version=SCHEMA_VERSION,
            assertion_id="x",
            paper_id="p",
            subject="a",
            predicate="CITES",
            object="b",
            spans=(),
            risk_tier="low",
            epistemic_status="candidate",
            audit_status="pending",
            import_eligible=True,
        )


def test_silent_upgrade_on_span_change() -> None:
    spans1 = [{"artifact_hash": "h1", "page": 1}]
    spans2 = [{"artifact_hash": "h2", "page": 2}]
    a1 = build_evidence_assertion(
        paper_id="p",
        subject="A",
        predicate="USES_TECHNIQUE",
        object="B",
        claim_text="A uses technique B in experiments",
        span_text="A uses technique B in experiments carefully.",
        spans=spans1,
        confidence=0.5,
    )
    a2 = build_evidence_assertion(
        paper_id="p",
        subject="A",
        predicate="USES_TECHNIQUE",
        object="B",
        claim_text="A uses technique B in experiments",
        span_text="A uses technique B in experiments carefully.",
        spans=spans2,
        confidence=0.9,
    )
    findings = detect_silent_upgrades([a1], [a2])
    assert findings
    assert "spans" in findings[0].changed_fields or "confidence" in findings[0].changed_fields


def test_promotion_boundary_requires_user_go() -> None:
    spans = [{"artifact_hash": "h", "page": 1, "bbox": [0, 0, 1, 1]}]
    ea = build_evidence_assertion(
        paper_id="p",
        subject="M",
        predicate="USES_TECHNIQUE",
        object="T",
        claim_text="M uses technique T successfully",
        span_text="M uses technique T successfully in the paper body.",
        spans=spans,
    )
    blocked = promotion_boundary_check(ea, user_go=False)
    assert blocked["import_eligible"] is False
    assert "user_go_required" in blocked["block_reasons"]
    still = promotion_boundary_check(ea, user_go=True, observed_precision=0.9)
    assert still["import_eligible"] is False
