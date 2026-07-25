"""Build staged EvidenceAssertion from claim-like payloads (M279 E4).

Application pure. Uses faithfulness + resolvability. Never import.
"""

from __future__ import annotations

import hashlib
from collections.abc import Mapping, Sequence
from typing import Any

from research_graph.application.corpus.verification_checks import (
    check_claim_faithfulness,
    classify_risk_tier,
)
from research_graph.domain.evidence_assertion import SCHEMA_VERSION, EvidenceAssertion


def _assertion_id(
    paper_id: str, subject: str, predicate: str, obj: str, spans: Sequence[Mapping[str, Any]]
) -> str:
    raw = f"{paper_id}|{subject}|{predicate}|{obj}|{len(spans)}"
    return "ea_" + hashlib.sha256(raw.encode("utf-8")).hexdigest()[:16]


def build_evidence_assertion(
    *,
    paper_id: str,
    subject: str,
    predicate: str,
    object: str,
    claim_text: str,
    span_text: str | None = None,
    spans: Sequence[Mapping[str, Any]] | None = None,
    intent_hash: str | None = None,
    constraints_hash: str | None = None,
    confidence: float | None = None,
    metric: str | None = None,
    conditions: str | None = None,
    provenance: Mapping[str, str] | None = None,
) -> EvidenceAssertion:
    """Build staging assertion; epistemic status from faithfulness."""
    span_list = [dict(s) for s in (spans or ()) if isinstance(s, Mapping)]
    faith = check_claim_faithfulness(
        claim_text=claim_text or f"{subject} {predicate} {object}",
        span_text=span_text,
        spans=span_list,
        relation_type=predicate,
    )
    tier = faith.risk_tier or classify_risk_tier(
        relation_type=predicate, claim_text=claim_text
    )
    if faith.faithful and faith.resolvable:
        epistemic: Any = "supported"
        audit = "passed" if not faith.audit_required else "required"
    elif not faith.resolvable:
        epistemic = "candidate"
        audit = "failed"
    else:
        epistemic = "contested"
        audit = "failed" if faith.promote_blocked else "pending"

    warnings: list[str] = []
    if faith.promote_blocked:
        warnings.append(f"promote_blocked:{faith.reason}")
    if faith.audit_required:
        warnings.append("high_impact_audit_required")

    return EvidenceAssertion(
        schema_version=SCHEMA_VERSION,
        assertion_id=_assertion_id(paper_id, subject, predicate, object, span_list),
        paper_id=paper_id,
        subject=subject,
        predicate=predicate,
        object=object,
        spans=tuple(span_list),
        risk_tier=tier,
        epistemic_status=epistemic,
        audit_status=audit,
        faithfulness_reason=faith.reason,
        resolvable=faith.resolvable,
        intent_hash=intent_hash,
        constraints_hash=constraints_hash,
        confidence=confidence,
        metric=metric,
        conditions=conditions,
        provenance=dict(provenance or {}),
        validation_warnings=tuple(warnings),
    )


__all__ = ["build_evidence_assertion"]
