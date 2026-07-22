"""Application helpers for Falkor dry-run translation (M203 S03).

Composes operation plans with any KnowledgeGraphProjectionPort result.
Does not import infrastructure adapters (onion: application stays pure).
"""

from __future__ import annotations

from research_graph.application.graph.falkor_operation_plan import (
    FalkorOperationPlan,
    attach_plan_to_result,
    build_falkor_operation_plan,
)
from research_graph.domain.ports import (
    KnowledgeGraphProjectionPort,
    ProjectionRequest,
    ProjectionResult,
)


def project_with_falkor_plan(
    adapter: KnowledgeGraphProjectionPort,
    request: ProjectionRequest,
) -> tuple[ProjectionResult, FalkorOperationPlan]:
    """Project via port then attach Falkor no-write operation plan diagnostics."""
    request.candidate_packet.assert_no_write()
    base = adapter.project(request)
    plan = build_falkor_operation_plan(request)
    return attach_plan_to_result(base, plan), plan


__all__ = ["project_with_falkor_plan"]
