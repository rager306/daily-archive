"""Check domain types — rule definitions and results."""

from dataclasses import dataclass, field
from enum import Enum
from typing import Optional


class CheckSeverity(str, Enum):
    HIGH = "high"  # Blocking — verdict becomes "blocked"
    MEDIUM = "medium"  # Drift risk — verdict becomes "drift_risk"


class CheckStatus(str, Enum):
    CLEAR = "clear"
    VIOLATION = "violation"


@dataclass(frozen=True)
class CheckRule:
    """A rule loaded from YAML data."""

    id: str
    anchor: str
    severity: str  # "high" or "medium"
    scan: str  # Directory or file to scan
    pattern: str  # Regex pattern
    rationale: str
    filename_filter: Optional[str] = None
    path_prefix_include: Optional[str] = None
    path_prefix_exclude: Optional[str] = None
    section_filter: Optional[str] = None
    suffix_allowlist: Optional[list[str]] = None


@dataclass
class CheckResult:
    """Result of running a check rule."""

    rule: CheckRule
    status: CheckStatus
    violations: list = field(default_factory=list)

    @property
    def is_blocking(self) -> bool:
        return self.status == CheckStatus.VIOLATION and self.rule.severity == "high"
