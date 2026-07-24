"""Pilot write authorization contract (M205 S02).

Separate from production SafetyFlags. Grants disposable pilot write authority
only when bound to an unexpired M204 eligibility packet, environment
prerequisites, operation plan, and rollback plan. Does not mutate SafetyFlags.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import UTC, datetime
from typing import Any, Literal

from research_graph.domain.universal_kb.contracts import SafetyFlags

AuthStatus = Literal["authorized", "denied", "expired"]


@dataclass(frozen=True, slots=True)
class PilotWriteAuthorization:
    """Disposable pilot write authority — not production import eligibility."""

    auth_id: str
    candidate_id: str
    packet_hash: str
    operation_plan_fingerprint: str
    environment_prerequisites: tuple[str, ...]
    rollback_plan: tuple[str, ...]
    expiry_utc: str
    human_approval_token: str
    scope: str
    authorized: bool
    status: AuthStatus
    production_activation: bool = False
    # Production SafetyFlags snapshot must remain fail-closed (unchanged).
    production_safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        # Never mutate production SafetyFlags; assert they stay fail-closed.
        self.production_safety_flags.assert_no_write()
        if self.production_activation:
            raise ValueError("pilot authorization cannot enable production activation")
        if self.authorized and self.status != "authorized":
            raise ValueError("authorized requires status=authorized")
        if self.status == "authorized" and not self.authorized:
            raise ValueError("status=authorized requires authorized=True")
        if self.authorized and not self.human_approval_token.strip():
            raise ValueError("authorized pilot write requires human_approval_token")

    def assert_production_flags_closed(self) -> None:
        self.production_safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "auth_id": self.auth_id,
            "candidate_id": self.candidate_id,
            "packet_hash": self.packet_hash,
            "operation_plan_fingerprint": self.operation_plan_fingerprint,
            "environment_prerequisites": list(self.environment_prerequisites),
            "rollback_plan": list(self.rollback_plan),
            "expiry_utc": self.expiry_utc,
            "human_approval_token": self.human_approval_token,
            "scope": self.scope,
            "authorized": self.authorized,
            "status": self.status,
            "production_activation": self.production_activation,
            "production_safety_flags": self.production_safety_flags.to_dict(),
            "diagnostics": list(self.diagnostics),
        }


def _parse_expiry(expiry_utc: str) -> datetime:
    text = expiry_utc.strip().replace("Z", "+00:00")
    return datetime.fromisoformat(text)


def issue_pilot_write_authorization(
    *,
    auth_id: str,
    candidate_id: str,
    packet_hash: str,
    operation_plan_fingerprint: str,
    environment_prerequisites: tuple[str, ...],
    rollback_plan: tuple[str, ...],
    expiry_utc: str,
    human_approval_token: str,
    scope: str = "controlled_falkor_write_pilot",
    required_prerequisites: tuple[str, ...] = (),
    now: datetime | None = None,
) -> PilotWriteAuthorization:
    """Bind M204 packet fields into pilot write auth; deny if expired/missing."""
    stamp = now or datetime.now(UTC)
    diagnostics: list[str] = []
    status: AuthStatus = "authorized"
    authorized = True

    if not packet_hash.strip() or not operation_plan_fingerprint.strip():
        authorized = False
        status = "denied"
        diagnostics.append("missing_packet_or_plan")
    if not human_approval_token.strip():
        authorized = False
        status = "denied"
        diagnostics.append("missing_human_approval")
    try:
        expiry = _parse_expiry(expiry_utc)
        if expiry.tzinfo is None:
            expiry = expiry.replace(tzinfo=UTC)
        if stamp > expiry:
            authorized = False
            status = "expired"
            diagnostics.append("authorization_expired")
    except ValueError:
        authorized = False
        status = "denied"
        diagnostics.append("invalid_expiry")

    missing = [p for p in required_prerequisites if p not in environment_prerequisites]
    if missing:
        authorized = False
        status = "denied"
        diagnostics.append(f"missing_prerequisites:{len(missing)}")

    if authorized:
        diagnostics.append("pilot_write_authorized_disposable_only")

    auth = PilotWriteAuthorization(
        auth_id=auth_id,
        candidate_id=candidate_id,
        packet_hash=packet_hash,
        operation_plan_fingerprint=operation_plan_fingerprint,
        environment_prerequisites=environment_prerequisites,
        rollback_plan=rollback_plan,
        expiry_utc=expiry_utc,
        human_approval_token=human_approval_token,
        scope=scope,
        authorized=authorized,
        status=status,
        diagnostics=tuple(diagnostics),
    )
    auth.assert_production_flags_closed()
    return auth


__all__ = [
    "AuthStatus",
    "PilotWriteAuthorization",
    "issue_pilot_write_authorization",
]
