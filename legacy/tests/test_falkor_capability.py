"""M203 S01: Falkor no-write capability tracer tests."""

from __future__ import annotations

from typing import Literal

import ast
from pathlib import Path

import pytest

from research_graph.application.graph.falkor_capability import (
    FalkorCapabilityReport,
    capability_summary,
    probe_falkor_capabilities,
)
from research_graph.infrastructure.graph.falkor_capability_probe import (
    probe_local_falkor_service,
)

APP_PATH = Path("src/research_graph/application/graph/falkor_capability.py")
INFRA_PATH = Path("src/research_graph/infrastructure/graph/falkor_capability_probe.py")
FORBIDDEN_IMPORT_ROOTS = {
    "falkordb",
    "redis",
    "networkx",
    "ladybug",
    "httpx",
    "requests",
    "openai",
}


def _imports(path: Path) -> set[str]:
    tree = ast.parse(path.read_text(encoding="utf-8"))
    roots: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                roots.add(alias.name.split(".")[0])
        elif isinstance(node, ast.ImportFrom) and node.module:
            roots.add(node.module.split(".")[0])
    return roots


def test_static_matrix_marks_writes_blocked_and_reads_supported() -> None:
    report = probe_falkor_capabilities()
    assert report.backend == "falkordb"
    assert "MATCH" in report.supported_cypher()
    assert "RETURN" in report.supported_cypher()
    assert "CREATE" in report.blocked_cypher()
    assert "MERGE" in report.blocked_cypher()
    assert report.sdk_imported is False
    assert report.graphdb_port_changed is False
    assert report.write_capable_dependency is False
    assert report.safety_flags.import_eligible is False
    report.assert_no_write()


def test_service_probe_uses_injectable_transport() -> None:
    def fake_transport(host: str, port: int, timeout_s: float) -> tuple[Literal["reachable"], str]:
        assert host == "127.0.0.1"
        assert port == 6381
        return ("reachable", "fake_ok")

    report = probe_falkor_capabilities(
        host="127.0.0.1",
        port=6381,
        probe_service=True,
        transport=fake_transport,
    )
    assert report.service is not None
    assert report.service.status == "reachable"
    assert report.service.detail == "fake_ok"
    summary = capability_summary(report)
    assert summary["service_status"] == "reachable"
    assert "api_key" not in str(report.to_dict())


def test_service_probe_misconfigured_fail_closed() -> None:
    report = probe_falkor_capabilities(probe_service=True)
    assert report.service is not None
    assert report.service.status == "probe_error"


def test_report_rejects_write_capable_flags() -> None:
    with pytest.raises(ValueError, match="no-write"):
        FalkorCapabilityReport(sdk_imported=True)


def test_no_forbidden_sdk_imports_in_capability_modules() -> None:
    for path in (APP_PATH, INFRA_PATH):
        roots = _imports(path)
        leaked = roots & FORBIDDEN_IMPORT_ROOTS
        assert not leaked, f"{path} imported forbidden roots: {leaked}"
    # application must not import infrastructure
    app_src = APP_PATH.read_text(encoding="utf-8")
    assert "research_graph.infrastructure" not in app_src


def test_local_probe_wrapper_returns_report() -> None:
    report = probe_local_falkor_service(host="127.0.0.1", port=1, timeout_s=0.05)
    assert isinstance(report, FalkorCapabilityReport)
    assert report.service is not None
    assert report.service.status in {"unreachable", "probe_error", "reachable"}
    assert "CREATE" in report.blocked_cypher()
