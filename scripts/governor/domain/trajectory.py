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
    severity: str  # "high" or "medium"
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
        }
