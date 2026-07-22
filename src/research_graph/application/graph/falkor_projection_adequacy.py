"""Projection port adequacy for Falkor no-write planning (M203 S02).

Compares KnowledgeGraphProjectionPort request/result surface to the Falkor
capability matrix. Does not change GraphDBPort or authorize writes.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Literal

from research_graph.application.graph.falkor_capability import (
    FalkorCapabilityReport,
    matrix_as_mapping,
    probe_falkor_capabilities,
)
from research_graph.domain.ports import ProjectionRequest, ProjectionResult
from research_graph.domain.universal_kb.contracts import SafetyFlags

AdequacyVerdict = Literal["sufficient", "insufficient", "adr_required"]

# Fields the no-write projection port must carry for Falkor planning.
REQUIRED_REQUEST_ATTRS = (
    "candidate_packet",
    "schema_version",
)
REQUIRED_PACKET_ATTRS = (
    "candidate_id",
    "graph_node_refs",
    "graph_edge_refs",
    "evidence_refs",
    "provenance_refs",
    "schema_version",
)
REQUIRED_RESULT_ATTRS = (
    "schema_version",
    "backend",
    "node_refs",
    "edge_refs",
    "evidence_refs",
    "provenance_refs",
    "diagnostics",
    "safety_flags",
)
# Read clauses required for future O1/O2 query plans.
REQUIRED_READ_CLAUSES = ("MATCH", "RETURN", "WHERE", "WITH", "LIMIT")
# Write clauses that must remain blocked (not executed via projection port).
REQUIRED_BLOCKED_WRITES = ("CREATE", "MERGE", "DELETE", "SET")


@dataclass(frozen=True, slots=True)
class AdequacyFinding:
    code: str
    detail: str
    severity: str = "info"

    def to_dict(self) -> dict[str, str]:
        return {"code": self.code, "detail": self.detail, "severity": self.severity}


@dataclass(frozen=True, slots=True)
class AdequacyReport:
    """Whether ProjectionRequest/Result adequately map Falkor capabilities."""

    verdict: AdequacyVerdict
    findings: tuple[AdequacyFinding, ...] = ()
    required_read_clauses: tuple[str, ...] = REQUIRED_READ_CLAUSES
    blocked_write_clauses: tuple[str, ...] = REQUIRED_BLOCKED_WRITES
    projection_port: str = "KnowledgeGraphProjectionPort"
    graphdb_port_required: bool = False
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.graphdb_port_required and self.verdict == "sufficient":
            raise ValueError("sufficient verdict cannot require GraphDBPort change")

    def assert_no_write(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "verdict": self.verdict,
            "findings": [f.to_dict() for f in self.findings],
            "required_read_clauses": list(self.required_read_clauses),
            "blocked_write_clauses": list(self.blocked_write_clauses),
            "projection_port": self.projection_port,
            "graphdb_port_required": self.graphdb_port_required,
            "safety_flags": self.safety_flags.to_dict(),
        }


def assess_projection_port_adequacy(
    request: ProjectionRequest,
    *,
    capability: FalkorCapabilityReport | None = None,
    sample_result: ProjectionResult | None = None,
) -> AdequacyReport:
    """Assess ProjectionRequest/Result vs Falkor capability matrix."""
    request.candidate_packet.assert_no_write()
    cap = capability or probe_falkor_capabilities()
    matrix = matrix_as_mapping(cap)
    findings: list[AdequacyFinding] = []

    for attr in REQUIRED_REQUEST_ATTRS:
        if not hasattr(request, attr):
            findings.append(
                AdequacyFinding(
                    code="missing_request_attr",
                    detail=attr,
                    severity="error",
                )
            )
    packet = request.candidate_packet
    for attr in REQUIRED_PACKET_ATTRS:
        if not hasattr(packet, attr):
            findings.append(
                AdequacyFinding(
                    code="missing_packet_attr",
                    detail=attr,
                    severity="error",
                )
            )

    if sample_result is not None:
        sample_result.assert_no_write()
        for attr in REQUIRED_RESULT_ATTRS:
            if not hasattr(sample_result, attr):
                findings.append(
                    AdequacyFinding(
                        code="missing_result_attr",
                        detail=attr,
                        severity="error",
                    )
                )
        if sample_result.backend not in {"falkordb", "networkx", "ladybugdb", "disabled_backend"}:
            findings.append(
                AdequacyFinding(
                    code="unexpected_backend",
                    detail=sample_result.backend,
                    severity="warning",
                )
            )

    missing_reads = [c for c in REQUIRED_READ_CLAUSES if matrix.get(c) != "supported"]
    for clause in missing_reads:
        findings.append(
            AdequacyFinding(
                code="missing_read_capability",
                detail=clause,
                severity="error",
            )
        )

    unblocked_writes = [c for c in REQUIRED_BLOCKED_WRITES if matrix.get(c) != "blocked"]
    for clause in unblocked_writes:
        findings.append(
            AdequacyFinding(
                code="write_clause_not_blocked",
                detail=clause,
                severity="error",
            )
        )

    # GraphDBPort is intentionally not required for no-write projection.
    findings.append(
        AdequacyFinding(
            code="graphdb_port_not_required_for_no_write",
            detail="KnowledgeGraphProjectionPort is the no-write boundary",
            severity="info",
        )
    )

    errors = [f for f in findings if f.severity == "error"]
    if errors:
        # If core projection attrs missing → insufficient; if dialect gap → adr_required
        if any(f.code.startswith("missing_") for f in errors):
            verdict: AdequacyVerdict = "insufficient"
        elif any(f.code == "missing_read_capability" for f in errors):
            verdict = "adr_required"
        else:
            verdict = "insufficient"
    else:
        verdict = "sufficient"

    report = AdequacyReport(
        verdict=verdict,
        findings=tuple(findings),
        graphdb_port_required=False,
    )
    report.assert_no_write()
    return report


__all__ = [
    "AdequacyFinding",
    "AdequacyReport",
    "AdequacyVerdict",
    "REQUIRED_BLOCKED_WRITES",
    "REQUIRED_READ_CLAUSES",
    "assess_projection_port_adequacy",
]
