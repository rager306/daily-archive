"""Tests for M055 parser benchmark S04 hybrid routing."""

from __future__ import annotations

import copy
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import benchmark_m055_hybrid_routing as hybrid  # noqa: E402

SAFETY_KEYS = {
    "graph_import_allowed",
    "graphdb_written",
    "ladybugdb_written",
    "production_import_attempted",
    "import_eligible",
}


def grobid_packet(
    arxiv_id: str = "1804.02767",
    *,
    title: bool = True,
    authors: int = 2,
    abstract: bool = True,
    refs: int = 1,
    bibls: int = 1,
    body_elements: int = 0,
    low_quality: bool = True,
    duration_ms: int = 1000,
) -> dict[str, Any]:
    return {
        "schema_version": "m055-parser-benchmark.grobid-only.v1",
        "arxiv_id": arxiv_id,
        "article_key": arxiv_id,
        "category": "cs-cv",
        "pdf_path": f"data/{arxiv_id}.pdf",
        "manifest_sha256": "abc123",
        "status": "low_quality_source" if low_quality else "success",
        "low_quality_source": low_quality,
        "duration_ms": duration_ms,
        "bytes": 3029,
        "tei_size_bytes": 3029,
        "header_title_present": title,
        "header_author_count": authors,
        "abstract_present": abstract,
        "ref_count": refs,
        "bibl_count": bibls,
        "body_element_count": body_elements,
        "safety_defaults": dict.fromkeys(SAFETY_KEYS, False),
    }


def opendataloader_packet(
    arxiv_id: str = "1804.02767",
    *,
    markdown_size: int = 24_000,
    tables: int = 4,
    images: int = 2,
    sections: int = 12,
    pages: int = 6,
    boxes: int = 300,
    low_quality: bool = False,
    duration_ms: int = 1500,
) -> dict[str, Any]:
    return {
        "schema_version": "m055-parser-benchmark.opendataloader-only.v1",
        "arxiv_id": arxiv_id,
        "article_key": arxiv_id,
        "category": "cs-cv",
        "pdf_path": f"data/{arxiv_id}.pdf",
        "manifest_sha256": "abc123",
        "status": "success" if not low_quality else "low_quality_source",
        "low_quality_source": low_quality,
        "duration_ms": duration_ms,
        "bytes": markdown_size,
        "markdown_size_bytes": markdown_size,
        "section_count": sections,
        "page_count": pages,
        "table_count": tables,
        "image_count": images,
        "bounding_box_count": boxes,
        "safety_defaults": dict.fromkeys(SAFETY_KEYS, False),
    }


def normalize_generated_at(payload: Any) -> Any:
    if isinstance(payload, dict):
        return {
            key: "<generated>" if key == "generated_at" else normalize_generated_at(value)
            for key, value in payload.items()
        }
    if isinstance(payload, list):
        return [normalize_generated_at(value) for value in payload]
    return payload


def write_packet_pair(
    root: Path,
    arxiv_id: str,
    *,
    grobid_overrides: dict[str, Any] | None = None,
    opendl_overrides: dict[str, Any] | None = None,
) -> tuple[Path, Path]:
    grobid_dir = root / "grobid" / "per-pdf"
    opendl_dir = root / "opendl" / "per-pdf"
    grobid_dir.mkdir(parents=True, exist_ok=True)
    opendl_dir.mkdir(parents=True, exist_ok=True)
    g_packet = grobid_packet(arxiv_id)
    o_packet = opendataloader_packet(arxiv_id)
    if grobid_overrides:
        g_packet.update(grobid_overrides)
    if opendl_overrides:
        o_packet.update(opendl_overrides)
    (grobid_dir / f"{arxiv_id}.json").write_text(json.dumps(g_packet), encoding="utf-8")
    (opendl_dir / f"{arxiv_id}.json").write_text(json.dumps(o_packet), encoding="utf-8")
    return grobid_dir, opendl_dir


def test_compare_dimensions_routes_each_dimension_to_winner() -> None:
    comparison = hybrid._compare_dimensions(grobid_packet(), opendataloader_packet())

    assert comparison["metadata"]["winner"] == "grobid"
    assert comparison["citations"]["winner"] == "grobid"
    assert comparison["body_content"]["winner"] == "opendataloader"
    assert comparison["layout"]["winner"] == "opendataloader"
    assert comparison["processing_time"]["winner"] == "grobid"
    assert comparison["quality"]["winner"] == "opendataloader"


def test_propose_route_returns_hybrid_when_dimensions_split() -> None:
    comparison = hybrid._compare_dimensions(grobid_packet(), opendataloader_packet())

    route = hybrid._propose_route(comparison)

    assert route["hybrid_route"] == "grobid_header + opendataloader_body"
    assert route["confidence"] == "high"
    assert route["use_grobid_for"] == ["metadata", "citations"]
    assert route["use_opendataloader_for"] == ["body_content", "layout"]
    assert route["diagnostic_winners"] == {
        "processing_time": "grobid",
        "quality": "opendataloader",
    }


def test_identify_residual_gaps_when_parsers_do_not_close_body_or_layout() -> None:
    comparison = hybrid._compare_dimensions(
        grobid_packet(title=False, authors=0, abstract=False, refs=0, bibls=0, body_elements=0),
        opendataloader_packet(markdown_size=0, tables=0, images=0, sections=0, pages=0, boxes=0, low_quality=True),
    )

    gaps = hybrid._identify_residual_gaps(comparison)
    gap_names = {gap["gap"] for gap in gaps}

    assert "ocr_required_for_scanned_or_empty_pdf" in gap_names
    assert "math_equation_or_dense_body_extraction" in gap_names


def test_hybrid_routing_aggregate_writes_per_pdf_recommendations(tmp_path: Path) -> None:
    grobid_dir = opendl_dir = None
    for arxiv_id in ["1804.02767", "2108.12409"]:
        grobid_dir, opendl_dir = write_packet_pair(tmp_path, arxiv_id)
    assert grobid_dir is not None
    assert opendl_dir is not None

    summary = hybrid.compare_hybrid_routing(grobid_dir, opendl_dir, tmp_path / "hybrid")

    assert summary["total_pdfs"] == 2
    assert summary["aggregate_routing_recommendation"]["hybrid_pdf_count"] == 2
    assert summary["aggregate_routing_recommendation"]["hybrid_percent"] == 100.0
    assert summary["aggregate_routing_recommendation"]["route_counts"] == {
        "grobid_header + opendataloader_body": 2
    }
    for arxiv_id in ["1804.02767", "2108.12409"]:
        packet_path = tmp_path / "hybrid" / "per-pdf" / f"{arxiv_id}.json"
        packet = json.loads(packet_path.read_text(encoding="utf-8"))
        assert packet["recommended_route"]["hybrid_route"] == "grobid_header + opendataloader_body"
        assert packet["grobid_metrics"]["ref_count"] == 1
        assert packet["opendataloader_metrics"]["markdown_size_bytes"] == 24_000


def test_5_safety_defaults_all_false(tmp_path: Path) -> None:
    grobid_dir, opendl_dir = write_packet_pair(tmp_path, "1804.02767")

    summary = hybrid.compare_hybrid_routing(grobid_dir, opendl_dir, tmp_path / "hybrid")
    packet = json.loads((tmp_path / "hybrid" / "per-pdf" / "1804.02767.json").read_text(encoding="utf-8"))

    assert set(summary["safety_defaults"]) == SAFETY_KEYS
    assert all(value is False for value in summary["safety_defaults"].values())
    assert set(packet["safety_defaults"]) == SAFETY_KEYS
    assert all(value is False for value in packet["safety_defaults"].values())


def test_idempotent_summary_modulo_generated_at(tmp_path: Path) -> None:
    grobid_dir, opendl_dir = write_packet_pair(tmp_path, "1804.02767")
    output_dir = tmp_path / "hybrid"

    first = hybrid.compare_hybrid_routing(grobid_dir, opendl_dir, output_dir)
    first_file = json.loads((output_dir / "summary.json").read_text(encoding="utf-8"))
    second = hybrid.compare_hybrid_routing(grobid_dir, opendl_dir, output_dir)
    second_file = json.loads((output_dir / "summary.json").read_text(encoding="utf-8"))

    assert normalize_generated_at(first) == normalize_generated_at(second)
    assert normalize_generated_at(first_file) == normalize_generated_at(second_file)


def test_load_packets_rejects_duplicate_arxiv_ids(tmp_path: Path) -> None:
    per_pdf_dir = tmp_path / "per-pdf"
    per_pdf_dir.mkdir()
    packet = grobid_packet("1804.02767")
    (per_pdf_dir / "a.json").write_text(json.dumps(packet), encoding="utf-8")
    duplicated = copy.deepcopy(packet)
    duplicated["article_key"] = "still-duplicate"
    (per_pdf_dir / "b.json").write_text(json.dumps(duplicated), encoding="utf-8")

    try:
        hybrid._load_packets(per_pdf_dir)
    except ValueError as exc:
        assert "Duplicate arxiv_id" in str(exc)
    else:  # pragma: no cover - clearer assertion message than pytest.raises here
        raise AssertionError("Expected duplicate arxiv_id ValueError")
