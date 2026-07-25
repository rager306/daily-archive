"""Artifact tests for M056-lchpnp S05 Wave 5 acquisition and analysis."""

from __future__ import annotations

import json
from pathlib import Path
from types import ModuleType

from scripts import analyze_m056_wave_5 as analyzer

ROOT = Path(__file__).resolve().parents[1]
WAVE_ORDER = Path("/tmp/wave-order.json")
WAVE_4_DIR = ROOT / "artifacts" / "m056-bfs-graph" / "wave-4"
WAVE_5_DIR = ROOT / "artifacts" / "m056-bfs-graph" / "wave-5"
ACQUISITION_LOG = WAVE_5_DIR / "acquisition-log.json"
CORPUS_MANIFEST = WAVE_5_DIR / "corpus-manifest.json"
ANALYSIS_JSON = WAVE_5_DIR / "analysis.json"
ANALYSIS_MD = WAVE_5_DIR / "analysis.md"
CUMULATIVE_CORPUS = WAVE_5_DIR / "cumulative-corpus.json"
GROBID_DIR = WAVE_5_DIR / "grobid-fulltext"
OPENDATALOADER_DIR = WAVE_5_DIR / "opendataloader"


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def assert_all_false(value: object) -> None:
    assert isinstance(value, dict)
    assert value
    assert all(item is False for item in value.values())


def load_analyzer() -> ModuleType:
    return analyzer


def test_wave_5_acquisition_matches_positions_121_to_150() -> None:
    acquisition = load_json(ACQUISITION_LOG)
    manifest = load_json(CORPUS_MANIFEST)
    expected_ids = json.loads(WAVE_ORDER.read_text(encoding="utf-8"))[120:150]

    assert acquisition["requested_arxiv_ids"] == expected_ids
    assert acquisition["success_count"] >= 25
    assert acquisition["success_count"] == 30
    assert acquisition["status_counts"] == {"acquired": 30}
    assert len(manifest["pdfs"]) == 30
    assert {pdf["arxiv_id"] for pdf in manifest["pdfs"]} == set(expected_ids)
    assert_all_false(acquisition["safety_defaults"])
    assert_all_false(manifest["safety_defaults"])


def test_wave_5_parser_packet_counts_and_safety_defaults() -> None:
    grobid_packets = sorted((GROBID_DIR / "per-pdf").glob("*.json"))
    opendataloader_packets = sorted((OPENDATALOADER_DIR / "per-pdf").glob("*.json"))
    grobid_summary = load_json(GROBID_DIR / "summary.json")
    opendataloader_summary = load_json(OPENDATALOADER_DIR / "summary.json")

    assert len(grobid_packets) == 30
    assert len(opendataloader_packets) == 30
    assert grobid_summary["total_pdfs"] == 30
    assert opendataloader_summary["total_pdfs"] == 30
    assert grobid_summary["success_count"] == 30
    assert_all_false(grobid_summary["safety_defaults"])
    assert_all_false(opendataloader_summary["safety_defaults"])
    for packet_path in grobid_packets + opendataloader_packets:
        assert_all_false(load_json(packet_path)["safety_defaults"])


def test_wave_5_analysis_tracks_cumulative_saturation() -> None:
    analysis = load_json(ANALYSIS_JSON)
    previous = load_json(WAVE_4_DIR / "analysis.json")
    connectivity = analysis["connectivity"]

    assert analysis["schema_version"] == "m056-bfs-wave-5-analysis.v1"
    assert connectivity["wave_1_edge_count"] == previous["connectivity"]["wave_1_edge_count"]
    assert (
        connectivity["wave_2_new_edge_count"] == previous["connectivity"]["wave_2_new_edge_count"]
    )
    assert (
        connectivity["wave_3_new_edge_count"] == previous["connectivity"]["wave_3_new_edge_count"]
    )
    assert (
        connectivity["wave_4_new_edge_count"] == previous["connectivity"]["wave_4_new_edge_count"]
    )
    assert (
        connectivity["cumulative_edge_count"] >= previous["connectivity"]["cumulative_edge_count"]
    )
    assert connectivity["cumulative_edge_count"] == len(connectivity["cumulative_edges"])
    assert (
        connectivity["edge_saturation_by_wave"]["wave_5"] == connectivity["wave_5_new_edge_count"]
    )
    assert connectivity["saturation_status"] in {"saturated", "expanded"}
    assert_all_false(analysis["safety_defaults"])


def test_wave_5_markdown_contains_required_guardrail_language() -> None:
    markdown = ANALYSIS_MD.read_text(encoding="utf-8")

    assert "This evidence is not authorized for graph import or fact promotion." in markdown
    assert "Cumulative directed edges:" in markdown
    assert "Wave 5 new directed edges to target set:" in markdown
    assert "localhost" not in markdown


def test_wave_5_cumulative_corpus_records_deduped_actual_size_and_safety() -> None:
    cumulative = load_json(CUMULATIVE_CORPUS)
    analysis = load_json(ANALYSIS_JSON)

    assert cumulative["expected_total_pdfs"] == 170
    assert cumulative["pdf_count"] == analysis["cumulative_corpus"]["actual_total"]
    assert cumulative["pdf_count"] <= cumulative["expected_total_pdfs"]
    assert analysis["cumulative_corpus"] == {
        "expected_total": 170,
        "actual_total": cumulative["pdf_count"],
        "path": "artifacts/m056-bfs-graph/wave-5/cumulative-corpus.json",
    }
    assert_all_false(cumulative["safety_defaults"])


def test_atomic_write_json_uses_tmp_path(tmp_path: Path) -> None:
    analyzer = load_analyzer()
    output_path = tmp_path / "nested" / "payload.json"

    analyzer._atomic_write_json(output_path, {"safety_defaults": analyzer._safety_defaults()})

    payload = load_json(output_path)
    assert_all_false(payload["safety_defaults"])
    assert not output_path.with_suffix(".json.tmp").exists()
