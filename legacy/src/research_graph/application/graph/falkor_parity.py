"""NetworkX vs Falkor no-write projection parity (M203 S04)."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Literal

from research_graph.domain.ports import (
    KnowledgeGraphProjectionPort,
    ProjectionRequest,
    ProjectionResult,
)
from research_graph.domain.universal_kb.contracts import SafetyFlags

ParityVerdict = Literal["match", "mismatch"]


@dataclass(frozen=True, slots=True)
class ParityReport:
    """Compare refs projected by two no-write adapters for one candidate."""

    verdict: ParityVerdict
    networkx_backend: str
    falkor_backend: str
    shared_node_refs: tuple[str, ...]
    shared_edge_refs: tuple[str, ...]
    only_networkx_nodes: tuple[str, ...]
    only_falkor_nodes: tuple[str, ...]
    only_networkx_edges: tuple[str, ...]
    only_falkor_edges: tuple[str, ...]
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()

    def assert_no_write(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "verdict": self.verdict,
            "networkx_backend": self.networkx_backend,
            "falkor_backend": self.falkor_backend,
            "shared_node_refs": list(self.shared_node_refs),
            "shared_edge_refs": list(self.shared_edge_refs),
            "only_networkx_nodes": list(self.only_networkx_nodes),
            "only_falkor_nodes": list(self.only_falkor_nodes),
            "only_networkx_edges": list(self.only_networkx_edges),
            "only_falkor_edges": list(self.only_falkor_edges),
            "safety_flags": self.safety_flags.to_dict(),
            "diagnostics": list(self.diagnostics),
        }


def _node_refs(result: ProjectionResult) -> set[str]:
    return {n.ref for n in result.node_refs}


def _edge_refs(result: ProjectionResult) -> set[str]:
    return {e.ref for e in result.edge_refs}


def compare_projection_parity(
    request: ProjectionRequest,
    *,
    networkx_adapter: KnowledgeGraphProjectionPort,
    falkor_adapter: KnowledgeGraphProjectionPort,
) -> ParityReport:
    """Compare NetworkX and Falkor no-write projections for one candidate packet."""
    request.candidate_packet.assert_no_write()
    nx_result = networkx_adapter.project(request)
    fk_result = falkor_adapter.project(request)
    nx_result.assert_no_write()
    fk_result.assert_no_write()

    nx_nodes, fk_nodes = _node_refs(nx_result), _node_refs(fk_result)
    nx_edges, fk_edges = _edge_refs(nx_result), _edge_refs(fk_result)
    shared_nodes = tuple(sorted(nx_nodes & fk_nodes))
    shared_edges = tuple(sorted(nx_edges & fk_edges))
    only_nx_n = tuple(sorted(nx_nodes - fk_nodes))
    only_fk_n = tuple(sorted(fk_nodes - nx_nodes))
    only_nx_e = tuple(sorted(nx_edges - fk_edges))
    only_fk_e = tuple(sorted(fk_edges - nx_edges))
    match = not (only_nx_n or only_fk_n or only_nx_e or only_fk_e)
    return ParityReport(
        verdict="match" if match else "mismatch",
        networkx_backend=nx_result.backend,
        falkor_backend=fk_result.backend,
        shared_node_refs=shared_nodes,
        shared_edge_refs=shared_edges,
        only_networkx_nodes=only_nx_n,
        only_falkor_nodes=only_fk_n,
        only_networkx_edges=only_nx_e,
        only_falkor_edges=only_fk_e,
        diagnostics=(
            "parity_metadata_refs_only",
            f"verdict:{'match' if match else 'mismatch'}",
        ),
    )


__all__ = ["ParityReport", "ParityVerdict", "compare_projection_parity"]
