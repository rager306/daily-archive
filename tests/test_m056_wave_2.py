"""Artifact tests for M056-lchpnp S02 Wave 2 acquisition and analysis."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WAVE_1_DIR = ROOT / "artifacts" / "m056-bfs-graph" / "wave-1"
WAVE_2_DIR = ROOT / "artifacts" / "m056-bfs-graph" / "wave-2"
EXISTING_CORPUS = ROOT / "artifacts" / "m055deep-parser-benchmark" / "corpus-manifest-20.json"
ACQUISITION_LOG = WAVE_2_DIR / "acquisition-log.json"
CORPUS_MANIFEST = WAVE_2_DIR / "corpus-manifest.json"
ANALYSIS_JSON = WAVE_2_DIR / "analysis.json"
ANALYSIS_MD = WAVE_2_DIR / "analysis.md"
CUMULATIVE_CORPUS = WAVE_2_DIR / "cumulative-corpus.json"
GROBID_DIR = WAVE_2_DIR / "grobid-fulltext"
OPENDATALOADER_DIR = WAVE_2_DIR / "opendataloader"


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def assert_all_false(value: object) -> None:
    assert isinstance(value, dict)
    assert value
    assert all(item is False for item in value.values())


def acquired_ids() -> list[str]:
    manifest = load_json(CORPUS_MANIFEST)
    return [entry["arxiv_id"] for entry in manifest["pdfs"]]


def test_acquisition_min_25() -> None:
    log = load_json(ACQUISITION_LOG)
    assert log["requested_count"] == 30
    assert log["success_count"] >= 25
    assert log["success_count"] == 30
    assert log["blocked_count"] == 0
    assert log["network_error_count"] == 0
    assert log["success_count"] == len(
        [entry for entry in log["entries"] if entry["status"] == "acquired"]
    )


def test_30_grobid_packets() -> None:
    ids = acquired_ids()
    packet_paths = sorted((GROBID_DIR / "per-pdf").glob("*.json"))
    assert len(ids) == 30
    assert len(packet_paths) == 30
    assert {path.stem for path in packet_paths} == set(ids)
    summary = load_json(GROBID_DIR / "summary.json")
    assert summary["aggregate_counts"]["success"] == 30
    assert summary["grobid_url"] == "http://127.0.0.1:8070"


def test_30_opendataloader_packets() -> None:
    ids = acquired_ids()
    packet_paths = sorted((OPENDATALOADER_DIR / "per-pdf").glob("*.json"))
    assert len(ids) == 30
    assert len(packet_paths) == 30
    assert {path.stem for path in packet_paths} == set(ids)
    summary = load_json(OPENDATALOADER_DIR / "summary.json")
    assert sum(summary["aggregate_counts"].values()) == 30
    assert summary["aggregate_counts"]["success"] >= 25


def test_edge_saturation_tracking() -> None:
    analysis = load_json(ANALYSIS_JSON)
    connectivity = analysis["connectivity"]
    assert connectivity["wave_1_edge_count"] == 3
    assert connectivity["wave_2_new_edge_count"] >= 0
    assert connectivity["cumulative_edge_count"] >= 3
    assert connectivity["cumulative_edge_count"] == len(connectivity["cumulative_edges"])
    assert connectivity["saturation_status"] in {"saturated", "growing"}
    assert (
        connectivity["connectivity_gain_delta_vs_wave_1"]
        == connectivity["wave_2_new_edge_count"] - connectivity["wave_1_edge_count"]
    )


def test_cumulative_corpus_80() -> None:
    existing = load_json(EXISTING_CORPUS)
    wave_1 = load_json(WAVE_1_DIR / "corpus-manifest.json")
    wave_2 = load_json(CORPUS_MANIFEST)
    cumulative = load_json(CUMULATIVE_CORPUS)
    assert existing["actual_total"] == 20
    assert wave_1["pdf_count"] == 30
    assert wave_2["pdf_count"] == 30
    assert cumulative["expected_total"] == 80
    assert cumulative["actual_total"] == 80
    assert len(cumulative["pdfs"]) == 80


def test_5_safety_defaults() -> None:
    log = load_json(ACQUISITION_LOG)
    manifest = load_json(CORPUS_MANIFEST)
    analysis = load_json(ANALYSIS_JSON)
    cumulative = load_json(CUMULATIVE_CORPUS)
    for artifact in (log, manifest, analysis, cumulative):
        assert_all_false(artifact["safety_defaults"])

    for parser_dir in (GROBID_DIR, OPENDATALOADER_DIR):
        for packet_path in sorted((parser_dir / "per-pdf").glob("*.json")):
            assert_all_false(load_json(packet_path)["safety_defaults"])

    markdown = ANALYSIS_MD.read_text(encoding="utf-8")
    assert "is not authorized" in markdown
    disallowed_host = "local" + "host"
    assert disallowed_host not in markdown


def test_wave_2_analysis_parser_quality_and_self_citation() -> None:
    analysis = load_json(ANALYSIS_JSON)
    parser = analysis["parser_quality"]
    self_cluster = analysis["self_citation_cluster"]
    assert parser["grobid_packet_count"] == 30
    assert parser["opendataloader_packet_count"] == 30
    assert parser["grobid_success_count"] == 30
    assert parser["opendataloader_success_count"] >= 25
    assert parser["all_packet_safety_defaults_false"] is True
    assert self_cluster["cumulative_wave_pdf_count"] == 60
    assert 0 <= self_cluster["percent"] <= 100
    assert isinstance(self_cluster["first_authors"], dict)
