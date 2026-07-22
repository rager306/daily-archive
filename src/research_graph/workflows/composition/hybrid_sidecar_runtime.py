"""Hybrid sidecar runtime composition (M212).

Injectable ports for GROBID + OpenDataLoader; default is offline/unavailable
unless ports are provided. Never authorizes graph import/writes.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Protocol

from research_graph.application.hybrid_sidecar import (
    HybridCandidatePacket,
    merge_hybrid_sidecar_packets,
)
from research_graph.domain.universal_kb.contracts import SafetyFlags


class GrobidSidecarPort(Protocol):
    def extract_metrics(self, pdf_path: Path, *, paper_id: str) -> dict[str, Any]:
        """Return metrics dict (M055-like) or status=unavailable."""
        ...


class OpenDataLoaderSidecarPort(Protocol):
    def extract_metrics(self, pdf_path: Path, *, paper_id: str) -> dict[str, Any]:
        """Return metrics dict including optional markdown body."""
        ...


@dataclass(frozen=True, slots=True)
class HybridRuntimeRequest:
    paper_id: str
    pdf_path: Path | None = None
    allow_live_services: bool = False  # reserved; live clients not defaulted


@dataclass(frozen=True, slots=True)
class HybridRuntimeResult:
    packet: HybridCandidatePacket
    diagnostics: tuple[str, ...] = ()
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        self.packet.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "packet": self.packet.to_dict(),
            "diagnostics": list(self.diagnostics),
            "safety_flags": self.safety_flags.to_dict(),
        }


def run_hybrid_sidecar_runtime(
    request: HybridRuntimeRequest,
    *,
    grobid: GrobidSidecarPort | None = None,
    opendataloader: OpenDataLoaderSidecarPort | None = None,
) -> HybridRuntimeResult:
    """Run hybrid merge with injected ports (or deferred if missing)."""
    diagnostics: list[str] = []
    g_metrics: dict[str, Any] | None = None
    o_metrics: dict[str, Any] | None = None

    if grobid is None:
        diagnostics.append("grobid_port_not_injected")
        g_metrics = {"status": "unavailable"}
    else:
        if request.pdf_path is None:
            diagnostics.append("grobid_missing_pdf_path")
            g_metrics = {"status": "unavailable"}
        else:
            g_metrics = grobid.extract_metrics(request.pdf_path, paper_id=request.paper_id)
            diagnostics.append("grobid_port_invoked")

    if opendataloader is None:
        diagnostics.append("odl_port_not_injected")
        o_metrics = {"status": "unavailable"}
    else:
        if request.pdf_path is None:
            diagnostics.append("odl_missing_pdf_path")
            o_metrics = {"status": "unavailable"}
        else:
            o_metrics = opendataloader.extract_metrics(
                request.pdf_path, paper_id=request.paper_id
            )
            diagnostics.append("odl_port_invoked")

    body_md = None
    if isinstance(o_metrics, dict) and isinstance(o_metrics.get("markdown"), str):
        body_md = o_metrics["markdown"]

    packet = merge_hybrid_sidecar_packets(
        paper_id=request.paper_id,
        grobid=g_metrics,
        opendataloader=o_metrics,
        body_markdown=body_md,
    )
    diagnostics.extend(packet.diagnostics)
    return HybridRuntimeResult(packet=packet, diagnostics=tuple(diagnostics))


__all__ = [
    "GrobidSidecarPort",
    "HybridRuntimeRequest",
    "HybridRuntimeResult",
    "OpenDataLoaderSidecarPort",
    "run_hybrid_sidecar_runtime",
]
