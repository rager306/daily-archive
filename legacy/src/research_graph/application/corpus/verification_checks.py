"""Existence vs faithfulness + risk strata (M278 E3.2–E3.3).

Application pure. Bibliographic existence ≠ claim support.
High-impact relations require faithfulness + resolvable span for promote path.
Import always false.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any, Literal

from research_graph.application.corpus.evidence_resolvability import (
    evaluate_assertion_resolvability,
)
from research_graph.domain.relation_types import (
    CAUSAL_RELATIONS,
    CITATION_RELATIONS,
    COMPARISON_RELATIONS,
)

RiskTier = Literal["high", "medium", "low"]

# High-impact: causal + comparison claims that often drive wrong graph edges
HIGH_IMPACT_RELATIONS: frozenset[str] = frozenset(CAUSAL_RELATIONS | COMPARISON_RELATIONS)
# Numerical / abstract-conclusion style markers (string signals on claim text)
_NUMERICAL_MARKERS = (
    "%",
    "pp.",
    "percentage",
    "outperform",
    "outperforms",
    "state-of-the-art",
    "sota",
    "significantly",
    "p<",
    "p <",
)


@dataclass(frozen=True, slots=True)
class ExistenceVerdict:
    """Bibliographic / citation existence only — not claim support."""

    exists: bool
    check: str
    detail: str
    implies_faithfulness: bool = False  # always False by policy
    import_eligible: bool = False

    def to_dict(self) -> dict[str, Any]:
        return {
            "exists": self.exists,
            "check": self.check,
            "detail": self.detail,
            "implies_faithfulness": False,
            "import_eligible": False,
            "graph_writes_allowed": False,
        }


@dataclass(frozen=True, slots=True)
class FaithfulnessVerdict:
    """Claim text/relation grounded in span evidence."""

    faithful: bool
    reason: str
    risk_tier: RiskTier
    audit_required: bool
    resolvable: bool
    promote_blocked: bool
    import_eligible: bool = False

    def to_dict(self) -> dict[str, Any]:
        return {
            "faithful": self.faithful,
            "reason": self.reason,
            "risk_tier": self.risk_tier,
            "audit_required": self.audit_required,
            "resolvable": self.resolvable,
            "promote_blocked": self.promote_blocked,
            "import_eligible": False,
            "graph_writes_allowed": False,
        }


def classify_risk_tier(
    *,
    relation_type: str | None = None,
    claim_text: str | None = None,
) -> RiskTier:
    """Risk strata for audit path (E3.3)."""
    rt = (relation_type or "").strip()
    text = (claim_text or "").casefold()
    if rt in HIGH_IMPACT_RELATIONS:
        return "high"
    if rt in CITATION_RELATIONS:
        # SUPPORTS/CONTRASTS can be high if claimy; bare CITES is medium
        if rt == "CITES":
            return "medium"
        return "high"
    if any(m in text for m in _NUMERICAL_MARKERS):
        return "high"
    if "conclusion" in text or "we show" in text or "we prove" in text:
        return "high"
    if rt:
        return "medium"
    return "low"


def check_bibliographic_existence(
    *,
    citation_key: str | None = None,
    resolved_bib_id: str | None = None,
    title_found: bool = False,
    in_local_kb: bool = False,
) -> ExistenceVerdict:
    """Existence-only check. Never implies claim faithfulness."""
    if resolved_bib_id:
        return ExistenceVerdict(
            exists=True,
            check="resolved_bib_id",
            detail=str(resolved_bib_id),
        )
    if in_local_kb and citation_key:
        return ExistenceVerdict(
            exists=True,
            check="local_kb",
            detail=str(citation_key),
        )
    if title_found:
        return ExistenceVerdict(
            exists=True,
            check="title_match",
            detail=citation_key or "title",
        )
    if citation_key:
        return ExistenceVerdict(
            exists=False,
            check="unresolved_key",
            detail=str(citation_key),
        )
    return ExistenceVerdict(
        exists=False,
        check="missing_identifier",
        detail="no citation key or bib id",
    )


def _token_overlap(claim: str, span_text: str) -> float:
    def toks(s: str) -> set[str]:
        return {t for t in "".join(ch.lower() if ch.isalnum() else " " for ch in s).split() if len(t) > 2}

    a, b = toks(claim), toks(span_text)
    if not a or not b:
        return 0.0
    return len(a & b) / len(a)


def check_claim_faithfulness(
    *,
    claim_text: str,
    span_text: str | None,
    spans: Sequence[Mapping[str, Any]] | None = None,
    relation_type: str | None = None,
    min_overlap: float = 0.25,
) -> FaithfulnessVerdict:
    """Faithfulness of claim to span; high-impact requires pass for promote."""
    tier = classify_risk_tier(relation_type=relation_type, claim_text=claim_text)
    audit_required = tier == "high"
    res = evaluate_assertion_resolvability(list(spans or []))
    resolvable = res.resolvable

    if not claim_text or not claim_text.strip():
        return FaithfulnessVerdict(
            faithful=False,
            reason="empty_claim",
            risk_tier=tier,
            audit_required=audit_required,
            resolvable=resolvable,
            promote_blocked=True,
        )

    if not span_text or not span_text.strip():
        return FaithfulnessVerdict(
            faithful=False,
            reason="missing_span_text",
            risk_tier=tier,
            audit_required=audit_required,
            resolvable=resolvable,
            promote_blocked=True,
        )

    if not resolvable:
        return FaithfulnessVerdict(
            faithful=False,
            reason="span_not_resolvable",
            risk_tier=tier,
            audit_required=audit_required,
            resolvable=False,
            promote_blocked=True,
        )

    overlap = _token_overlap(claim_text, span_text)
    if overlap < min_overlap:
        return FaithfulnessVerdict(
            faithful=False,
            reason=f"low_token_overlap:{overlap:.3f}",
            risk_tier=tier,
            audit_required=audit_required,
            resolvable=True,
            promote_blocked=True,
        )

    # Faithful candidate — high impact still audit_required but not promote_blocked
    # on faithfulness alone; promotion still import-locked globally.
    return FaithfulnessVerdict(
        faithful=True,
        reason=f"token_overlap:{overlap:.3f}",
        risk_tier=tier,
        audit_required=audit_required,
        resolvable=True,
        promote_blocked=False,
    )


def promote_path_allowed(
    *,
    existence: ExistenceVerdict | None,
    faithfulness: FaithfulnessVerdict,
    relation_type: str | None = None,
) -> dict[str, Any]:
    """Whether candidate may enter promote staging (still not import).

    Policy:
    - citation existence alone never unlocks promote for claimy relations
    - high-impact requires faithfulness.faithful and not promote_blocked
    - import_eligible always false
    """
    tier = faithfulness.risk_tier
    if relation_type in CITATION_RELATIONS and relation_type == "CITES":
        # bare citation edge may stage on existence, still no import
        ok = bool(existence and existence.exists)
        return {
            "promote_staging_allowed": ok,
            "reason": "citation_existence" if ok else "citation_missing",
            "risk_tier": tier,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": "existence is not claim faithfulness",
        }

    if not faithfulness.faithful or faithfulness.promote_blocked:
        return {
            "promote_staging_allowed": False,
            "reason": "faithfulness_failed:" + faithfulness.reason,
            "risk_tier": tier,
            "audit_required": faithfulness.audit_required,
            "import_eligible": False,
            "graph_writes_allowed": False,
        }

    return {
        "promote_staging_allowed": True,
        "reason": "faithfulness_ok",
        "risk_tier": tier,
        "audit_required": faithfulness.audit_required,
        "import_eligible": False,
        "graph_writes_allowed": False,
    }


__all__ = [
    "HIGH_IMPACT_RELATIONS",
    "RiskTier",
    "ExistenceVerdict",
    "FaithfulnessVerdict",
    "classify_risk_tier",
    "check_bibliographic_existence",
    "check_claim_faithfulness",
    "promote_path_allowed",
]
