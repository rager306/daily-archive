"""Schema evolution rehearsal for Falkor projection (M203 S06)."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Literal

from research_graph.domain.graph_projection_schema import (
    GraphProjectionSchemaGate,
    SchemaGateResult,
)
from research_graph.domain.ports import ProjectionRequest
from research_graph.domain.universal_kb.contracts import SafetyFlags

RehearsalVerdict = Literal["current_accepted", "migration_placeholder", "rejected"]


@dataclass(frozen=True, slots=True)
class SchemaRehearsalReport:
    current: SchemaGateResult
    next_version: SchemaGateResult
    verdict: RehearsalVerdict
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        self.current.assert_no_write()
        self.next_version.assert_no_write()

    def assert_no_write(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "current": self.current.to_dict(),
            "next_version": self.next_version.to_dict(),
            "verdict": self.verdict,
            "safety_flags": self.safety_flags.to_dict(),
            "diagnostics": list(self.diagnostics),
        }


def rehearse_schema_evolution(
    request: ProjectionRequest,
    *,
    next_schema_version: str = "graph-projection.v0-next",
    gate: GraphProjectionSchemaGate | None = None,
) -> SchemaRehearsalReport:
    """Run schema gate for current and next versions without executing migrations."""
    request.candidate_packet.assert_no_write()
    schema_gate = gate or GraphProjectionSchemaGate()
    current = schema_gate.validate(request)
    next_request = ProjectionRequest(
        candidate_packet=request.candidate_packet,
        schema_version=next_schema_version,
    )
    next_result = schema_gate.validate(next_request)
    if current.accepted and next_result.migration_required:
        verdict: RehearsalVerdict = "migration_placeholder"
    elif current.accepted:
        verdict = "current_accepted"
    else:
        verdict = "rejected"
    report = SchemaRehearsalReport(
        current=current,
        next_version=next_result,
        verdict=verdict,
        diagnostics=(
            "schema_rehearsal_no_execution",
            f"current_accepted:{current.accepted}",
            f"next_migration_required:{next_result.migration_required}",
        ),
    )
    report.assert_no_write()
    return report


__all__ = ["RehearsalVerdict", "SchemaRehearsalReport", "rehearse_schema_evolution"]
