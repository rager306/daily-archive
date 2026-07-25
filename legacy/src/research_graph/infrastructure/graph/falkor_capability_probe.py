"""Infrastructure seam for Falkor capability probing (M203 S01).

Keeps optional local endpoint defaults and re-exports the application tracer.
No Falkor/redis SDK import. No GraphDBPort. No graph writes.
"""

from __future__ import annotations

from research_graph.application.graph.falkor_capability import (
    FalkorCapabilityReport,
    default_tcp_probe,
    probe_falkor_capabilities,
)

# Local disposable defaults (docker-compose style); probe is opt-in.
DEFAULT_FALKOR_HOST = "127.0.0.1"
DEFAULT_FALKOR_PORT = 6379


def probe_local_falkor_service(
    *,
    host: str = DEFAULT_FALKOR_HOST,
    port: int = DEFAULT_FALKOR_PORT,
    timeout_s: float = 0.25,
) -> FalkorCapabilityReport:
    """Capability matrix + optional TCP reachability for a local Falkor endpoint.

    Reachability only means a TCP listener exists; it does not authorize writes,
    open a Falkor client, or run corpus Cypher.
    """
    return probe_falkor_capabilities(
        host=host,
        port=port,
        probe_service=True,
        transport=default_tcp_probe,
        timeout_s=timeout_s,
    )


__all__ = [
    "DEFAULT_FALKOR_HOST",
    "DEFAULT_FALKOR_PORT",
    "probe_local_falkor_service",
]
