from __future__ import annotations

import json
from pathlib import Path

import pytest

from scripts import m060c_applicability_matrix

ROOT = Path(__file__).resolve().parents[1]
MATRIX_SCRIPT_SOURCE = ROOT / "scripts" / "m060c_applicability_matrix.py"
FORBIDDEN_LOOPBACK_HOSTNAME = "local" + "host"


@pytest.fixture(scope="module")
def matrix_report(tmp_path_factory: pytest.TempPathFactory) -> dict:
    output_dir = tmp_path_factory.mktemp("m060c-s02-matrix")
    return m060c_applicability_matrix.emit_m060c_s02_applicability_outputs(output_dir)


def test_applicability_matrix_emitted(matrix_report: dict) -> None:
    json_path = Path(matrix_report["metadata"]["json_path"])
    markdown_path = Path(matrix_report["metadata"]["markdown_path"])
    assert json_path.exists()
    assert markdown_path.exists()
    payload = json.loads(json_path.read_text(encoding="utf-8"))
    markdown = markdown_path.read_text(encoding="utf-8")
    assert payload["artifact"] == "m060c_s02_applicability_matrix"
    assert "# M060c S02 Applicability Matrix" in markdown
    assert (
        "| Library | Milestone | Score | Use-case fit | Integration cost | Decision |" in markdown
    )


def test_applicability_matrix_7_libraries(matrix_report: dict) -> None:
    assert matrix_report["libraries"] == [
        "NetworkX",
        "igraph",
        "rustworkx",
        "graph-tool",
        "PyG",
        "DGL",
        "NetworkX-Temporal",
        "GraphScope",
    ]
    assert len(matrix_report["libraries"]) == 8


def test_applicability_matrix_5_milestones(matrix_report: dict) -> None:
    assert matrix_report["milestones"] == [
        "M060b (intermediate layer)",
        "M061 (2-hop BFS)",
        "M062 (fd hardening)",
        "M063 (GraphDB selection)",
        "M064+ (production)",
    ]
    assert len(matrix_report["cells"]) == 8 * 5
    for cell in matrix_report["cells"]:
        assert 0 <= cell["applicability_score"] <= 3
        assert cell["use_case_fit"]
        assert cell["integration_cost"]
        assert cell["decision"]


def test_applicability_matrix_aggregate_counts(matrix_report: dict) -> None:
    assert matrix_report["aggregate_score_ge_2_count"] == {
        "NetworkX": 5,
        "igraph": 5,
        "rustworkx": 3,
        "graph-tool": 0,
        "PyG": 0,
        "DGL": 0,
        "NetworkX-Temporal": 0,
        "GraphScope": 1,
    }


def test_adr_016_binding() -> None:
    adr_path = ROOT / "doc" / "adr" / "ADR-016-graph-library-selection.md"
    adr = adr_path.read_text(encoding="utf-8")
    assert "**Status:** Accepted (binding)" in adr
    assert "NetworkX as the primary graph representation" in adr
    assert "igraph as the supplementary read-only accelerator" in adr
    assert "rustworkx" in adr
    for section in range(15):
        assert f"## {section}." in adr
    assert "## 14. LLM Reading Notes" in adr
    assert "This ADR does not authorize graph writes" in adr
    assert FORBIDDEN_LOOPBACK_HOSTNAME not in adr


def test_m061_decision_doc() -> None:
    decision_path = ROOT / "artifacts" / "m060c-benchmark" / "m061-m065-decision.md"
    decision = decision_path.read_text(encoding="utf-8")
    for milestone in ("M060b", "M061", "M062", "M063", "M064+"):
        assert milestone in decision
    assert "NetworkX остаётся основной библиотекой" in decision
    assert "igraph" in decision and "supplementary backend" in decision
    assert "rustworkx" in decision and "optional supplementary backend" in decision
    assert "Production import is not authorized." in decision
    assert FORBIDDEN_LOOPBACK_HOSTNAME not in decision


def test_5_safety_defaults(matrix_report: dict) -> None:
    assert matrix_report["safety_defaults"] == {
        "graph_writes_authorized": False,
        "production_import_authorized": False,
        "fact_promotion_authorized": False,
        "external_network_enabled": False,
        "llm_calls_enabled": False,
    }
    assert matrix_report["safety_statements"] == [
        "Graph writes are not authorized.",
        "Production import is not authorized.",
        "Fact promotion is not authorized.",
        "External network default is disabled.",
        "LLM calls default is disabled.",
    ]
    assert matrix_report["loopback_host"] == "127.0.0.1"
    assert FORBIDDEN_LOOPBACK_HOSTNAME not in MATRIX_SCRIPT_SOURCE.read_text(encoding="utf-8")


def test_m050_m060g_s01_regression_surfaces_remain_read_only() -> None:
    for artifact_dir in (
        "m050-work-requests",
        "m052-rlm-e2e",
        "m053-grobid-pilot",
        "m054-pdf-acquisition",
        "m055-parser-benchmark",
        "m056-bfs-graph",
        "m057-fd-marker",
        "m058-marker",
        "m059-architecture",
        "m060g-judge",
    ):
        assert (ROOT / "artifacts" / artifact_dir).exists(), artifact_dir

    report = (ROOT / "artifacts" / "m060g-judge" / "REPORT.md").read_text(encoding="utf-8")
    scope = (ROOT / "artifacts" / "m060g-judge" / "m061-scope.md").read_text(encoding="utf-8")
    for phrase in (
        "Graph writes are not authorized.",
        "Production import is not authorized.",
        "Fact promotion is not authorized.",
        "External network default is disabled.",
        "LLM calls default is disabled.",
    ):
        assert phrase in report or phrase in scope
