"""No-write Falkor backend verdict package for M205 prerequisites (M203 S08)."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Literal

from research_graph.application.graph.falkor_capability import FalkorCapabilityReport
from research_graph.application.graph.falkor_parity import ParityReport
from research_graph.application.graph.falkor_projection_adequacy import AdequacyReport
from research_graph.application.graph.falkor_query_plans import SeedLineagePlanBundle
from research_graph.application.graph.falkor_schema_rehearsal import SchemaRehearsalReport
from research_graph.domain.universal_kb.contracts import SafetyFlags

BackendVerdict = Literal["proceed", "repair", "reject"]

CONTROLLED_WRITE_PILOT_PREREQUISITES: tuple[str, ...] = (
    "write_driver_falkordb_or_redis_protocol",
    "GraphDBPort_adapter_implementing_upsert_scientific_kg",
    "explicit_graph_writes_allowed_authorization",
    "schema_migration_execution_path_beyond_placeholder",
    "transaction_execute_and_commit_phases_enabled",
    "staged_validation_evidence_package_for_write_pilot",
)


@dataclass(frozen=True, slots=True)
class NoWriteBackendVerdict:
    verdict: BackendVerdict
    reasons: tuple[str, ...]
    controlled_write_pilot_prerequisites: tuple[str, ...] = CONTROLLED_WRITE_PILOT_PREREQUISITES
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    evidence: dict[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()

    def assert_no_write(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "verdict": self.verdict,
            "reasons": list(self.reasons),
            "controlled_write_pilot_prerequisites": list(self.controlled_write_pilot_prerequisites),
            "safety_flags": self.safety_flags.to_dict(),
            "evidence": dict(self.evidence),
        }


def decide_no_write_backend_verdict(
    *,
    capability: FalkorCapabilityReport,
    adequacy: AdequacyReport,
    parity: ParityReport,
    schema: SchemaRehearsalReport,
    queries: SeedLineagePlanBundle,
) -> NoWriteBackendVerdict:
    """Aggregate S01–S07 evidence into proceed/repair/reject for M205 handoff."""
    for item in (capability, adequacy, parity, schema, queries):
        item.assert_no_write()

    reasons: list[str] = []
    if not capability.supported_cypher():
        reasons.append("no_supported_cypher")
    if capability.blocked_cypher() and "CREATE" not in capability.blocked_cypher():
        reasons.append("writes_not_blocked")
    if adequacy.verdict != "sufficient":
        reasons.append(f"adequacy:{adequacy.verdict}")
    if parity.verdict != "match":
        reasons.append("parity_mismatch")
    if schema.verdict == "rejected":
        reasons.append("schema_rejected")
    if not queries.o1_seed.validated:
        reasons.append("o1_seed_invalid")
    if not queries.o2_lineage.validated:
        reasons.append("o2_lineage_invalid")

    hard = {
        "no_supported_cypher",
        "writes_not_blocked",
        "schema_rejected",
        "adequacy:insufficient",
    }
    if any(r in hard or r.startswith("adequacy:insufficient") for r in reasons):
        verdict: BackendVerdict = "reject"
    elif reasons:
        verdict = "repair"
    else:
        verdict = "proceed"
        reasons.append("falkor_no_write_surface_ready")
        reasons.append("controlled_write_adapter_still_required")

    report = NoWriteBackendVerdict(
        verdict=verdict,
        reasons=tuple(reasons),
        evidence={
            "capability_supported_count": len(capability.supported_cypher()),
            "adequacy": adequacy.verdict,
            "parity": parity.verdict,
            "schema": schema.verdict,
            "o1_validated": queries.o1_seed.validated,
            "o2_validated": queries.o2_lineage.validated,
            "plan_fingerprint": queries.base_plan.plan_fingerprint,
        },
    )
    report.assert_no_write()
    return report


__all__ = [
    "BackendVerdict",
    "CONTROLLED_WRITE_PILOT_PREREQUISITES",
    "NoWriteBackendVerdict",
    "decide_no_write_backend_verdict",
]
