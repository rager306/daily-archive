"""Trajectory domain types — report, verdict, dimensions."""

from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Optional


class TrajectoryVerdict(str, Enum):
    ON_TRACK = "on_track"
    DRIFT_RISK = "drift_risk"
    BLOCKED = "blocked"


class DimensionStatus(str, Enum):
    IMPLEMENTED = "implemented"
    CRYSTALLIZED = "crystallized"
    SUPERSEDED = "superseded_by_samyama"
    PHASE_A_COMPLETE = "phase_a_complete"
    SKELETON_DONE = "skeleton_done"
    MEASURED = "measured"
    ACTIVE = "active"
    WIRED = "wired"
    PROTOTYPE_DONE = "prototype_done"
    IN_PROGRESS = "in_progress"
    DESIGN_ACCEPTED = "design_accepted"
    NOT_STARTED = "not_started"
    MISSING = "missing"


@dataclass
class Dimension:
    """A project maturity dimension."""

    id: str
    description: str
    status: str
    evidence: list[str] = field(default_factory=list)


@dataclass
class DriftFlag:
    """A drift signal detected by governor."""

    flag: str
    severity: str  # "high" (blocking) | "medium" (drift) | "low" (informational)
    evidence: str = ""
    dimension: Optional[str] = None


@dataclass
class TrajectoryReport:
    """The full trajectory report produced by governor."""

    schema_version: str
    phase: str
    verdict: str
    dimensions: dict[str, Any] = field(default_factory=dict)
    drift_flags: list[dict] = field(default_factory=list)
    reverse_adr_audit_details: dict[str, Any] = field(default_factory=dict)
    next_actions: list[str] = field(default_factory=list)
    milestone_context: dict[str, Any] = field(default_factory=dict)
    # D491 / R141: the effective active-research policy the report actually ran
    # under. Populated only when the authorization DATA validates (graph/import
    # locks retained, no third-party rights or quality pass inferred, historical
    # M513 C3 left pending). An empty list means the C3 process gates are still
    # in force, which is the fail-closed default.
    active_policy: list[dict] = field(default_factory=list)
    # M516-th0oby/S06: the numeric scorecard. Three axes (measurability,
    # declaration synchronisation, feature-gated coverage) with their values and
    # definitions, plus the five governor improvements. Serialized so a session
    # reads the numbers from the report rather than inferring them from prose.
    scorecard: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict:
        return {
            "schema_version": self.schema_version,
            "phase": self.phase,
            "verdict": self.verdict,
            "dimensions": self.dimensions,
            "drift_flags": self.drift_flags,
            "reverse_adr_audit_details": self.reverse_adr_audit_details,
            "next_actions": self.next_actions,
            "milestone_context": self.milestone_context,
            "active_policy": self.active_policy,
            "scorecard": self.scorecard,
        }
