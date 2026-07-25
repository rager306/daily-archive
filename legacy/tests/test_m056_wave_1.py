"""Artifact tests for M056-lchpnp S01 Wave 1 acquisition and analysis."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WAVE_DIR = ROOT / "artifacts" / "m056-bfs-graph" / "wave-1"
ACQUISITION_LOG = WAVE_DIR / "acquisition-log.json"
CORPUS_MANIFEST = WAVE_DIR / "corpus-manifest.json"
ANALYSIS_JSON = WAVE_DIR / "analysis.json"
CUMULATIVE_CORPUS = WAVE_DIR / "cumulative-corpus.json"
GROBID_DIR = WAVE_DIR / "grobid-fulltext"
OPENDATALOADER_DIR = WAVE_DIR / "opendataloader"


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def assert_all_false(value: object) -> None:
    assert isinstance(value, dict)
    assert value
    assert all(item is False for item in value.values())


def acquired_ids() -> list[str]:
    manifest = load_json(CORPUS_MANIFEST)
    return [entry["arxiv_id"] for entry in manifest["pdfs"]]


def test_acquisition_min_25_pdfs() -> None:
    log = load_json(ACQUISITION_LOG)
    assert log["requested_count"] == 30
    assert log["success_count"] >= 25
    assert log["success_count"] == len(
        [entry for entry in log["entries"] if entry["status"] == "acquired"]
    )


def test_grobid_fulltext_30_packets() -> None:
    ids = acquired_ids()
    packet_paths = sorted((GROBID_DIR / "per-pdf").glob("*.json"))
    assert len(ids) == 30
    assert len(packet_paths) == 30
    assert {path.stem for path in packet_paths} == set(ids)
    assert (GROBID_DIR / "summary.json").exists()


def test_opendataloader_30_packets() -> None:
    ids = acquired_ids()
    packet_paths = sorted((OPENDATALOADER_DIR / "per-pdf").glob("*.json"))
    assert len(ids) == 30
    assert len(packet_paths) == 30
    assert {path.stem for path in packet_paths} == set(ids)
    assert (OPENDATALOADER_DIR / "summary.json").exists()


def test_connectivity_gain_nonzero() -> None:
    analysis = load_json(ANALYSIS_JSON)
    assert analysis["connectivity"]["new_edge_count"] >= 1
    assert analysis["connectivity"]["edges"]


def test_self_citation_cluster_detection() -> None:
    analysis = load_json(ANALYSIS_JSON)
    cluster = analysis["self_citation_cluster"]
    assert cluster["wave_pdf_count"] == 30
    assert 0 <= cluster["percent"] <= 100
    assert isinstance(cluster["first_authors"], dict)


def test_cumulative_corpus_50_pdfs() -> None:
    cumulative = load_json(CUMULATIVE_CORPUS)
    assert cumulative["expected_total"] == 50
    assert cumulative["actual_total"] == 50
    assert len(cumulative["pdfs"]) == 50


def test_5_safety_defaults_all_false() -> None:
    log = load_json(ACQUISITION_LOG)
    manifest = load_json(CORPUS_MANIFEST)
    analysis = load_json(ANALYSIS_JSON)
    cumulative = load_json(CUMULATIVE_CORPUS)
    for artifact in (log, manifest, analysis, cumulative):
        assert_all_false(artifact["safety_defaults"])

    for packet_path in sorted((GROBID_DIR / "per-pdf").glob("*.json")):
        assert_all_false(load_json(packet_path)["safety_defaults"])
    for packet_path in sorted((OPENDATALOADER_DIR / "per-pdf").glob("*.json")):
        assert_all_false(load_json(packet_path)["safety_defaults"])


def test_parser_quality_counts_present() -> None:
    analysis = load_json(ANALYSIS_JSON)
    parser = analysis["parser_quality"]
    assert parser["grobid_packet_count"] == 30
    assert parser["opendataloader_packet_count"] == 30
    assert parser["all_packet_safety_defaults_false"] is True
    assert parser["grobid_success_count"] >= 1
    assert parser["opendataloader_success_count"] >= 1
