"""Artifact domain types — field checks and violations."""

from dataclasses import dataclass
from typing import Optional


@dataclass(frozen=True)
class ArtifactField:
    """A field to check in an artifact JSON file."""

    name: str
    must_be: object  # Expected value (usually False for gate fields)
    severity: str = "high"  # high = blocking, medium = drift_risk
    anchor: str = ""  # ADR/decision reference


@dataclass(frozen=True)
class ArtifactViolation:
    """A violation found in an artifact file."""

    rule_id: str
    file: str
    line: Optional[int]
    anchor: str
    severity: str
    rationale: str
    matched_text: str = ""

    def to_dict(self) -> dict:
        return {
            "rule_id": self.rule_id,
            "file": self.file,
            "line": self.line,
            "anchor": self.anchor,
            "severity": self.severity,
            "rationale": self.rationale,
            "matched_text": self.matched_text,
        }
