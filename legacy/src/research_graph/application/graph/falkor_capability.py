"""FalkorDB no-write capability tracer (M203 S01).

Maps Falkor OpenCypher/read surface into metadata-only reports for projection
planning. Does not import Falkor/redis SDKs, open GraphDBPort, load corpus, or
authorize writes.
"""

from __future__ import annotations

import socket
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
from typing import Any, Literal

from research_graph.domain.universal_kb.contracts import SafetyFlags

CapabilityStatus = Literal["supported", "unsupported", "unknown", "blocked"]
ServiceStatus = Literal["not_probed", "reachable", "unreachable", "probe_error"]

# Static OpenCypher / Falkor capability matrix for no-write planning.
# Supported means dialect-compatible for future read/plan paths, not that a
# live mutation is authorized.
DEFAULT_CYPHER_CAPABILITIES: tuple[tuple[str, CapabilityStatus, str], ...] = (
    ("MATCH", "supported", "node/relationship pattern match"),
    ("RETURN", "supported", "projection of bound symbols"),
    ("WHERE", "supported", "predicate filter"),
    ("WITH", "supported", "query pipeline staging"),
    ("ORDER_BY", "supported", "result ordering"),
    ("LIMIT", "supported", "result bounding"),
    ("OPTIONAL_MATCH", "supported", "optional pattern match"),
    ("UNWIND", "supported", "list expansion"),
    ("CREATE", "blocked", "write clause — blocked under no-write governance"),
    ("MERGE", "blocked", "write clause — blocked under no-write governance"),
    ("DELETE", "blocked", "write clause — blocked under no-write governance"),
    ("SET", "blocked", "write clause — blocked under no-write governance"),
    ("REMOVE", "blocked", "write clause — blocked under no-write governance"),
    ("CALL_DB_INDEXES", "supported", "schema introspection read"),
    ("EXPLAIN", "supported", "plan-only query analysis"),
    ("PROFILE", "unknown", "runtime profile may require live service"),
)

DEFAULT_VECTOR_CAPABILITIES: tuple[tuple[str, CapabilityStatus, str], ...] = (
    ("vector_index_read", "supported", "Falkor vector index read planned"),
    ("vector_index_write", "blocked", "write-path deferred to M205"),
    ("fulltext_read", "supported", "full-text query read planned"),
    ("fulltext_write", "blocked", "write-path deferred to M205"),
)


@dataclass(frozen=True, slots=True)
class CapabilityItem:
    """One dialect/feature capability entry."""

    name: str
    status: CapabilityStatus
    detail: str

    def to_dict(self) -> dict[str, str]:
        return {"name": self.name, "status": self.status, "detail": self.detail}


@dataclass(frozen=True, slots=True)
class FalkorServiceProbe:
    """Optional local service reachability (TCP only, no protocol handshake)."""

    host: str
    port: int
    status: ServiceStatus
    detail: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "host": self.host,
            "port": self.port,
            "status": self.status,
            "detail": self.detail,
        }


@dataclass(frozen=True, slots=True)
class FalkorCapabilityReport:
    """Metadata-only Falkor capability map for no-write compatibility."""

    backend: str = "falkordb"
    dialect: str = "opencypher"
    cypher_capabilities: tuple[CapabilityItem, ...] = ()
    vector_capabilities: tuple[CapabilityItem, ...] = ()
    service: FalkorServiceProbe | None = None
    projection_port: str = "KnowledgeGraphProjectionPort"
    graphdb_port_changed: bool = False
    write_capable_dependency: bool = False
    sdk_imported: bool = False
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.write_capable_dependency or self.sdk_imported or self.graphdb_port_changed:
            raise ValueError(
                "Falkor capability tracer must remain no-write: "
                "no SDK import, no write-capable dependency, no GraphDBPort change"
            )

    def assert_no_write(self) -> None:
        self.safety_flags.assert_no_write()

    def supported_cypher(self) -> tuple[str, ...]:
        return tuple(c.name for c in self.cypher_capabilities if c.status == "supported")

    def blocked_cypher(self) -> tuple[str, ...]:
        return tuple(c.name for c in self.cypher_capabilities if c.status == "blocked")

    def to_dict(self) -> dict[str, Any]:
        payload = {
            "backend": self.backend,
            "dialect": self.dialect,
            "cypher_capabilities": [c.to_dict() for c in self.cypher_capabilities],
            "vector_capabilities": [c.to_dict() for c in self.vector_capabilities],
            "service": self.service.to_dict() if self.service else None,
            "projection_port": self.projection_port,
            "graphdb_port_changed": self.graphdb_port_changed,
            "write_capable_dependency": self.write_capable_dependency,
            "sdk_imported": self.sdk_imported,
            "safety_flags": self.safety_flags.to_dict(),
            "diagnostics": list(self.diagnostics),
        }
        # leakage control for serialized reports
        text = str(payload).lower()
        for forbidden in ("api_key", "password", "embedding", "raw_text", "secret"):
            if forbidden in text:
                raise ValueError(f"capability report leaked forbidden token: {forbidden}")
        return payload


TransportProbe = Callable[[str, int, float], tuple[ServiceStatus, str]]


def default_tcp_probe(host: str, port: int, timeout_s: float = 0.25) -> tuple[ServiceStatus, str]:
    """Stdlib TCP connect probe; no redis/falkor protocol."""
    try:
        with socket.create_connection((host, port), timeout=timeout_s):
            return ("reachable", "tcp_connect_ok")
    except TimeoutError:
        return ("unreachable", "tcp_timeout")
    except OSError as exc:
        return ("unreachable", f"tcp_error:{type(exc).__name__}")
    except Exception as exc:  # noqa: BLE001 - fail-closed probe surface
        return ("probe_error", f"probe_error:{type(exc).__name__}")


def _items_from_matrix(
    matrix: Sequence[tuple[str, CapabilityStatus, str]],
) -> tuple[CapabilityItem, ...]:
    return tuple(CapabilityItem(name=n, status=s, detail=d) for n, s, d in matrix)


def probe_falkor_capabilities(
    *,
    host: str | None = None,
    port: int | None = None,
    probe_service: bool = False,
    transport: TransportProbe | None = None,
    cypher_matrix: Sequence[tuple[str, CapabilityStatus, str]] | None = None,
    vector_matrix: Sequence[tuple[str, CapabilityStatus, str]] | None = None,
    timeout_s: float = 0.25,
) -> FalkorCapabilityReport:
    """Build a no-write Falkor capability report; optional local TCP probe only."""
    cypher = _items_from_matrix(cypher_matrix or DEFAULT_CYPHER_CAPABILITIES)
    vector = _items_from_matrix(vector_matrix or DEFAULT_VECTOR_CAPABILITIES)
    diagnostics: list[str] = ["capability_matrix_static", "no_sdk_import", "graphdb_port_unchanged"]

    service: FalkorServiceProbe | None = None
    if probe_service:
        if not host or port is None:
            service = FalkorServiceProbe(
                host=host or "",
                port=int(port or 0),
                status="probe_error",
                detail="host_and_port_required",
            )
            diagnostics.append("service_probe_misconfigured")
        else:
            probe_fn = transport or default_tcp_probe
            status, detail = probe_fn(host, int(port), timeout_s)
            service = FalkorServiceProbe(host=host, port=int(port), status=status, detail=detail)
            diagnostics.append(f"service_probe:{status}")

    report = FalkorCapabilityReport(
        cypher_capabilities=cypher,
        vector_capabilities=vector,
        service=service,
        diagnostics=tuple(diagnostics),
    )
    report.assert_no_write()
    return report


def capability_summary(report: FalkorCapabilityReport) -> dict[str, Any]:
    """Compact operator-facing summary (metadata only)."""
    return {
        "backend": report.backend,
        "supported_cypher": list(report.supported_cypher()),
        "blocked_cypher": list(report.blocked_cypher()),
        "service_status": report.service.status if report.service else "not_probed",
        "projection_port": report.projection_port,
        "sdk_imported": report.sdk_imported,
        "graphdb_port_changed": report.graphdb_port_changed,
        "safety_flags": report.safety_flags.to_dict(),
    }


def matrix_as_mapping(report: FalkorCapabilityReport) -> Mapping[str, CapabilityStatus]:
    return {item.name: item.status for item in report.cypher_capabilities}


__all__ = [
    "DEFAULT_CYPHER_CAPABILITIES",
    "DEFAULT_VECTOR_CAPABILITIES",
    "CapabilityItem",
    "CapabilityStatus",
    "FalkorCapabilityReport",
    "FalkorServiceProbe",
    "ServiceStatus",
    "TransportProbe",
    "capability_summary",
    "default_tcp_probe",
    "matrix_as_mapping",
    "probe_falkor_capabilities",
]
