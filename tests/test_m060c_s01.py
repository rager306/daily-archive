from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = ROOT / "scripts" / "m060c_benchmark.py"
FORBIDDEN_LOOPBACK_HOSTNAME = "local" + "host"

spec = importlib.util.spec_from_file_location("m060c_benchmark", SCRIPT_PATH)
assert spec is not None and spec.loader is not None
m060c_benchmark = importlib.util.module_from_spec(spec)
sys.modules["m060c_benchmark"] = m060c_benchmark
spec.loader.exec_module(m060c_benchmark)


@pytest.fixture(scope="module")
def benchmark_report(tmp_path_factory: pytest.TempPathFactory) -> dict:
    output_dir = tmp_path_factory.mktemp("m060c-benchmark")
    return m060c_benchmark.run_benchmark(
        output_dir=output_dir,
        synthetic_edges=(60, 120),
        runs=1,
    )


def test_igraph_installed() -> None:
    pytest.importorskip("igraph")


def test_rustworkx_installed() -> None:
    pytest.importorskip("rustworkx")


def test_benchmark_runs_3_libraries(benchmark_report: dict) -> None:
    assert benchmark_report["libraries"] == ["networkx", "igraph", "rustworkx"]
    assert {graph["name"] for graph in benchmark_report["graphs"]} == {
        "m058_4_layer_9418",
        "synthetic_60",
        "synthetic_120",
    }
    assert len(benchmark_report["results"]) == 3 * 3 * 4
    assert {result["status"] for result in benchmark_report["results"]} <= {"ok", "skipped", "error"}


def test_benchmark_has_all_4_algorithms(benchmark_report: dict) -> None:
    assert benchmark_report["algorithms"] == [
        "bfs",
        "pagerank",
        "shortest_path",
        "connected_components",
    ]
    for graph in {result["graph"] for result in benchmark_report["results"]}:
        for library in benchmark_report["libraries"]:
            algorithms = {
                result["algorithm"]
                for result in benchmark_report["results"]
                if result["graph"] == graph and result["library"] == library
            }
            assert algorithms == set(benchmark_report["algorithms"])


def test_benchmark_comparison_table(benchmark_report: dict) -> None:
    rows = benchmark_report["comparison_table"]
    assert len(rows) == 9
    for row in rows:
        assert {"graph", "library", "nodes", "edges", "bfs", "pagerank", "shortest_path", "connected_components"} <= set(row)
    markdown_path = Path(benchmark_report["metadata"]["markdown_path"])
    markdown = markdown_path.read_text(encoding="utf-8")
    assert "| Graph | Library | Nodes | Edges | BFS | PageRank | Shortest path | Connected components |" in markdown
    assert "## Speedup vs NetworkX" in markdown
    assert FORBIDDEN_LOOPBACK_HOSTNAME not in markdown


def test_5_safety_defaults(benchmark_report: dict) -> None:
    assert benchmark_report["safety_defaults"] == {
        "graph_writes_authorized": False,
        "production_import_authorized": False,
        "fact_promotion_authorized": False,
        "external_network_enabled": False,
        "llm_calls_enabled": False,
    }
    assert benchmark_report["safety_statements"] == [
        "Graph writes are not authorized.",
        "Production import is not authorized.",
        "Fact promotion is not authorized.",
        "External network default is disabled.",
        "LLM calls default is disabled.",
    ]
    assert benchmark_report["metadata"]["loopback_host"] == "127.0.0.1"
    assert FORBIDDEN_LOOPBACK_HOSTNAME not in SCRIPT_PATH.read_text(encoding="utf-8")


def test_m050_m060g_regression_surfaces_remain_read_only() -> None:
    report = (ROOT / "artifacts" / "m060g-judge" / "REPORT.md").read_text(encoding="utf-8")
    scope = (ROOT / "artifacts" / "m060g-judge" / "m061-scope.md").read_text(encoding="utf-8")
    guardrail = ROOT / "artifacts" / "m044-grobid-architecture-guardrail" / "architecture-context-pack.json"
    assert guardrail.exists()
    for phrase in (
        "Graph writes are not authorized.",
        "Production import is not authorized.",
        "Fact promotion is not authorized.",
        "External network default is disabled.",
        "LLM calls default is disabled.",
    ):
        assert phrase in report or phrase in scope
    assert "NetworkX-Temporal" not in SCRIPT_PATH.read_text(encoding="utf-8")
    assert "graph-tool" not in SCRIPT_PATH.read_text(encoding="utf-8")
    assert "PyG" not in SCRIPT_PATH.read_text(encoding="utf-8")
    assert "DGL" not in SCRIPT_PATH.read_text(encoding="utf-8")


def test_library_research_reports_exist() -> None:
    research_dir = ROOT / "artifacts" / "m060c-benchmark" / "library-research"
    expected_reports = {
        "python-igraph.md",
        "rustworkx.md",
        "pytorch_geometric.md",
        "dgl.md",
        "networkx-temporal.md",
        "graphscope.md",
    }
    for report_name in expected_reports:
        report_path = research_dir / report_name
        assert report_path.exists(), report_name
        report = report_path.read_text(encoding="utf-8")
        assert "## Architecture summary" in report
        assert "## Algorithm support table" in report
        assert "## Our use case fit" in report
        assert "## Decision" in report

    graph_tool_note = research_dir / "graph-tool.md"
    assert graph_tool_note.exists()
    graph_tool_text = graph_tool_note.read_text(encoding="utf-8")
    assert "**NOT_VENDORED**" in graph_tool_text
    assert "**DEFER**" in graph_tool_text
