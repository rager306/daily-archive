"""Tests for M055deep S04 20-PDF GROBID fulltext and OpenDataLoader benchmark artifacts."""

from __future__ import annotations

import json
from collections import Counter
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
BENCHMARK_DIR = ROOT / "artifacts" / "m055deep-parser-benchmark"
MANIFEST_PATH = BENCHMARK_DIR / "corpus-manifest-20.json"
GROBID_DIR = BENCHMARK_DIR / "grobid-fulltext-20"
OPENDATALOADER_DIR = BENCHMARK_DIR / "opendataloader-20"
SAFETY_KEYS = {
    "graph_import_allowed",
    "graphdb_written",
    "ladybugdb_written",
    "production_import_attempted",
    "import_eligible",
}
GROBID_REQUIRED_PACKET_FIELDS = {
    "status",
    "tei_size_bytes",
    "ref_count",
    "bibl_count",
    "body_element_count",
    "equation_count",
    "figure_count",
    "sections",
    "safety_defaults",
}
OPENDATALOADER_REQUIRED_PACKET_FIELDS = {
    "status",
    "markdown_size_bytes",
    "table_count",
    "image_count",
    "section_count",
    "page_count",
    "bounding_box_count",
    "safety_defaults",
}


def read_json(path: Path) -> dict[str, Any]:
    assert path.exists(), f"missing JSON artifact: {path}"
    return json.loads(path.read_text(encoding="utf-8"))


def manifest_ids() -> set[str]:
    manifest = read_json(MANIFEST_PATH)
    return {str(entry["arxiv_id"]) for entry in manifest["pdfs"]}


def per_pdf_packets(output_dir: Path) -> list[dict[str, Any]]:
    packet_paths = sorted((output_dir / "per-pdf").glob("*.json"))
    return [read_json(path) for path in packet_paths]


def normalized_status_counts(summary: dict[str, Any]) -> Counter[str]:
    counts = Counter(summary["aggregate_counts"])
    if "opendataloader_unavailable" in counts:
        counts["blocked"] += counts.pop("opendataloader_unavailable")
    return counts


def assert_safety_defaults_false(packet: dict[str, Any]) -> None:
    safety_defaults = packet.get("safety_defaults")
    assert set(safety_defaults) == SAFETY_KEYS
    assert all(value is False for value in safety_defaults.values())


def test_grobid_fulltext_20_pdfs() -> None:
    summary = read_json(GROBID_DIR / "summary.json")
    packets = per_pdf_packets(GROBID_DIR)

    assert summary["total_pdfs"] == 20
    assert len(summary["packets"]) == 20
    assert len(packets) == 20
    assert {packet["arxiv_id"] for packet in packets} == manifest_ids()
    assert Counter(packet["status"] for packet in packets) == Counter(summary["aggregate_counts"])
    assert all(packet["status"] in {"success", "low_quality_source", "blocked"} for packet in packets)


def test_opendataloader_20_pdfs() -> None:
    summary = read_json(OPENDATALOADER_DIR / "summary.json")
    packets = per_pdf_packets(OPENDATALOADER_DIR)

    assert summary["total_pdfs"] == 20
    assert len(packets) == 20
    assert {packet["arxiv_id"] for packet in packets} == manifest_ids()
    assert Counter(packet["status"] for packet in packets) == Counter(summary["aggregate_counts"])
    assert all(packet["status"] in {"success", "low_quality_source", "opendataloader_unavailable"} for packet in packets)


def test_aggregate_counts() -> None:
    grobid = read_json(GROBID_DIR / "summary.json")
    opendataloader = read_json(OPENDATALOADER_DIR / "summary.json")

    assert normalized_status_counts(grobid) == Counter({"success": 20, "low_quality_source": 0, "blocked": 0})
    normalized_opendataloader = normalized_status_counts(opendataloader)
    assert set(normalized_opendataloader) == {"success", "low_quality_source", "blocked"}
    assert sum(normalized_opendataloader.values()) == 20
    assert normalized_opendataloader["blocked"] == 0
    assert normalized_opendataloader["success"] >= 1


def test_5_safety_defaults_all_false() -> None:
    summaries = [read_json(GROBID_DIR / "summary.json"), read_json(OPENDATALOADER_DIR / "summary.json")]
    packets = per_pdf_packets(GROBID_DIR) + per_pdf_packets(OPENDATALOADER_DIR)

    for summary in summaries:
        assert_safety_defaults_false(summary)
    for packet in packets:
        assert_safety_defaults_false(packet)


def test_idempotent_summary() -> None:
    grobid = read_json(GROBID_DIR / "summary.json")
    grobid_packets = per_pdf_packets(GROBID_DIR)
    opendataloader = read_json(OPENDATALOADER_DIR / "summary.json")
    opendataloader_packets = per_pdf_packets(OPENDATALOADER_DIR)

    grobid_counts = Counter({status: 0 for status in grobid["aggregate_counts"]})
    grobid_counts.update(packet["status"] for packet in grobid_packets)
    assert grobid["aggregate_counts"] == dict(grobid_counts)
    assert grobid["total_ref_count"] == sum(packet["ref_count"] for packet in grobid_packets)
    assert grobid["total_bibl_count"] == sum(packet["bibl_count"] for packet in grobid_packets)
    assert grobid["total_body_element_count"] == sum(packet["body_element_count"] for packet in grobid_packets)
    assert grobid["total_equation_count"] == sum(packet["equation_count"] for packet in grobid_packets)
    assert grobid["total_figure_count"] == sum(packet["figure_count"] for packet in grobid_packets)

    opendataloader_counts = Counter({status: 0 for status in opendataloader["aggregate_counts"]})
    opendataloader_counts.update(packet["status"] for packet in opendataloader_packets)
    assert opendataloader["aggregate_counts"] == dict(opendataloader_counts)
    assert opendataloader["total_markdown_size_bytes"] == sum(
        packet["markdown_size_bytes"] for packet in opendataloader_packets
    )
    assert opendataloader["total_table_count"] == sum(packet["table_count"] for packet in opendataloader_packets)
    assert opendataloader["total_image_count"] == sum(packet["image_count"] for packet in opendataloader_packets)
    assert opendataloader["total_section_count"] == sum(packet["section_count"] for packet in opendataloader_packets)
    assert opendataloader["total_page_count"] == sum(packet["page_count"] for packet in opendataloader_packets)
    assert opendataloader["total_bounding_box_count"] == sum(
        packet["bounding_box_count"] for packet in opendataloader_packets
    )


def test_required_per_pdf_fields() -> None:
    for packet in per_pdf_packets(GROBID_DIR):
        assert GROBID_REQUIRED_PACKET_FIELDS <= set(packet)
        assert isinstance(packet["sections"], list)
        assert packet["tei_size_bytes"] == packet["bytes"]

    for packet in per_pdf_packets(OPENDATALOADER_DIR):
        assert OPENDATALOADER_REQUIRED_PACKET_FIELDS <= set(packet)
        assert packet["markdown_size_bytes"] >= 0
        assert packet["page_count"] >= 0


def test_manifest_alignment() -> None:
    ids = manifest_ids()
    assert ids == {packet["arxiv_id"] for packet in per_pdf_packets(GROBID_DIR)}
    assert ids == {packet["arxiv_id"] for packet in per_pdf_packets(OPENDATALOADER_DIR)}
    assert len(ids) == 20
