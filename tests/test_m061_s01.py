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
def pilot_output() -> dict[str, Any]:
    output_dir = m061_anchor_pilot.DEFAULT_OUTPUT_DIR
    summary = m061_anchor_pilot.run_pilot(output_dir=output_dir, max_papers=30)
    return {
        "summary": summary,
        "output_dir": output_dir,
        "decision_path": output_dir.parent / "s01-decision.md",
    }


def read_json(path: Path) -> Any:
    return json.loads(path.read_text())


def test_1_anchor_pipeline_runs_with_real_acquisition(pilot_output: dict[str, Any]) -> None:
    summary = pilot_output["summary"]
    assert summary["schema_version"] == "m061-2hop.anchor-pilot-summary.v2"
    assert summary["anchor_arxiv_id"] == "2605.18747"
    assert summary["sync_execution"] is True
    assert summary["queue_execution"] is False
    assert summary["network_host_reference"] == "127.0.0.1"
    assert summary["real_arxiv_downloaded_pdf_count"] == 30
    assert summary["real_arxiv_downloaded_eprint_count"] == 30
    assert summary["fully_processed_real_paper_count"] == 30
    assert Path(summary["artifacts"]["arxiv_acquisition"]).exists()
    assert Path(summary["artifacts"]["per_paper_stage_report"]).exists()


def test_2_hop_bfs_produces_new_arxiv_ids(pilot_output: dict[str, Any]) -> None:
    summary = pilot_output["summary"]
    two_hop = read_json(Path(summary["artifacts"]["two_hop_bfs"]))
    assert summary["one_hop_validated_count"] == 165
    assert summary["two_hop_new_arxiv_id_count"] == 2491
    assert two_hop["new_2hop_arxiv_id_count"] == 2491
    assert len(two_hop["edges"]) >= 4000


def test_8_stages_complete_per_paper_with_real_pdfs(pilot_output: dict[str, Any]) -> None:
    summary = pilot_output["summary"]
    report = read_json(Path(summary["artifacts"]["per_paper_stage_report"]))
    assert report["selected_paper_count"] == 30
    assert report["locally_available_pdf_count"] == 30
    assert report["fully_processed_real_paper_count"] == 30
    assert report["manifest_validation_success_rate"] >= 0.90
    for paper in report["papers"]:
        assert paper["pdf_available_locally"] is True
        assert paper["fully_processed_real_paper"] is True
        assert len(paper["stage_records"]) == 8
        assert {record["stage"] for record in paper["stage_records"]} == set(range(1, 9))
        assert all(
            record["status"] not in {"failed", "partial", "validation_failed"}
            for record in paper["stage_records"]
        )
        assert paper["parser_result"]["grobid_status"] in {"success", "reused_existing_m056"}
        plotextractor = read_json(
            pilot_output["output_dir"] / paper["parser_result"]["plotextractor_output"]
        )
        assert plotextractor["per_pdf"][0]["tex_status"] == "downloaded_eprint_source"


def test_m3_judge_scores_collected(pilot_output: dict[str, Any]) -> None:
    summary = pilot_output["summary"]
    judgments = read_json(Path(summary["artifacts"]["m3_judgments"]))
    assert summary["m3_judge_figure_count"] >= 30
    assert summary["m3_judge_success_rate"] >= 0.80
    assert judgments["status"] == "complete_reused_m060g_diagnostic"
    assert judgments["diagnostic_llm_calls_override"]["llm_calls_authorized"] is True


def test_5_layer_graph_emitted(pilot_output: dict[str, Any]) -> None:
    summary = pilot_output["summary"]
    graph = read_json(Path(summary["artifacts"]["graph_manifest"]))
    layer_names = [layer["name"] for layer in graph["layers"]]
    assert graph["layer_count"] == 5
    assert layer_names == [
        "citation_m056_plus_m061_2hop",
        "table_similarity_m057",
        "figure_similarity_m057_v1",
        "figure_similarity_m058_v2",
        "judge_scores_m3_m060g_diagnostic",
    ]
    assert summary["graph_node_count_per_layer"]["judge_scores_m3_m060g_diagnostic"] >= 30
    assert summary["graph_edge_count_per_layer"]["citation_m056_plus_m061_2hop"] >= 4000


def test_arxiv_rate_limit_respected(pilot_output: dict[str, Any]) -> None:
    summary = pilot_output["summary"]
    acquisition = read_json(Path(summary["artifacts"]["arxiv_acquisition"]))
    metrics = acquisition["rate_limit_metrics"]
    assert acquisition["downloaded_pdf_count"] == 30
    assert metrics["user_agent"] == "daily-archive/1.0 (mailto: contact@example.com)"
    assert metrics["min_interval_seconds"] == 3.0
    assert metrics["max_retry_attempts_per_request"] == 3
    assert metrics["backoff_schedule_seconds"] == [1.0, 5.0, 15.0, 60.0, 300.0]
    assert metrics["requests_made"] >= 60
    assert metrics["request_kinds"]["pdf"] == 30
    assert metrics["request_kinds"]["eprint"] == 30
    assert metrics["http_429_rate"] >= 0.0
    if metrics["pacing_delay_count"]:
        assert metrics["average_pacing_delay_seconds"] >= 2.5


def test_5_safety_defaults_with_override(pilot_output: dict[str, Any]) -> None:
    summary = pilot_output["summary"]
    assert set(summary["safety_defaults"]) == {
        "external_network_authorized",
        "graph_writes_authorized",
        "production_import_authorized",
        "fact_promotion_authorized",
        "llm_calls_authorized",
    }
    assert all(value is False for value in summary["safety_defaults"].values())
    assert summary["external_network_override"]["external_network_authorized"] is True
    assert "M064-wqfgfa S01" in summary["external_network_override"]["scope"]
    assert "no production import" in summary["external_network_override"]["scope"]
    assert "no graph writes" in summary["external_network_override"]["scope"]


def test_m050_m063_regression_input_contracts() -> None:
    required_paths = [
        ROOT / "artifacts/m056-bfs-graph/candidate-edges.json",
        ROOT / "artifacts/m056-bfs-graph/cumulative-corpus.json",
        ROOT / "artifacts/m057-fd-marker/table-similarity/edges.json",
        ROOT / "artifacts/m057-fd-marker/figure-links/edges.json",
        ROOT / "artifacts/m058-plotextractor/edges.json",
        ROOT / "artifacts/m060g-judge/comparison.json",
        ROOT / "doc/adr/ADR-013-manifest-driven-pdf-ingest.md",
        ROOT / "doc/adr/ADR-014-minimax-judge-m3-multimodal.md",
        ROOT / "doc/adr/ADR-017-pipeline-queue-deferred.md",
    ]
    for path in required_paths:
        assert path.exists(), path
    candidate = read_json(ROOT / "artifacts/m056-bfs-graph/candidate-edges.json")
    assert candidate["diagnostic_only"] is True
    assert candidate["graph_writes_authorized"] is False
    assert candidate["production_import_authorized"] is False
    assert candidate["safety_flags"]["graph_writes"] is False
