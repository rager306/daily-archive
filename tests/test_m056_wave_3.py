"""Artifact tests for M056-lchpnp S03 Wave 3 acquisition and analysis."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXISTING_CORPUS = ROOT / "artifacts" / "m055deep-parser-benchmark" / "corpus-manifest-20.json"
WAVE_1_DIR = ROOT / "artifacts" / "m056-bfs-graph" / "wave-1"
WAVE_2_DIR = ROOT / "artifacts" / "m056-bfs-graph" / "wave-2"
WAVE_3_DIR = ROOT / "artifacts" / "m056-bfs-graph" / "wave-3"
ACQUISITION_LOG = WAVE_3_DIR / "acquisition-log.json"
CORPUS_MANIFEST = WAVE_3_DIR / "corpus-manifest.json"
ANALYSIS_JSON = WAVE_3_DIR / "analysis.json"
ANALYSIS_MD = WAVE_3_DIR / "analysis.md"
CUMULATIVE_CORPUS = WAVE_3_DIR / "cumulative-corpus.json"
GROBID_DIR = WAVE_3_DIR / "grobid-fulltext"
OPENDATALOADER_DIR = WAVE_3_DIR / "opendataloader"


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def assert_all_false(value: object) -> None:
    assert isinstance(value, dict)
    assert value
    assert all(item is False for item in value.values())


def packet_paths(directory: Path) -> list[Path]:
    return sorted((directory / "per-pdf").glob("*.json"))


def edge_key(edge: dict[str, str]) -> tuple[str, str]:
    return (edge["source_arxiv_id"], edge["target_arxiv_id"])


def test_acquisition_min_25() -> None:
    log = load_json(ACQUISITION_LOG)
    manifest = load_json(CORPUS_MANIFEST)
    assert log["success_count"] >= 25
    assert log["success_count"] == 30
    assert log["blocked_count"] == 0
    assert manifest["pdf_count"] == 30
    assert len(manifest["pdfs"]) == 30
    assert_all_false(log["safety_defaults"])
    assert_all_false(manifest["safety_defaults"])


def test_30_grobid_packets() -> None:
    summary = load_json(GROBID_DIR / "summary.json")
    paths = packet_paths(GROBID_DIR)
    assert len(paths) == 30
    assert len(summary["packets"]) == 30
    assert summary["success_count"] == 30
    for path in paths:
        packet = load_json(path)
        assert packet["status"] == "success"
        assert_all_false(packet["safety_defaults"])


def test_30_opendataloader_packets() -> None:
    summary = load_json(OPENDATALOADER_DIR / "summary.json")
    paths = packet_paths(OPENDATALOADER_DIR)
    assert len(paths) == 30
    assert summary["total_pdfs"] == 30
    assert len(summary["per_pdf_statuses"]) == 30
    assert sum(summary["aggregate_counts"].values()) == 30
    for path in paths:
        packet = load_json(path)
        assert packet["status"] in {"success", "low_quality_source", "opendataloader_unavailable", "blocked"}
        assert_all_false(packet["safety_defaults"])


def test_cumulative_edges() -> None:
    analysis = load_json(ANALYSIS_JSON)
    connectivity = analysis["connectivity"]
    wave_counts = connectivity["edge_saturation_by_wave"]
    expected_count = wave_counts["wave_1"] + wave_counts["wave_2"] + wave_counts["wave_3"]
    assert wave_counts["wave_1"] == 3
    assert wave_counts["wave_2"] == 2
    assert wave_counts["wave_3"] >= 0
    assert connectivity["cumulative_edge_count"] == expected_count
    assert connectivity["cumulative_edge_count"] >= 5
    assert len({edge_key(edge) for edge in connectivity["cumulative_edges"]}) == connectivity["cumulative_edge_count"]


def test_cumulative_corpus_count() -> None:
    analysis = load_json(ANALYSIS_JSON)
    cumulative = load_json(CUMULATIVE_CORPUS)
    unique_ids = {entry["arxiv_id"] for entry in cumulative["pdfs"]}
    assert cumulative["expected_total_pdfs"] == 110
    assert cumulative["pdf_count"] == len(unique_ids)
    assert len(cumulative["pdfs"]) == len(unique_ids)
    assert cumulative["pdf_count"] == analysis["cumulative_corpus"]["actual_total"]
    assert analysis["cumulative_corpus"]["expected_total"] == 110
    assert 90 <= analysis["cumulative_corpus"]["actual_total"] <= 110
    assert_all_false(cumulative["safety_defaults"])


def test_5_safety_defaults() -> None:
    analysis = load_json(ANALYSIS_JSON)
    assert_all_false(analysis["safety_defaults"])
    assert analysis["parser_quality"]["all_packet_safety_defaults_false"] is True
    markdown = ANALYSIS_MD.read_text(encoding="utf-8")
    assert "is not authorized" in markdown
    forbidden_host_token = "local" + "host"
    assert forbidden_host_token not in markdown


def test_m050_m055deep_and_wave_1_2_regression() -> None:
    existing = load_json(EXISTING_CORPUS)
    wave_1 = load_json(WAVE_1_DIR / "analysis.json")
    wave_2 = load_json(WAVE_2_DIR / "analysis.json")
    assert len(existing["pdfs"]) == 20
    assert wave_1["connectivity"]["new_edge_count"] == 3
    assert wave_2["connectivity"]["wave_1_edge_count"] == 3
    assert wave_2["connectivity"]["wave_2_new_edge_count"] == 2
    assert wave_2["connectivity"]["cumulative_edge_count"] == 5
    assert_all_false(wave_1["safety_defaults"])
    assert_all_false(wave_2["safety_defaults"])
