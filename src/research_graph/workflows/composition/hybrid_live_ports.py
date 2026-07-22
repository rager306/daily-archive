"""Composition helper: optional live GROBID/ODL ports for hybrid body path.

Composition-root only (workflows/). Application stays free of docker/HTTP.
Default is offline: no live adapters unless explicitly enabled.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from research_graph.workflows.composition.hybrid_sidecar_runtime import (
    GrobidSidecarPort,
    OpenDataLoaderSidecarPort,
)


@dataclass(frozen=True, slots=True)
class LiveHybridPorts:
    """Resolved live (or missing) hybrid sidecar ports + probe diagnostics."""

    grobid: GrobidSidecarPort | None
    opendataloader: OpenDataLoaderSidecarPort | None
    enabled: bool
    diagnostics: tuple[str, ...]

    @property
    def any_available(self) -> bool:
        return self.grobid is not None or self.opendataloader is not None

    def to_dict(self) -> dict[str, Any]:
        return {
            "enabled": self.enabled,
            "grobid_injected": self.grobid is not None,
            "opendataloader_injected": self.opendataloader is not None,
            "diagnostics": list(self.diagnostics),
        }


def resolve_live_hybrid_ports(
    *,
    enable: bool,
    ensure_containers: bool = True,
) -> LiveHybridPorts:
    """Build live hybrid ports when enable=True; otherwise empty offline bundle.

    Fail-closed: enable=False never touches docker/import. enable=True may probe
    and optionally auto-start GROBID; missing services still return adapters that
    report unavailable metrics (extract_metrics fail-closed).
    """
    if not enable:
        return LiveHybridPorts(
            grobid=None,
            opendataloader=None,
            enabled=False,
            diagnostics=("live_hybrid_disabled",),
        )

    # Lazy infra import: composition root only.
    from research_graph.infrastructure.corpus.parsing.live_sidecar_adapters import (
        LiveGrobidSidecarAdapter,
        LiveOpenDataLoaderSidecarAdapter,
    )
    from research_graph.infrastructure.corpus.parsing.sidecar_services import (
        probe_parser_sidecars,
    )

    status = probe_parser_sidecars(ensure=ensure_containers)
    # Always inject live adapters when enable=True so extract_metrics can re-probe;
    # hybrid merge still requires body evidence for claimed success.
    grobid: GrobidSidecarPort = LiveGrobidSidecarAdapter(ensure_service=ensure_containers)
    odl: OpenDataLoaderSidecarPort = LiveOpenDataLoaderSidecarAdapter(
        ensure_import=ensure_containers
    )
    diagnostics = (
        "live_hybrid_enabled",
        f"sidecar_probe_grobid:{status.grobid.available}",
        f"sidecar_probe_odl:{status.opendataloader.available}",
        f"auto_start_attempted:{status.grobid.auto_start_attempted}",
        f"auto_start_ok:{status.grobid.auto_start_ok}",
        f"compose_file:{status.compose_file}",
    )
    return LiveHybridPorts(
        grobid=grobid,
        opendataloader=odl,
        enabled=True,
        diagnostics=diagnostics,
    )


__all__ = [
    "LiveHybridPorts",
    "resolve_live_hybrid_ports",
]
