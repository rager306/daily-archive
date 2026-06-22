"""Tests for M055 parser benchmark S05 report and ADR deliverables."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import render_m055_report as report  # noqa: E402

BENCHMARK_DIR = ROOT / "artifacts" / "m055-parser-benchmark"
ADR_008 = ROOT / "doc" / "adr" / "ADR-008-hybrid-parser-architecture.md"
ADR_INDEX = ROOT / "doc" / "adr" / "ADR-INDEX.md"
SAFETY_KEYS = {
    "graph_import_allowed",
    "graphdb_written",
    "ladybugdb_written",
    "production_import_attempted",
    "import_eligible",
}
ARXIV_IDS = {"1804.02767", "2108.12409", "2109.10862", "2111.00396", "2203.14465"}


def render_to_tmp(tmp_path: Path) -> str:
    output = tmp_path / "REPORT.md"
    text = report.render_report(BENCHMARK_DIR, output)
    assert output.exists()
    assert output.read_text(encoding="utf-8") == text
    return text


def test_report_contains_executive_summary(tmp_path: Path) -> None:
    text = render_to_tmp(tmp_path)

    assert "## Executive Summary" in text
    assert "100% hybrid recommendation" in text
    assert "5 PDFs" in text
    assert "6 dimensions" in text
    assert "grobid_header + opendataloader_body" in text
    assert "m055-parser-benchmark-report.v1" in text


def test_report_contains_per_pdf_tables(tmp_path: Path) -> None:
    text = render_to_tmp(tmp_path)

    assert (
        "| arxiv_id | category | pages | GROBID TEI metrics | OpenDataLoader md metrics | recommended route |"
        in text
    )
    for arxiv_id in ARXIV_IDS:
        assert arxiv_id in text
        assert f"### PDF {arxiv_id}" in text
    assert text.count("grobid_header + opendataloader_body") >= len(ARXIV_IDS)


def test_report_contains_safety_block(tmp_path: Path) -> None:
    text = render_to_tmp(tmp_path)

    assert "## Five-Flag Safety Defaults" in text
    for key in SAFETY_KEYS:
        assert f'"{key}": false' in text
    assert "graph import is not authorized" in text
    assert "production import is not authorized" in text
    assert "LadybugDB writes are not authorized" in text


def test_adr_008_exists_and_references_m055() -> None:
    text = ADR_008.read_text(encoding="utf-8")

    assert ADR_008.exists()
    assert "# ADR-008: Hybrid Parser Architecture" in text
    assert "**Status:** Accepted (binding)" in text
    assert "M055" in text
    assert "M033" in text
    assert "M043" in text
    assert "ADR-001" in text
    assert "GROBID" in text
    assert "OpenDataLoader" in text
    assert "100% hybrid" in text
    assert "graph import is not authorized" in text


def test_adr_index_updated() -> None:
    text = ADR_INDEX.read_text(encoding="utf-8")

    assert ADR_INDEX.exists()
    assert "ADR-008" in text
    assert "Hybrid Parser Architecture" in text
    assert "doc/adr/ADR-008-hybrid-parser-architecture.md" in text
