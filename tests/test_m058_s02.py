from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import m058_marker_compare_5 as compare_5  # noqa: E402
import m058_marker_extract_5 as extract_5  # noqa: E402

ARTIFACT_ROOT = ROOT / "artifacts" / "m058-marker" / "pilot-5"
SUMMARY = ARTIFACT_ROOT / "summary.json"
COMPARISON = ARTIFACT_ROOT / "comparison.json"
COMPARISON_MD = ARTIFACT_ROOT / "comparison.md"
DECISION = ARTIFACT_ROOT / "decision.md"
PER_PDF = ARTIFACT_ROOT / "per-pdf"
EXPECTED_EXECUTED_IDS = {"2603.21520", "2605.28617v1", "2508.07434", "2412.15118", "1804.02767"}
EXPECTED_REQUESTED_IDS = {"2603.21520", "2605.28617v1", "2508.07434", "2412.15118", "2305.14314"}
EXPECTED_SAFETY_DEFAULTS = {
    "external_network_authorized": False,
    "fact_promotion_authorized": False,
    "graph_writes_authorized": False,
    "llm_calls_authorized": False,
    "production_import_authorized": False,
}


def _load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _assert_five_false_safety_defaults(payload: dict) -> None:
    safety = payload.get("safety_defaults")
    assert safety == EXPECTED_SAFETY_DEFAULTS
    assert all(value is False for value in safety.values())


def test_marker_5_pdfs_extracted() -> None:
    summary = _load_json(SUMMARY)
    _assert_five_false_safety_defaults(summary)
    assert summary["sample_size"] == 5
    assert summary["successful"] == 5
    assert summary["failed"] == 0
    assert set(summary["executed_sample"]) == EXPECTED_EXECUTED_IDS
    assert set(summary["requested_sample"]) == EXPECTED_REQUESTED_IDS
    assert summary["unavailable_requested_sample"][0]["arxiv_id"] == "2305.14314"
    assert summary["page_range"] == "0"
    for arxiv_id in EXPECTED_EXECUTED_IDS:
        packet = _load_json(PER_PDF / f"{arxiv_id}.json")
        assert packet["status"] == "marker_extracted"
        assert packet["arxiv_id"] == arxiv_id


def test_marker_markdown_length_positive() -> None:
    summary = _load_json(SUMMARY)
    for packet in summary["per_pdf"]:
        assert packet["markdown_length"] > 0, packet["arxiv_id"]
        assert packet["body_word_count"] > 0, packet["arxiv_id"]
        assert packet["marker_version"]
        assert packet["transformers_version"]


def test_marker_vs_opendataloader_comparison() -> None:
    comparison = _load_json(COMPARISON)
    _assert_five_false_safety_defaults(comparison)
    assert comparison["sample_size"] == 5
    assert comparison["successful_marker_extractions"] == 5
    assert comparison["available_odl_comparisons"] >= 2
    assert comparison["avg_quality_delta"] > 0
    assert comparison["marker_better_than_odl_percent"] > 0
    assert comparison["page_limited"] is True
    assert comparison["substituted_input"] is True
    assert comparison["go_to_s03"] is False
    assert COMPARISON_MD.exists()
    assert DECISION.exists()
    compared = [item for item in comparison["per_pdf"] if item["status"] == "compared"]
    assert {item["arxiv_id"] for item in compared} >= {"2605.28617v1", "1804.02767"}
    for item in compared:
        assert item["marker_table_count"] is not None
        assert item["marker_body_word_count"] > 0
        assert item["quality_delta"] is not None


def test_marker_time_per_page_recorded() -> None:
    summary = _load_json(SUMMARY)
    assert summary["total_elapsed_sec"] > 0
    assert summary["avg_elapsed_sec"] > 0
    for packet in summary["per_pdf"]:
        assert packet["elapsed_sec"] > 0, packet["arxiv_id"]
    comparison = _load_json(COMPARISON)
    for item in comparison["per_pdf"]:
        assert item["marker_elapsed_sec"] is not None
        assert item["marker_elapsed_sec"] > 0
        if item["time_ratio_marker_over_odl"] is not None:
            assert item["time_ratio_marker_over_odl"] > 0


def test_5_safety_defaults() -> None:
    _assert_five_false_safety_defaults(_load_json(SUMMARY))
    _assert_five_false_safety_defaults(_load_json(COMPARISON))
    assert extract_5.SAFETY_DEFAULTS == EXPECTED_SAFETY_DEFAULTS
    assert compare_5.SAFETY_DEFAULTS == EXPECTED_SAFETY_DEFAULTS
    assert extract_5.LOOPBACK_BIND_HOST == "127.0.0.1"
    assert compare_5.LOOPBACK_BIND_HOST == "127.0.0.1"
    checked_paths = [
        ROOT / "scripts" / "m058_marker_extract_5.py",
        ROOT / "scripts" / "m058_marker_compare_5.py",
        COMPARISON_MD,
        DECISION,
    ]
    forbidden_loopback_alias = "".join(["local", "host"])
    for path in checked_paths:
        assert forbidden_loopback_alias not in path.read_text(encoding="utf-8").lower()


def test_m050_m058_s01_regression() -> None:
    representative_paths = [
        ROOT / "artifacts" / "m050-work-requests",
        ROOT / "artifacts" / "m052-rlm-e2e",
        ROOT / "artifacts" / "m053-grobid-pilot",
        ROOT / "artifacts" / "m054-pdf-acquisition",
        ROOT / "artifacts" / "m055-parser-benchmark",
        ROOT / "artifacts" / "m056-bfs-graph",
        ROOT / "artifacts" / "m057-fd-marker" / "figure-links" / "summary.json",
        ROOT / "artifacts" / "m058-plotextractor" / "summary.json",
    ]
    for path in representative_paths:
        assert path.exists(), path
    m057_summary = _load_json(
        ROOT / "artifacts" / "m057-fd-marker" / "figure-links" / "summary.json"
    )
    assert m057_summary["edges_total"] == 15
    assert m057_summary["inter_doc_edges"] == 15
    assert m057_summary["total_figures"] == 937
    m058_summary = _load_json(ROOT / "artifacts" / "m058-plotextractor" / "summary.json")
    assert m058_summary["edges_total"] == 15
    assert {item["arxiv_id"] for item in m058_summary["per_pdf"]} == {
        "2605.18747",
        "2601.05808",
        "2602.10090",
        "2507.19457",
        "1804.02767",
    }


def test_comparison_helpers_with_tmp_path(tmp_path: Path) -> None:
    packet_path = tmp_path / "odl.json"
    packet_path.write_text(
        json.dumps(
            {
                "arxiv_id": "x",
                "status": "success",
                "tables_total": 2,
                "figures_total": 3,
                "body_word_count": 100,
                "elapsed_sec": 4.0,
            }
        ),
        encoding="utf-8",
    )
    packet = _load_json(packet_path)
    assert compare_5.odl_table_count(packet) == 2
    assert compare_5.odl_figure_count(packet) == 3
    assert compare_5.odl_body_word_count(packet) == 100
    assert compare_5.odl_elapsed_sec(packet) == 4.0
    assert compare_5.quality_score(2, 3, 100) == 260
