"""Explicit promotion boundary composition (M204).

Composes existing review post-check, import-boundary rehearsal, schema gate, and
readiness handoff *results* into pilot eligibility decisions. Does not
duplicate SafetyFlags, does not set import_eligible=true, does not authorize
graph writes, and does not call GraphDBPort.
"""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from datetime import UTC, datetime, timedelta
from typing import Any, Literal

from research_graph.domain.graph_projection_schema import SchemaGateResult
from research_graph.domain.universal_kb.contracts import SafetyFlags

SeamName = Literal[
    "review_post_check",
    "import_boundary",
    "schema_gate",
    "readiness_handoff",
]
GapSeverity = Literal["info", "warning", "error", "blocker"]
PilotDecision = Literal["eligible", "denied"]
PromotionVerdict = Literal["pilot_eligible", "denied", "repair"]


@dataclass(frozen=True, slots=True)
class SeamObservation:
    """One existing seam's metadata-only observation (no corpus payload)."""

    name: SeamName
    passed: bool
    codes: tuple[str, ...] = ()
    detail: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "passed": self.passed,
            "codes": list(self.codes),
            "detail": self.detail,
        }


@dataclass(frozen=True, slots=True)
class BoundaryGap:
    seam: SeamName
    code: str
    severity: GapSeverity
    detail: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "seam": self.seam,
            "code": self.code,
            "severity": self.severity,
            "detail": self.detail,
        }


@dataclass(frozen=True, slots=True)
class BoundaryGapReport:
    """Typed gap report after traversing existing promotion-related seams."""

    candidate_id: str
    seams: tuple[SeamObservation, ...]
    gaps: tuple[BoundaryGap, ...]
    review_post_check_first: bool = True
    import_eligible: bool = False
    graph_write_allowed: bool = False
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.import_eligible or self.graph_write_allowed:
            raise ValueError(
                "gap report cannot grant import eligibility or graph write authority"
            )
        if not self.review_post_check_first:
            raise ValueError("authoritative review post-check must run first")

    def assert_no_write(self) -> None:
        self.safety_flags.assert_no_write()

    @property
    def has_blockers(self) -> bool:
        return any(g.severity in {"error", "blocker"} for g in self.gaps)

    def to_dict(self) -> dict[str, Any]:
        return {
            "candidate_id": self.candidate_id,
            "seams": [s.to_dict() for s in self.seams],
            "gaps": [g.to_dict() for g in self.gaps],
            "review_post_check_first": self.review_post_check_first,
            "import_eligible": self.import_eligible,
            "graph_write_allowed": self.graph_write_allowed,
            "safety_flags": self.safety_flags.to_dict(),
            "diagnostics": list(self.diagnostics),
        }


@dataclass(frozen=True, slots=True)
class PilotEligibilityDecision:
    """Application-owned pilot eligibility — not import eligibility or write auth."""

    candidate_id: str
    decision: PilotDecision
    verdict: PromotionVerdict
    reasons: tuple[str, ...]
    gap_report: BoundaryGapReport
    pilot_eligible: bool
    import_eligible: bool = False
    graph_write_allowed: bool = False
    persistence_authority: bool = False
    graph_adapter_invocations: int = 0
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        self.gap_report.assert_no_write()
        if self.import_eligible or self.graph_write_allowed or self.persistence_authority:
            raise ValueError(
                "pilot eligibility must not grant import, write, or persistence authority"
            )
        if self.graph_adapter_invocations != 0:
            raise ValueError("no-write promotion path forbids graph adapter invocations")
        if self.pilot_eligible and self.decision != "eligible":
            raise ValueError("pilot_eligible requires decision=eligible")
        if self.decision == "eligible" and not self.pilot_eligible:
            raise ValueError("decision=eligible requires pilot_eligible=True")

    def assert_no_write(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "candidate_id": self.candidate_id,
            "decision": self.decision,
            "verdict": self.verdict,
            "reasons": list(self.reasons),
            "pilot_eligible": self.pilot_eligible,
            "import_eligible": self.import_eligible,
            "graph_write_allowed": self.graph_write_allowed,
            "persistence_authority": self.persistence_authority,
            "graph_adapter_invocations": self.graph_adapter_invocations,
            "gap_report": self.gap_report.to_dict(),
            "safety_flags": self.safety_flags.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class PilotApprovalPacket:
    """Operator packet for M205 controlled write pilot (metadata only)."""

    candidate_id: str
    packet_hash: str
    environment_prerequisites: tuple[str, ...]
    operation_plan_fingerprint: str
    rollback_plan: tuple[str, ...]
    expiry_utc: str
    proposed_scope: str
    risks: tuple[str, ...]
    pilot_eligible: bool
    import_eligible: bool = False
    graph_write_allowed: bool = False
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.import_eligible or self.graph_write_allowed:
            raise ValueError("approval packet cannot authorize import or writes")
        if not self.packet_hash or not self.expiry_utc:
            raise ValueError("packet_hash and expiry_utc required")

    def assert_no_write(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "candidate_id": self.candidate_id,
            "packet_hash": self.packet_hash,
            "environment_prerequisites": list(self.environment_prerequisites),
            "operation_plan_fingerprint": self.operation_plan_fingerprint,
            "rollback_plan": list(self.rollback_plan),
            "expiry_utc": self.expiry_utc,
            "proposed_scope": self.proposed_scope,
            "risks": list(self.risks),
            "pilot_eligible": self.pilot_eligible,
            "import_eligible": self.import_eligible,
            "graph_write_allowed": self.graph_write_allowed,
            "safety_flags": self.safety_flags.to_dict(),
            "diagnostics": list(self.diagnostics),
        }


# Default M205 environment prerequisites (not authorization).
DEFAULT_ENV_PREREQUISITES: tuple[str, ...] = (
    "graph_writes_allowed_explicit_true_in_future_milestone",
    "falkordb_write_driver_available",
    "GraphDBPort_adapter_path_m205_only",
    "staged_validation_evidence_package",
    "operator_approval_packet_unexpired",
)

DEFAULT_ROLLBACK: tuple[str, ...] = (
    "abort_before_commit_if_validation_fails",
    "no_partial_trusted_fact_promotion",
    "retain_candidate_evidence_only",
    "reset_graph_writes_allowed_false",
)


def observe_review_post_check(
    *,
    completed: bool,
    diagnostics: Sequence[str] = (),
    require_completed_review: bool = True,
) -> SeamObservation:
    """Map review artifact post-check outcome (authoritative first seam)."""
    codes = tuple(str(c) for c in diagnostics)
    if require_completed_review and not completed:
        return SeamObservation(
            name="review_post_check",
            passed=False,
            codes=codes + ("review_incomplete",),
            detail="require_completed_review not satisfied",
        )
    if diagnostics:
        # soft diagnostics may still pass if completed
        return SeamObservation(
            name="review_post_check",
            passed=completed,
            codes=codes or ("review_diagnostics_present",),
            detail="review post-check with diagnostics",
        )
    return SeamObservation(
        name="review_post_check",
        passed=True,
        codes=("review_post_check_ok",),
        detail="authoritative review post-check passed",
    )


def observe_import_boundary(
    *,
    valid_rehearsal: bool,
    accepted_count: int = 0,
    import_eligible_any: bool = False,
    refusal_codes: Sequence[str] = (),
) -> SeamObservation:
    """Map import-boundary rehearsal validation (negative/no-write expected)."""
    codes = list(refusal_codes)
    # Pilot path expects boundary valid but NOT import-eligible trusted acceptance.
    if import_eligible_any:
        codes.append("import_eligible_true_forbidden_on_promotion_boundary")
        return SeamObservation(
            name="import_boundary",
            passed=False,
            codes=tuple(codes),
            detail="import_eligible must remain false at promotion boundary",
        )
    if not valid_rehearsal:
        codes.append("import_boundary_invalid")
        return SeamObservation(
            name="import_boundary",
            passed=False,
            codes=tuple(codes),
            detail="import boundary rehearsal invalid",
        )
    if accepted_count > 0:
        codes.append("trusted_accept_count_nonzero")
        return SeamObservation(
            name="import_boundary",
            passed=False,
            codes=tuple(codes),
            detail="trusted import accepts must be zero on no-write boundary",
        )
    return SeamObservation(
        name="import_boundary",
        passed=True,
        codes=tuple(codes) + ("import_boundary_negative_ok",),
        detail="import boundary rehearsal valid and non-eligible",
    )


def observe_schema_gate(result: SchemaGateResult) -> SeamObservation:
    """Map GraphProjectionSchemaGate result."""
    result.assert_no_write()
    if result.accepted and not result.migration_required:
        return SeamObservation(
            name="schema_gate",
            passed=True,
            codes=tuple(result.diagnostics) + ("schema_gate_ok",),
            detail="schema versions current",
        )
    return SeamObservation(
        name="schema_gate",
        passed=False,
        codes=tuple(result.diagnostics)
        + (("schema_migration_required",) if result.migration_required else ("schema_rejected",)),
        detail="schema gate not accepted for pilot",
    )


def observe_readiness_handoff(
    *,
    readiness_state: str,
    dry_run_only: bool = True,
    graph_write_allowed: bool = False,
    promotion_allowed: bool = False,
    production_import_attempted: bool = False,
) -> SeamObservation:
    """Map ReadinessHandoff-like metadata without importing workflow module."""
    if readiness_state != "diagnostics_only":
        return SeamObservation(
            name="readiness_handoff",
            passed=False,
            codes=("readiness_state_not_diagnostic",),
            detail=f"state={readiness_state}",
        )
    if (
        not dry_run_only
        or graph_write_allowed
        or promotion_allowed
        or production_import_attempted
    ):
        return SeamObservation(
            name="readiness_handoff",
            passed=False,
            codes=("handoff_authority_leak",),
            detail="handoff must remain dry-run diagnostics only",
        )
    return SeamObservation(
        name="readiness_handoff",
        passed=True,
        codes=("readiness_handoff_ok",),
        detail="diagnostics_only handoff",
    )


def trace_promotion_boundary_gaps(
    *,
    candidate_id: str,
    review: SeamObservation,
    import_boundary: SeamObservation,
    schema: SeamObservation,
    handoff: SeamObservation,
) -> BoundaryGapReport:
    """Authoritative order: review post-check first, then other seams."""
    if review.name != "review_post_check":
        raise ValueError("first seam must be review_post_check")
    seams = (review, import_boundary, schema, handoff)
    gaps: list[BoundaryGap] = []
    # Enforce order presence
    order = [s.name for s in seams]
    if order[0] != "review_post_check":
        gaps.append(
            BoundaryGap(
                seam="review_post_check",
                code="review_not_first",
                severity="blocker",
                detail="review post-check must run before decision synthesis",
            )
        )
    for seam in seams:
        if seam.passed:
            continue
        severity: GapSeverity = (
            "blocker" if seam.name == "review_post_check" else "error"
        )
        code = seam.codes[0] if seam.codes else f"{seam.name}_failed"
        gaps.append(
            BoundaryGap(
                seam=seam.name,
                code=code,
                severity=severity,
                detail=seam.detail,
            )
        )
    report = BoundaryGapReport(
        candidate_id=candidate_id,
        seams=seams,
        gaps=tuple(gaps),
        review_post_check_first=True,
        diagnostics=(
            "promotion_boundary_gap_trace",
            "import_eligible_false",
            "graph_write_allowed_false",
        ),
    )
    report.assert_no_write()
    return report


def decide_pilot_eligibility(gap_report: BoundaryGapReport) -> PilotEligibilityDecision:
    """Compose seam gaps into eligible/denied without persistence authority."""
    gap_report.assert_no_write()
    reasons: list[str] = []
    if gap_report.has_blockers:
        reasons.extend(f"{g.seam}:{g.code}" for g in gap_report.gaps)
        decision: PilotDecision = "denied"
        verdict: PromotionVerdict = (
            "repair"
            if any(g.severity == "error" and g.seam != "review_post_check" for g in gap_report.gaps)
            and not any(g.severity == "blocker" for g in gap_report.gaps)
            else "denied"
        )
        if any(g.severity == "blocker" for g in gap_report.gaps):
            verdict = "denied"
        elif any(g.severity == "error" for g in gap_report.gaps):
            verdict = "repair"
        pilot = False
    else:
        decision = "eligible"
        verdict = "pilot_eligible"
        pilot = True
        reasons.append("all_seams_passed")
        reasons.append("pilot_eligible_not_import_eligible")
        reasons.append("no_write_authority_granted")

    result = PilotEligibilityDecision(
        candidate_id=gap_report.candidate_id,
        decision=decision,
        verdict=verdict,
        reasons=tuple(reasons),
        gap_report=gap_report,
        pilot_eligible=pilot,
        graph_adapter_invocations=0,
    )
    result.assert_no_write()
    return result


def build_pilot_approval_packet(
    decision: PilotEligibilityDecision,
    *,
    operation_plan_fingerprint: str,
    proposed_scope: str = "m205_controlled_falkor_write_pilot_single_candidate",
    ttl_hours: int = 72,
    now: datetime | None = None,
) -> PilotApprovalPacket:
    """Build operator approval packet with hash, rollback, expiry (no write auth)."""
    decision.assert_no_write()
    if not decision.pilot_eligible:
        raise ValueError("approval packet requires pilot_eligible decision")
    stamp = now or datetime.now(UTC)
    expiry = stamp + timedelta(hours=ttl_hours)
    payload = {
        "candidate_id": decision.candidate_id,
        "verdict": decision.verdict,
        "reasons": list(decision.reasons),
        "operation_plan_fingerprint": operation_plan_fingerprint,
        "proposed_scope": proposed_scope,
        "expiry_utc": expiry.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "import_eligible": False,
        "graph_write_allowed": False,
    }
    digest = hashlib.sha256(
        json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()
    packet = PilotApprovalPacket(
        candidate_id=decision.candidate_id,
        packet_hash=digest,
        environment_prerequisites=DEFAULT_ENV_PREREQUISITES,
        operation_plan_fingerprint=operation_plan_fingerprint,
        rollback_plan=DEFAULT_ROLLBACK,
        expiry_utc=payload["expiry_utc"],
        proposed_scope=proposed_scope,
        risks=(
            "write_pilot_may_mutate_graph_if_m205_authorizes",
            "rollback_required_on_validation_failure",
            "pilot_eligible_does_not_imply_import_eligible",
        ),
        pilot_eligible=True,
        diagnostics=("approval_packet_metadata_only", "m205_gate_input"),
    )
    packet.assert_no_write()
    return packet


def compact_operator_packet(packet: PilotApprovalPacket) -> Mapping[str, Any]:
    """Compact operator-facing view (S06)."""
    packet.assert_no_write()
    return {
        "candidate_id": packet.candidate_id,
        "packet_hash": packet.packet_hash,
        "expiry_utc": packet.expiry_utc,
        "proposed_scope": packet.proposed_scope,
        "risks": list(packet.risks),
        "rollback_plan": list(packet.rollback_plan),
        "environment_prerequisites": list(packet.environment_prerequisites),
        "operation_plan_fingerprint": packet.operation_plan_fingerprint,
        "pilot_eligible": packet.pilot_eligible,
        "import_eligible": False,
        "graph_write_allowed": False,
    }


__all__ = [
    "DEFAULT_ENV_PREREQUISITES",
    "DEFAULT_ROLLBACK",
    "BoundaryGap",
    "BoundaryGapReport",
    "PilotApprovalPacket",
    "PilotDecision",
    "PilotEligibilityDecision",
    "PromotionVerdict",
    "SeamName",
    "SeamObservation",
    "build_pilot_approval_packet",
    "compact_operator_packet",
    "decide_pilot_eligibility",
    "observe_import_boundary",
    "observe_readiness_handoff",
    "observe_review_post_check",
    "observe_schema_gate",
    "trace_promotion_boundary_gaps",
]
