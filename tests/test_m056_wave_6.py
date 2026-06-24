"""Artifact tests for M056-lchpnp S06 Wave 6 final 1-hop analysis."""

from __future__ import annotations

import json
from pathlib import Path
from types import ModuleType

from scripts import analyze_m056_wave_6 as analyzer

ROOT = Path(__file__).resolve().parents[1]
WAVE_ORDER = Path("/tmp/wave-order.json")
WAVE_5_DIR = ROOT / "artifacts" / "m056-bfs-graph" / "wave-5"
WAVE_6_DIR = ROOT / "artifacts" / "m056-bfs-graph" / "wave-6"
ACQUISITION_LOG = WAVE_6_DIR / "acquisition-log.json"
CORPUS_MANIFEST = WAVE_6_DIR / "corpus-manifest.json"
ANALYSIS_JSON = WAVE_6_DIR / "analysis.json"
ANALYSIS_MD = WAVE_6_DIR / "analysis.md"
CUMULATIVE_CORPUS = WAVE_6_DIR / "cumulative-corpus.json"
GROBID_DIR = WAVE_6_DIR / "grobid-fulltext"
OPENDATALOADER_DIR = WAVE_6_DIR / "opendataloader"
ANALYZER_SCRIPT_PATH = ROOT / "scripts" / "analyze_m056_wave_6.py"


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def assert_all_false(value: object) -> None:
    assert isinstance(value, dict)
    assert value
    assert all(item is False for item in value.values())


def load_analyzer() -> ModuleType:
    return analyzer


def test_wave_6_acquisition_matches_positions_151_to_166() -> None:
    wave_order = json.loads(WAVE_ORDER.read_text(encoding="utf-8"))
    expected_ids = wave_order[150:166]
    acquisition = load_json(ACQUISITION_LOG)
    manifest = load_json(CORPUS_MANIFEST)

    assert len(expected_ids) == 16
    assert acquisition["requested_arxiv_ids"] == expected_ids
    assert acquisition["success_count"] >= acquisition["minimum_acquired"] == 12
    assert acquisition["success_count"] == 16
    assert acquisition["status_counts"] == {"acquired": 16}
    assert manifest["pdf_count"] == 16
    assert [pdf["arxiv_id"] for pdf in manifest["pdfs"]] == expected_ids
    assert_all_false(acquisition["safety_defaults"])
    assert_all_false(manifest["safety_defaults"])


def test_wave_6_parser_packet_counts_and_safety_defaults() -> None:
    grobid_summary = load_json(GROBID_DIR / "summary.json")
    opendataloader_summary = load_json(OPENDATALOADER_DIR / "summary.json")
    grobid_packets = sorted((GROBID_DIR / "per-pdf").glob("*.json"))
    opendataloader_packets = sorted((OPENDATALOADER_DIR / "per-pdf").glob("*.json"))

    assert grobid_summary["total_pdfs"] == 16
    assert grobid_summary["success_count"] == 16
    assert opendataloader_summary["total_pdfs"] == 16
    assert opendataloader_summary["success_count"] >= 12
    assert len(grobid_packets) == 16
    assert len(opendataloader_packets) == 16
    assert_all_false(grobid_summary["safety_defaults"])
    assert_all_false(opendataloader_summary["safety_defaults"])
    for packet_path in grobid_packets + opendataloader_packets:
        assert_all_false(load_json(packet_path)["safety_defaults"])


def test_wave_6_analysis_tracks_final_saturation() -> None:
    previous = load_json(WAVE_5_DIR / "analysis.json")
    analysis = load_json(ANALYSIS_JSON)
    connectivity = analysis["connectivity"]

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
        connectivity["wave_5_new_edge_count"] == previous["connectivity"]["wave_5_new_edge_count"]
    )
    assert connectivity["wave_6_new_edge_count"] == 0
    assert (
        connectivity["cumulative_edge_count"] == previous["connectivity"]["cumulative_edge_count"]
    )
    assert connectivity["edge_saturation_by_wave"]["wave_6"] == 0
    assert connectivity["saturation_status"] == "final-saturated"


def test_wave_6_final_corpus_accounting_and_recommendation() -> None:
    analysis = load_json(ANALYSIS_JSON)
    cumulative = load_json(CUMULATIVE_CORPUS)
    final_1hop = analysis["final_1hop"]

    assert final_1hop["wave_order_entry_count"] == 166
    assert final_1hop["anchor_present_in_wave_order"] is True
    assert final_1hop["acquired_wave_entry_count"] == 166
    assert (
        final_1hop["total_unique_pdfs_with_anchor"]
        == cumulative["one_hop_unique_with_anchor_count"]
    )
    assert (
        final_1hop["evidence_corpus_unique_pdf_count"]
        == cumulative["evidence_corpus_unique_pdf_count"]
    )
    assert "2-hop needed for graph-readiness" in final_1hop["recommendation"]["decision"]
    assert "benchmark-only" in final_1hop["recommendation"]["decision"]
    assert_all_false(analysis["safety_defaults"])
    assert_all_false(cumulative["safety_defaults"])


def test_wave_6_markdown_is_safe_for_trajectory_scan() -> None:
    markdown = ANALYSIS_MD.read_text(encoding="utf-8")
    source = ANALYZER_SCRIPT_PATH.read_text(encoding="utf-8")

    assert "is not authorized" in markdown
    assert "localhost" not in markdown
    assert "localhost" not in source
    assert "Final 1-hop corpus accounting" in markdown
    assert "Final recommendation" in markdown


def test_wave_6_analyzer_atomic_write_uses_tmp_path(tmp_path: Path) -> None:
    analyzer = load_analyzer()
    output_path = tmp_path / "nested" / "payload.json"

    analyzer._atomic_write_json(output_path, {"safety_defaults": analyzer._safety_defaults()})

    payload = load_json(output_path)
    assert_all_false(payload["safety_defaults"])
    assert not output_path.with_suffix(".json.tmp").exists()
