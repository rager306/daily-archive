"""governor.domain — Pure domain logic for trajectory checking.

No IO, no filesystem access. Types and verdict logic only.
"""

from governor.domain.artifacts import ArtifactField, ArtifactViolation
from governor.domain.checks import CheckResult, CheckSeverity, CheckStatus
from governor.domain.retention import RetentionPolicy
from governor.domain.trajectory import (
    DimensionStatus,
    DriftFlag,
    TrajectoryReport,
    TrajectoryVerdict,
)

__all__ = [
    "ArtifactField",
    "ArtifactViolation",
    "CheckResult",
    "CheckSeverity",
    "CheckStatus",
    "DimensionStatus",
    "DriftFlag",
    "RetentionPolicy",
    "TrajectoryReport",
    "TrajectoryVerdict",
]
