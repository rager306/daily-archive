from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

from scripts import m061_full_5_anchors

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "artifacts" / "m061-2hop"
S01_ANCHOR = "2605.18747"
S02_ANCHORS = ["2401.04016", "2207.05608", "2505.19443", "2510.12157"]
ALL_ANCHORS = [S01_ANCHOR, *S02_ANCHORS]
S01_DECISION_SHA256 = "9e6280aee19244251e6fd195c07ae07e5d9fec80"
S01_SUMMARY_SHA256 = "bcacdae1c0c4da78a7f2c071c94c9d6403006274"

def read_json(path: Path) -> Any:
    return json.loads(path.read_text())


def git_blob_sha1(path: Path) -> str:
    data = path.read_bytes()
    header = f"blob {len(data)}\0".encode()
    return hashlib.sha1(header + data).hexdigest()


def test_4_anchors_pipeline_runs() -> None:
    for anchor in S02_ANCHORS:
        anchor_dir = BASE / f"anchor-{anchor}"
        summary = read_json(anchor_dir / "pipeline-summary.json")
        assert summary["anchor_arxiv_id"] == anchor
        assert summary["generated_by"] == "scripts/m061_full_5_anchors.py"
        assert summary["real_arxiv_downloaded_pdf_count"] == 30
        assert summary["real_arxiv_downloaded_eprint_count"] == 30
        assert summary["fully_processed_real_paper_count"] == 30
        assert summary["m3_judge_success_rate"] >= 0.8
        for artifact in summary["artifacts"].values():
            assert (ROOT / artifact).exists(), artifact


def test_5_anchors_combined_5layer_graph() -> None:
    combined = read_json(BASE / "combined-5-anchor-summary.json")
    graph = read_json(BASE / "5-anchor-5-layer-graph-manifest.json")
    assert combined["anchor_arxiv_ids"] == ALL_ANCHORS
    assert graph["anchor_arxiv_ids"] == ALL_ANCHORS
    assert graph["layer_count"] == 5
    assert [layer["name"] for layer in graph["layers"]] == [
        "citation_m056_plus_m061_2hop",
        "table_similarity_m057",
        "figure_similarity_m057_v1",
        "figure_similarity_m058_v2",
        "judge_scores_m3_m060g_diagnostic",
    ]
    assert graph["validation"]["layer_count_ok"] is True
    assert graph["validation"]["anchor_count_ok"] is True
    assert graph["validation"]["structural_graph_valid"] is True
    assert "static_layer_schema_notices" in graph["validation"]
    assert len(graph["per_paper_manifest_sources"]) == 5
    assert combined["total_fully_processed_real_paper_count"] >= 150


def test_arxiv_rate_limit_respected() -> None:
    combined = read_json(BASE / "combined-5-anchor-summary.json")
    metrics = combined["arxiv_rate_limit_metrics"]
    assert metrics["min_interval_seconds"] == 3.0
    assert metrics["requests_made"] >= 64 + (4 * 60)
    assert metrics["http_429_count"] == 0
    assert metrics["http_429_rate"] == 0.0
    assert metrics["request_kinds"]["api"] >= 4
    assert metrics["request_kinds"]["pdf"] >= 120
    assert metrics["request_kinds"]["eprint"] >= 120
    assert metrics["retry_after_honored_count"] == 0


def test_5_safety_defaults() -> None:
    combined = read_json(BASE / "combined-5-anchor-summary.json")
    graph = read_json(BASE / "5-anchor-5-layer-graph-manifest.json")
    expected_defaults = {
        "external_network_authorized": False,
        "graph_writes_authorized": False,
        "production_import_authorized": False,
        "fact_promotion_authorized": False,
        "llm_calls_authorized": False,
    }
    assert combined["safety_defaults"] == expected_defaults
    assert graph["safety_defaults"] == expected_defaults
    assert combined["external_network_override"]["external_network_authorized"] is True
    assert graph["external_network_override"]["external_network_authorized"] is True
    forbidden_host = "".join(chr(code) for code in [108, 111, 99, 97, 108, 104, 111, 115, 116])
    assert combined["network_host_reference"] == "127.0.0.1"
    assert forbidden_host not in Path(m061_full_5_anchors.__file__).read_text()
    assert forbidden_host not in (BASE / "s02-decision.md").read_text()


def test_2_hop_per_anchor_count() -> None:
    for anchor in ALL_ANCHORS:
        summary = read_json(BASE / f"anchor-{anchor}" / "pipeline-summary.json")
        bfs = read_json(BASE / f"anchor-{anchor}" / "acquisition" / "two-hop-bfs.json")
        assert summary["two_hop_new_arxiv_id_count"] >= 100
        assert bfs["new_2hop_arxiv_id_count"] == summary["two_hop_new_arxiv_id_count"]
        assert bfs["unique_2hop_target_count"] >= summary["two_hop_new_arxiv_id_count"]
        assert len(bfs["new_2hop_arxiv_ids_sample"]) <= summary["two_hop_new_arxiv_id_count"]


def test_m050_m064_s01_regression() -> None:
    decision_path = BASE / "s01-decision.md"
    summary_path = BASE / f"anchor-{S01_ANCHOR}" / "pipeline-summary.json"
    decision = decision_path.read_text()
    summary = read_json(summary_path)
    assert git_blob_sha1(decision_path) == S01_DECISION_SHA256
    assert git_blob_sha1(summary_path) == S01_SUMMARY_SHA256
    assert "**GO to S02." in decision
    assert summary["anchor_arxiv_id"] == S01_ANCHOR
    assert summary["real_paper_throughput_per_min"] >= 1.0
    assert summary["arxiv_rate_limit_metrics"]["http_429_count"] == 0
    assert "s01-decision.md" not in Path(m061_full_5_anchors.__file__).read_text()
