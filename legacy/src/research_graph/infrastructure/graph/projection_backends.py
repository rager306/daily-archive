"""Disabled graph projection backend seams for comparison planning.

These adapters implement the no-write projection port without importing backend
SDKs or opening graph connections. They are rehearsal/comparison placeholders,
not production graph persistence.
"""

from __future__ import annotations

from research_graph.domain.ports import (
    ProjectionDiagnostic,
    ProjectionEdgeRef,
    ProjectionNodeRef,
    ProjectionRequest,
    ProjectionResult,
)


class DisabledBackendProjectionAdapter:
    """No-write projection seam for disabled or dry-run backend candidates."""

    def __init__(self, *, backend: str, dry_run: bool = False) -> None:
        self.backend = backend
        self.dry_run = dry_run

    def project(self, request: ProjectionRequest) -> ProjectionResult:
        try:
            if self.dry_run:
                return ProjectionResult(
                    schema_version=request.schema_version,
                    backend=self.backend,
                    node_refs=tuple(
                        ProjectionNodeRef(ref=ref, node_type=_projection_ref_kind(ref, default="node"))
                        for ref in request.candidate_packet.graph_node_refs
                    ),
                    edge_refs=tuple(
                        _projection_edge_ref(ref) for ref in request.candidate_packet.graph_edge_refs
                    ),
                    evidence_refs=request.candidate_packet.evidence_refs,
                    provenance_refs=request.candidate_packet.provenance_refs,
                    diagnostics=(
                        ProjectionDiagnostic(
                            code="backend_projection_dry_run",
                            phase=f"{self.backend}_projection",
                        ),
                    ),
                )
            return ProjectionResult(
                schema_version=request.schema_version,
                backend=self.backend,
                diagnostics=(
                    ProjectionDiagnostic(
                        code="backend_projection_disabled",
                        phase=f"{self.backend}_projection",
                    ),
                ),
            )
        except ValueError:
            return ProjectionResult(
                schema_version=request.schema_version,
                backend="disabled_backend",
                diagnostics=(
                    ProjectionDiagnostic(
                        code="backend_projection_configuration_invalid",
                        phase="backend_projection",
                    ),
                ),
            )


class DisabledLadybugProjectionAdapter(DisabledBackendProjectionAdapter):
    """Disabled LadybugDB projection seam; never imports or writes LadybugDB."""

    def __init__(self, *, dry_run: bool = False) -> None:
        super().__init__(backend="ladybugdb", dry_run=dry_run)


class DisabledFalkorProjectionAdapter(DisabledBackendProjectionAdapter):
    """Disabled FalkorDB projection seam; never imports or writes FalkorDB."""

    def __init__(self, *, dry_run: bool = False) -> None:
        super().__init__(backend="falkordb", dry_run=dry_run)

    def project(self, request: ProjectionRequest) -> ProjectionResult:
        result = super().project(request)
        # Falkor-specific no-write translation metadata (M203 S03).
        # Does not import SDK or execute Cypher; only annotates diagnostics.
        extra = (
            ProjectionDiagnostic(
                code="falkordb_no_write_translation",
                phase="falkordb_projection",
            ),
            ProjectionDiagnostic(
                code=(
                    "falkordb_writes_blocked"
                    if not self.dry_run
                    else "falkordb_dry_run_plan_ready"
                ),
                phase="falkordb_projection",
            ),
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


def _projection_edge_ref(ref: str) -> ProjectionEdgeRef:
    body = ref.removeprefix("edge:")
    if "->" not in body:
        return ProjectionEdgeRef(
            ref=ref,
            edge_type="candidate_edge",
            source_ref="node:unknown:source",
            target_ref="node:unknown:target",
        )
    source, target = body.split("->", 1)
    source_ref = source if source.startswith("node:") else f"node:{source}"
    target_ref = target if target.startswith("node:") else f"node:{target}"
    return ProjectionEdgeRef(
        ref=ref,
        edge_type=(
            f"{_projection_ref_kind(source_ref, default='source')}_to_"
            f"{_projection_ref_kind(target_ref, default='target')}"
        ),
        source_ref=source_ref,
        target_ref=target_ref,
    )


def _projection_ref_kind(ref: str, *, default: str) -> str:
    parts = ref.split(":")
    if len(parts) >= 2 and parts[1]:
        return parts[1]
    return default


__all__ = [
    "DisabledBackendProjectionAdapter",
    "DisabledFalkorProjectionAdapter",
    "DisabledLadybugProjectionAdapter",
]
