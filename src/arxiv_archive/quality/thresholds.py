"""Severity thresholds for local maintainability diagnostics.

These thresholds are intentionally advisory. They annotate riskratchet scores for
operator visibility but must not decide process pass/fail state.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

SeverityBand = Literal["low", "medium", "high", "critical"]


@dataclass(frozen=True)
class MaintainabilityThresholds:
    """Advisory score boundaries for maintainability risk."""

    medium: float = 25.0
    high: float = 50.0
    critical: float = 75.0

    def severity_for_score(self, score: float) -> SeverityBand:
        """Return the advisory severity band for a risk score."""
        if score >= self.critical:
            return "critical"
        if score >= self.high:
            return "high"
        if score >= self.medium:
            return "medium"
        return "low"

    def as_dict(self) -> dict[str, float]:
        """Return a JSON-native threshold payload."""
        return {"medium": self.medium, "high": self.high, "critical": self.critical}


DEFAULT_THRESHOLDS = MaintainabilityThresholds()


def severity_for_score(score: float, thresholds: MaintainabilityThresholds = DEFAULT_THRESHOLDS) -> SeverityBand:
    """Classify a score with the default advisory severity bands."""
    return thresholds.severity_for_score(score)


__all__ = ["DEFAULT_THRESHOLDS", "MaintainabilityThresholds", "SeverityBand", "severity_for_score"]
