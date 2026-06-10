"""Tests for M055deep S05 20-PDF hybrid routing."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import benchmark_m055deep_hybrid_routing_20 as hybrid20  # noqa: E402

BENCHMARK_DIR = ROOT / "artifacts" / "m055deep-parser-benchmark"
GROBID_PER_PDF = BENCHMARK_DIR / "grobid-fulltext-20" / "per-pdf"
OPENDATALOADER_PER_PDF = BENCHMARK_DIR / "opendataloader-20" / "per-pdf"
SAFETY_KEYS = {
    "graph_import_allowed",
    "graphdb_written",
    "ladybugdb_written",
    "production_import_attempted",
    "import_eligible",
}


def read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def run_routing(tmp_path: Path) -> dict[str, Any]:
    return hybrid20.compare_hybrid_routing_20(
        GROBID_PER_PDF,
        OPENDATALOADER_PER_PDF,
        tmp_path / "hybrid-routing-20",
    )


def test_20_pdfs_routing(tmp_path: Path) -> None:
    summary = run_routing(tmp_path)
    per_pdf_dir = tmp_path / "hybrid-routing-20" / "per-pdf"
    packets = sorted(per_pdf_dir.glob("*.json"))

    assert summary["schema_version"] == "m055deep-parser-benchmark.hybrid-routing-20.v1"
    assert summary["total_pdfs"] == 20
    assert len(summary["packets"]) == 20
    assert len(packets) == 20
    assert summary["aggregate_routing_recommendation"]["hybrid_percent"] == 95.0
    assert summary["aggregate_routing_recommendation"]["route_counts"] == {
        "grobid_fulltext + opendataloader_body": 19,
        "grobid_fulltext_only": 1,
    }

    sample = read_json(per_pdf_dir / "1804.02767.json")
    assert sample["grobid_metrics"]["ref_count"] == 30
    assert sample["opendataloader_metrics"]["markdown_size_bytes"] > 0
    assert set(sample["comparison_table"]) == set(hybrid20.DIMENSIONS)
    assert sample["recommended_route"]["route_type"] == "hybrid"
    assert sample["length_bucket"] == "short"

    low_quality = read_json(per_pdf_dir / "2605.28617v1.json")
    assert low_quality["opendataloader_metrics"]["low_quality_source"] is True
    assert low_quality["comparison_table"]["body_content"]["winner"] == "grobid"
    assert low_quality["recommended_route"]["route_type"] == "single-parser"
    assert low_quality["recommended_route"]["recommended_route"] == "grobid_fulltext_only"


def test_per_dimension_winners(tmp_path: Path) -> None:
    summary = run_routing(tmp_path)

    assert summary["per_dimension_winner"] == {
        "metadata": "grobid",
        "citations": "grobid",
        "body_content": "opendataloader",
        "layout": "grobid",
        "processing_time": "grobid",
        "quality": "grobid",
    }
    assert summary["dimension_winners"]["body_content"] == {"grobid": 1, "opendataloader": 19}
    assert summary["dimension_winners"]["layout"] == {"grobid": 20}
    assert summary["dimension_winners"]["quality"] == {"grobid": 20}


def test_length_bucket_patterns(tmp_path: Path) -> None:
    summary = run_routing(tmp_path)
    patterns = summary["length_bucket_patterns"]

    assert patterns["short"]["pdf_count"] == 1
    assert patterns["short"]["hybrid_percent"] == 100.0
    assert patterns["medium"]["pdf_count"] == 12
    assert patterns["medium"]["hybrid_pdf_count"] == 11
    assert patterns["medium"]["hybrid_percent"] == 91.67
    assert patterns["long"]["pdf_count"] == 7
    assert patterns["long"]["hybrid_percent"] == 100.0


def test_fulltext_vs_header_delta(tmp_path: Path) -> None:
    summary = run_routing(tmp_path)
    delta = summary["fulltext_vs_header_delta"]

    assert delta["status"] == "compared"
    assert delta["overlap_pdf_count"] == 5
    assert delta["header_only_hybrid_percent"] == 100.0
    assert delta["fulltext_overlap_hybrid_percent"] == 100.0
    assert delta["hybrid_percent_delta_points"] == 0.0
    assert delta["dimension_winner_shifts"]
    assert all("layout" in shifts for shifts in delta["dimension_winner_shifts"].values())
    assert all("quality" in shifts for shifts in delta["dimension_winner_shifts"].values())


def test_5_safety_defaults_all_false(tmp_path: Path) -> None:
    summary = run_routing(tmp_path)
    assert set(summary["safety_defaults"]) == SAFETY_KEYS
    assert all(value is False for value in summary["safety_defaults"].values())

    for packet_path in (tmp_path / "hybrid-routing-20" / "per-pdf").glob("*.json"):
        packet = read_json(packet_path)
        assert set(packet["safety_defaults"]) == SAFETY_KEYS
        assert all(value is False for value in packet["safety_defaults"].values())


def test_idempotent_summary(tmp_path: Path) -> None:
    output_dir = tmp_path / "hybrid-routing-20"
    first = hybrid20.compare_hybrid_routing_20(GROBID_PER_PDF, OPENDATALOADER_PER_PDF, output_dir)
    second = hybrid20.compare_hybrid_routing_20(GROBID_PER_PDF, OPENDATALOADER_PER_PDF, output_dir)

    def normalize(summary: dict[str, Any]) -> dict[str, Any]:
        normalized = json.loads(json.dumps(summary))
        normalized.pop("generated_at", None)
        return normalized

    assert normalize(first) == normalize(second)
    assert normalize(read_json(output_dir / "summary.json")) == normalize(second)
