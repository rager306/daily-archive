from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path
from typing import Any

import pytest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = ROOT / "scripts" / "m061_anchor_pilot.py"

spec = importlib.util.spec_from_file_location("m061_anchor_pilot", SCRIPT_PATH)
assert spec is not None and spec.loader is not None
m061_anchor_pilot = importlib.util.module_from_spec(spec)
sys.modules["m061_anchor_pilot"] = m061_anchor_pilot
spec.loader.exec_module(m061_anchor_pilot)


@pytest.fixture(scope="session")
def pilot_output(tmp_path_factory: pytest.TempPathFactory) -> dict[str, Any]:
    output_dir = tmp_path_factory.mktemp("m061-s01") / "anchor-2605.18747"
    summary = m061_anchor_pilot.run_pilot(output_dir=output_dir, max_papers=30)
    return {"summary": summary, "output_dir": output_dir, "decision_path": output_dir.parent / "s01-decision.md"}


def read_json(path: Path) -> Any:
    return json.loads(path.read_text())


def test_1_anchor_pipeline_runs(pilot_output: dict[str, Any]) -> None:
    summary = pilot_output["summary"]
    output_dir = pilot_output["output_dir"]
    assert summary["anchor_arxiv_id"] == "2605.18747"
    assert summary["sync_execution"] is True
    assert summary["queue_execution"] is False
    assert summary["one_hop_validated_count"] == 165
    assert (output_dir / "pipeline-summary.json").is_file()
    assert pilot_output["decision_path"].is_file()


def test_2_hop_bfs_produces_new_arxiv_ids(pilot_output: dict[str, Any]) -> None:
    output_dir = pilot_output["output_dir"]
    summary = pilot_output["summary"]
    bfs = read_json(output_dir / "acquisition" / "two-hop-bfs.json")
    assert summary["two_hop_new_arxiv_id_count"] >= 100
    assert bfs["new_2hop_arxiv_id_count"] == summary["two_hop_new_arxiv_id_count"]
    assert bfs["one_hop_with_tei_count"] >= 140
    assert len(bfs["edges"]) > 4000


def test_8_stages_complete_per_paper(pilot_output: dict[str, Any]) -> None:
    output_dir = pilot_output["output_dir"]
    stage_report = read_json(output_dir / "parsing" / "per-paper-stage-report.json")
    assert stage_report["selected_paper_count"] == 30
    assert stage_report["manifest_validation_success_rate"] >= 0.90
    assert stage_report["manifest_validation_passed_count"] == 30
    for paper in stage_report["papers"]:
        assert len(paper["stage_records"]) == 8
        assert {record["stage"] for record in paper["stage_records"]} == set(range(1, 9))
        assert not any("failed" in record["status"] for record in paper["stage_records"])


def test_m3_judge_scores_collected(pilot_output: dict[str, Any]) -> None:
    output_dir = pilot_output["output_dir"]
    m3 = read_json(output_dir / "judgments" / "m3-judgments.json")
    assert m3["model_used"] == "MiniMax-M3"
    assert m3["figure_count"] >= 30
    assert m3["success_rate"] >= 0.80
    assert m3["diagnostic_llm_calls_override"]["llm_calls_authorized"] is True
    assert m3["safety_defaults"]["llm_calls_authorized"] is False


def test_5_layer_graph_emitted(pilot_output: dict[str, Any]) -> None:
    output_dir = pilot_output["output_dir"]
    graph = read_json(output_dir / "graph" / "5-layer-graph-manifest.json")
    layer_names = {layer["name"] for layer in graph["layers"]}
    assert graph["layer_count"] == 5
    assert layer_names == {
        "citation_m056_plus_m061_2hop",
        "table_similarity_m057",
        "figure_similarity_m057_v1",
        "figure_similarity_m058_v2",
        "m3_judge_m060g_diagnostic",
    }
    assert all(layer["edge_count"] > 0 for layer in graph["layers"])
    assert graph["total_node_count_by_layer_sum"] > 0


def test_5_safety_defaults(pilot_output: dict[str, Any]) -> None:
    summary = pilot_output["summary"]
    assert summary["safety_defaults"] == {
        "external_network_authorized": False,
        "graph_writes_authorized": False,
        "production_import_authorized": False,
        "fact_promotion_authorized": False,
        "llm_calls_authorized": False,
    }
    assert summary["network_host_reference"] == "127.0.0.1"
    forbidden_loopback_alias = "local" + "host"
    assert forbidden_loopback_alias not in SCRIPT_PATH.read_text()
    assert forbidden_loopback_alias not in pilot_output["decision_path"].read_text()


def test_m050_m063_regression_input_contracts() -> None:
    m056_edges = read_json(ROOT / "artifacts" / "m056-bfs-graph" / "candidate-edges.json")
    m056_corpus = read_json(ROOT / "artifacts" / "m056-bfs-graph" / "cumulative-corpus.json")
    m057_tables = read_json(ROOT / "artifacts" / "m057-fd-marker" / "table-similarity" / "edges.json")
    m057_figures = read_json(ROOT / "artifacts" / "m057-fd-marker" / "figure-links" / "edges.json")
    m058_figures = read_json(ROOT / "artifacts" / "m058-plotextractor" / "edges.json")
    m060g = read_json(ROOT / "artifacts" / "m060g-judge" / "comparison.json")

    assert m056_corpus["anchor_arxiv_id"] == "2605.18747"
    assert m056_corpus["unique_1hop_pdf_count"] == 165
    assert len(m056_edges["edges"]) == 4454
    assert len(m057_tables["edges"]) == 4934
    assert len(m057_figures["edges"]) == 15
    assert len(m058_figures["edges"]) == 15
    quality = m060g["aggregate"]["model_stats"]["figure-qa-judge-quality"]
    assert quality["model_used"] == "MiniMax-M3"
    assert quality["passed_count"] == 30
