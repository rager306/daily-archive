"""EvidenceAssertion staging model (M279 E4.1–E4.2).

Staging-only graph candidate: subject/predicate/object + grounded spans +
epistemic status. Never authorizes import or graph writes.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Final, Literal

SCHEMA_VERSION: Final[str] = "evidence-assertion.v1"

EpistemicStatus = Literal[
    "candidate",
    "supported",
    "contested",
    "retracted",
    "unknown",
]
AuditStatus = Literal[
    "pending",
    "passed",
    "failed",
    "skipped",
    "required",
]


@dataclass(frozen=True, slots=True)
class EvidenceAssertion:
    """One staged assertion with optional grounding and audit state."""

    schema_version: str
    assertion_id: str
    paper_id: str
    subject: str
    predicate: str
    object: str
    spans: tuple[dict[str, Any], ...]
    risk_tier: str
    epistemic_status: EpistemicStatus
    audit_status: AuditStatus
    faithfulness_reason: str = ""
    resolvable: bool = False
    intent_hash: str | None = None
    constraints_hash: str | None = None
    confidence: float | None = None
    metric: str | None = None
    conditions: str | None = None
    provenance: dict[str, str] = field(default_factory=dict)
    validation_warnings: tuple[str, ...] = ()
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("EvidenceAssertion cannot authorize import/writes")
        if self.schema_version != SCHEMA_VERSION:
            raise ValueError(f"unsupported assertion schema: {self.schema_version}")

    def identity_key(self) -> str:
        """Stable identity for silent-upgrade detection (no span/confidence)."""
        return "|".join(
            [
                self.paper_id,
                self.subject.strip().casefold(),
                self.predicate.strip().upper(),
                self.object.strip().casefold(),
            ]
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "assertion_id": self.assertion_id,
            "paper_id": self.paper_id,
            "subject": self.subject,
            "predicate": self.predicate,
            "object": self.object,
            "spans": [dict(s) for s in self.spans],
            "risk_tier": self.risk_tier,
            "epistemic_status": self.epistemic_status,
            "audit_status": self.audit_status,
            "faithfulness_reason": self.faithfulness_reason,
            "resolvable": self.resolvable,
            "intent_hash": self.intent_hash,
            "constraints_hash": self.constraints_hash,
            "confidence": self.confidence,
            "metric": self.metric,
            "conditions": self.conditions,
            "provenance": dict(self.provenance),
            "validation_warnings": list(self.validation_warnings),
            "identity_key": self.identity_key(),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": "Staging-only EvidenceAssertion. Never import authority.",
        }


__all__ = [
    "SCHEMA_VERSION",
    "EpistemicStatus",
    "AuditStatus",
    "EvidenceAssertion",
]
