"""Seed (O1) and lineage (O2) read query plans (M203 S07)."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Literal

from research_graph.application.graph.falkor_operation_plan import (
    FalkorOperationPlan,
    PlannedOperation,
    build_falkor_operation_plan,
)
from research_graph.domain.ports import ProjectionRequest
from research_graph.domain.universal_kb.contracts import SafetyFlags

QueryPlanKind = Literal["O1_seed", "O2_lineage"]


@dataclass(frozen=True, slots=True)
class ReadQueryPlan:
    kind: QueryPlanKind
    operations: tuple[PlannedOperation, ...]
    validated: bool
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if any(op.executable_now for op in self.operations):
            raise ValueError("read query plans must not be executable_now under M203")

    def assert_no_write(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "kind": self.kind,
            "operations": [op.to_dict() for op in self.operations],
            "validated": self.validated,
            "safety_flags": self.safety_flags.to_dict(),
            "diagnostics": list(self.diagnostics),
        }


@dataclass(frozen=True, slots=True)
class SeedLineagePlanBundle:
    candidate_id: str
    o1_seed: ReadQueryPlan
    o2_lineage: ReadQueryPlan
    base_plan: FalkorOperationPlan
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        self.o1_seed.assert_no_write()
        self.o2_lineage.assert_no_write()
        self.base_plan.assert_no_write()

    def assert_no_write(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "candidate_id": self.candidate_id,
            "o1_seed": self.o1_seed.to_dict(),
            "o2_lineage": self.o2_lineage.to_dict(),
            "base_plan_fingerprint": self.base_plan.plan_fingerprint,
            "safety_flags": self.safety_flags.to_dict(),
        }


def build_seed_and_lineage_plans(request: ProjectionRequest) -> SeedLineagePlanBundle:
    """Build O1 seed and O2 lineage read plans from one projected candidate."""
    request.candidate_packet.assert_no_write()
    base = build_falkor_operation_plan(request)
    seed_ops = tuple(op for op in base.operations if op.kind == "read_match")
    lineage_ops = tuple(op for op in base.operations if op.kind == "read_lineage")
    seed = ReadQueryPlan(
        kind="O1_seed",
        operations=seed_ops,
        validated=bool(seed_ops),
        diagnostics=("o1_seed_plan", f"ops:{len(seed_ops)}"),
    )
    lineage = ReadQueryPlan(
        kind="O2_lineage",
        operations=lineage_ops,
        validated=bool(lineage_ops),
        diagnostics=("o2_lineage_plan", f"ops:{len(lineage_ops)}"),
    )
    bundle = SeedLineagePlanBundle(
        candidate_id=request.candidate_packet.candidate_id,
        o1_seed=seed,
        o2_lineage=lineage,
        base_plan=base,
    )
    bundle.assert_no_write()
    return bundle


__all__ = [
    "QueryPlanKind",
    "ReadQueryPlan",
    "SeedLineagePlanBundle",
    "build_seed_and_lineage_plans",
]
