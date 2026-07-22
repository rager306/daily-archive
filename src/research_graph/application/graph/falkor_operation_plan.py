"""Falkor-specific no-write operation plan from projection metadata (M203 S03/S05).

Builds planned Cypher-shaped operation descriptors from ProjectionRequest refs.
Never executes Cypher, never imports Falkor SDK, never writes graph state.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Literal

from research_graph.domain.ports import ProjectionRequest, ProjectionResult
from research_graph.domain.universal_kb.contracts import SafetyFlags

OperationKind = Literal["read_match", "read_lineage", "schema_check", "write_blocked"]
PlanPhase = Literal["prepare", "validate", "execute_deferred", "commit_deferred"]


@dataclass(frozen=True, slots=True)
class PlannedOperation:
    """One planned Falkor operation (metadata only)."""

    op_id: str
    kind: OperationKind
    cypher_template: str
    bound_refs: tuple[str, ...] = ()
    executable_now: bool = False
    blocker: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "op_id": self.op_id,
            "kind": self.kind,
            "cypher_template": self.cypher_template,
            "bound_refs": list(self.bound_refs),
            "executable_now": self.executable_now,
            "blocker": self.blocker,
        }


@dataclass(frozen=True, slots=True)
class FalkorOperationPlan:
    """Stable operation plan for a candidate packet (idempotent metadata)."""

    candidate_id: str
    backend: str = "falkordb"
    operations: tuple[PlannedOperation, ...] = ()
    transaction_phases: tuple[PlanPhase, ...] = (
        "prepare",
        "validate",
        "execute_deferred",
        "commit_deferred",
    )
    plan_fingerprint: str = ""
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if any(op.executable_now and op.kind == "write_blocked" for op in self.operations):
            raise ValueError("write_blocked operations cannot be executable_now")
        if any(op.executable_now for op in self.operations):
            raise ValueError("M203 no-write plan forbids executable_now operations")

    def assert_no_write(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "candidate_id": self.candidate_id,
            "backend": self.backend,
            "operations": [op.to_dict() for op in self.operations],
            "transaction_phases": list(self.transaction_phases),
            "plan_fingerprint": self.plan_fingerprint,
            "safety_flags": self.safety_flags.to_dict(),
            "diagnostics": list(self.diagnostics),
        }


def _fingerprint(candidate_id: str, ops: tuple[PlannedOperation, ...]) -> str:
    import hashlib

    payload = candidate_id + "|" + "|".join(f"{o.op_id}:{o.kind}:{o.cypher_template}" for o in ops)
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()[:16]


def build_falkor_operation_plan(request: ProjectionRequest) -> FalkorOperationPlan:
    """Translate candidate refs into a deterministic Falkor no-write plan."""
    request.candidate_packet.assert_no_write()
    packet = request.candidate_packet
    ops: list[PlannedOperation] = []

    for i, node_ref in enumerate(packet.graph_node_refs):
        ops.append(
            PlannedOperation(
                op_id=f"match-node-{i}",
                kind="read_match",
                cypher_template="MATCH (n {ref: $ref}) RETURN n.ref AS ref",
                bound_refs=(node_ref,),
                executable_now=False,
                blocker="no_write_rehearsal_deferred",
            )
        )
    for i, edge_ref in enumerate(packet.graph_edge_refs):
        ops.append(
            PlannedOperation(
                op_id=f"match-edge-{i}",
                kind="read_lineage",
                cypher_template=(
                    "MATCH (a)-[r]->(b) WHERE r.ref = $ref "
                    "RETURN a.ref AS source, b.ref AS target, r.ref AS ref"
                ),
                bound_refs=(edge_ref,),
                executable_now=False,
                blocker="no_write_rehearsal_deferred",
            )
        )
    ops.append(
        PlannedOperation(
            op_id="schema-check",
            kind="schema_check",
            cypher_template="CALL db.indexes() YIELD name RETURN name",
            bound_refs=(request.schema_version,),
            executable_now=False,
            blocker="no_write_rehearsal_deferred",
        )
    )
    ops.append(
        PlannedOperation(
            op_id="write-create-blocked",
            kind="write_blocked",
            cypher_template="CREATE (n) /* blocked */",
            bound_refs=(),
            executable_now=False,
            blocker="graph_writes_not_authorized",
        )
    )

    operations = tuple(ops)
    plan = FalkorOperationPlan(
        candidate_id=packet.candidate_id,
        operations=operations,
        plan_fingerprint=_fingerprint(packet.candidate_id, operations),
        diagnostics=(
            "operation_plan_metadata_only",
            "execute_deferred_not_authorized",
            f"op_count:{len(operations)}",
        ),
    )
    plan.assert_no_write()
    return plan


def plan_diagnostics_for_projection(plan: FalkorOperationPlan) -> tuple[str, ...]:
    """Diagnostic codes attachable to ProjectionResult for dry-run translation."""
    return (
        f"falkor_plan_fingerprint:{plan.plan_fingerprint}",
        f"falkor_plan_ops:{len(plan.operations)}",
        "falkor_writes_blocked",
        *plan.diagnostics,
    )


def attach_plan_to_result(result: ProjectionResult, plan: FalkorOperationPlan) -> ProjectionResult:
    """Return a new ProjectionResult with plan diagnostics (no mutation of writes)."""
    from research_graph.domain.ports import ProjectionDiagnostic

    extra = tuple(
        ProjectionDiagnostic(code=code[:80], phase="falkordb_projection")
        for code in plan_diagnostics_for_projection(plan)
    )
    return ProjectionResult(
        schema_version=result.schema_version,
        backend=result.backend,
        node_refs=result.node_refs,
        edge_refs=result.edge_refs,
        evidence_refs=result.evidence_refs,
        provenance_refs=result.provenance_refs,
        diagnostics=result.diagnostics + extra,
        safety_flags=result.safety_flags,
    )


__all__ = [
    "FalkorOperationPlan",
    "OperationKind",
    "PlanPhase",
    "PlannedOperation",
    "attach_plan_to_result",
    "build_falkor_operation_plan",
    "plan_diagnostics_for_projection",
]
