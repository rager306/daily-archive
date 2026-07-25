"""Silent-upgrade detection + pure promotion boundary (M279 E4.2–E4.3).

Never authorizes import. User go remains external.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any

from research_graph.domain.evidence_assertion import EvidenceAssertion


@dataclass(frozen=True, slots=True)
class SilentUpgradeFinding:
    identity_key: str
    changed_fields: tuple[str, ...]
    previous_assertion_id: str
    current_assertion_id: str
    severity: str = "flag"

    def to_dict(self) -> dict[str, Any]:
        return {
            "identity_key": self.identity_key,
            "changed_fields": list(self.changed_fields),
            "previous_assertion_id": self.previous_assertion_id,
            "current_assertion_id": self.current_assertion_id,
            "severity": self.severity,
            "import_eligible": False,
        }


def detect_silent_upgrades(
    previous: Sequence[EvidenceAssertion] | Sequence[Mapping[str, Any]],
    current: Sequence[EvidenceAssertion] | Sequence[Mapping[str, Any]],
) -> list[SilentUpgradeFinding]:
    """Flag same SPO identity with changed span/confidence/epistemic without audit note."""

    def as_view(item: EvidenceAssertion | Mapping[str, Any]) -> dict[str, Any]:
        if isinstance(item, EvidenceAssertion):
            return item.to_dict()
        return dict(item)

    prev_map: dict[str, dict[str, Any]] = {}
    for item in previous:
        v = as_view(item)
        key = str(v.get("identity_key") or "")
        if key:
            prev_map[key] = v

    findings: list[SilentUpgradeFinding] = []
    watch = ("spans", "confidence", "epistemic_status", "faithfulness_reason", "resolvable")
    for item in current:
        v = as_view(item)
        key = str(v.get("identity_key") or "")
        if not key or key not in prev_map:
            continue
        old = prev_map[key]
        changed: list[str] = []
        for field in watch:
            if old.get(field) != v.get(field):
                changed.append(field)
        if changed:
            findings.append(
                SilentUpgradeFinding(
                    identity_key=key,
                    changed_fields=tuple(changed),
                    previous_assertion_id=str(old.get("assertion_id") or ""),
                    current_assertion_id=str(v.get("assertion_id") or ""),
                )
            )
    return findings


def promotion_boundary_check(
    assertion: EvidenceAssertion | Mapping[str, Any],
    *,
    min_precision: float = 0.85,
    observed_precision: float | None = None,
    user_go: bool = False,
    require_resolvable: bool = True,
    require_supported: bool = True,
) -> dict[str, Any]:
    """Precision-oriented promote gate. import_eligible always false without user_go+checks.

    Even with user_go, this helper only reports staging eligibility for a future
    write path; it never sets import true in this milestone.
    """
    if isinstance(assertion, EvidenceAssertion):
        data = assertion.to_dict()
    else:
        data = dict(assertion)

    reasons: list[str] = []
    if require_resolvable and not data.get("resolvable"):
        reasons.append("not_resolvable")
    if require_supported and data.get("epistemic_status") not in {"supported"}:
        reasons.append(f"epistemic:{data.get('epistemic_status')}")
    if data.get("audit_status") == "failed":
        reasons.append("audit_failed")
    if data.get("risk_tier") == "high" and data.get("audit_status") == "required":
        reasons.append("high_impact_audit_pending")
    if observed_precision is not None and observed_precision < min_precision:
        reasons.append(f"precision_below:{observed_precision}<{min_precision}")
    if not user_go:
        reasons.append("user_go_required")

    staging_ok = not reasons or reasons == ["user_go_required"]
    # Without user_go never claim import; with user_go still false here (D127)
    return {
        "promote_staging_allowed": staging_ok and user_go and not [
            r for r in reasons if r != "user_go_required"
        ],
        "block_reasons": reasons,
        "min_precision": min_precision,
        "observed_precision": observed_precision,
        "user_go": user_go,
        "import_eligible": False,
        "graph_writes_allowed": False,
        "note": "D127: import remains locked until explicit external go beyond this check",
    }


__all__ = [
    "SilentUpgradeFinding",
    "detect_silent_upgrades",
    "promotion_boundary_check",
]
