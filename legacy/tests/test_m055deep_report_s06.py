"""Tests for M055deep S06 report and ADR amendment artifacts."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

# pyrefly: ignore [missing-import]
import render_m055deep_report as report  # noqa: E402  # ty:ignore[unresolved-import]

BENCHMARK_DIR = ROOT / "artifacts" / "m055deep-parser-benchmark"
REPORT_PATH = BENCHMARK_DIR / "REPORT.md"
ADR_009_PATH = ROOT / "doc" / "adr" / "ADR-009-amend-hybrid-parser.md"
ADR_ACK_PATH = ROOT / "doc" / "adr" / "ADR-008-acknowledged-20-pdf.md"
ADR_INDEX_PATH = ROOT / "doc" / "adr" / "ADR-INDEX.md"
SAFETY_LINES = [
    "| `graph_import_allowed` | `false` |",
    "| `graphdb_written` | `false` |",
    "| `import_eligible` | `false` |",
    "| `ladybugdb_written` | `false` |",
    "| `production_import_attempted` | `false` |",
]


def ensure_report() -> str:
    if not REPORT_PATH.exists():
        report.render_report()
    return REPORT_PATH.read_text(encoding="utf-8")


def test_report_contains_executive_summary() -> None:
    text = ensure_report()
    assert "schema_version: m055deep-parser-benchmark-report.v1" in text
    assert "## Executive Summary" in text
    assert "GROBID fulltext dominates five dimensions" in text
    assert "OpenDataLoader remains the aggregate body-content winner" in text
    assert "Production import is not authorized" in text
    assert "```mermaid" in text
    assert len(text.splitlines()) >= 500


def test_report_contains_per_pdf_tables() -> None:
    text = ensure_report()
    assert "## Per-PDF Routing Table" in text
    assert "| # | arXiv ID | Bucket | Pages |" in text
    table_section = text.split("## Per-PDF Routing Table", 1)[1].split(
        "## Per-Dimension Winner Analysis", 1
    )[0]
    table_lines = [
        line
        for line in table_section.splitlines()
        if line.startswith("| ") and "grobid_fulltext" in line
    ]
    assert len(table_lines) == 20
    assert "2605.28617v1" in text
    assert "grobid_fulltext_only" in text


def test_report_contains_safety_block() -> None:
    text = ensure_report()
    assert "## Safety Defaults" in text
    for line in SAFETY_LINES:
        assert line in text


def test_adr_009_or_008_amendment_exists() -> None:
    assert ADR_009_PATH.exists() or ADR_ACK_PATH.exists()
    path = ADR_009_PATH if ADR_009_PATH.exists() else ADR_ACK_PATH
    text = path.read_text(encoding="utf-8")
    assert "**Status:** Accepted (binding)" in text
    assert "ADR-008" in text
    assert "```mermaid" in text
    for line in SAFETY_LINES:
        assert line in text


def test_adr_index_updated() -> None:
    text = ADR_INDEX_PATH.read_text(encoding="utf-8")
    assert "ADR-009" in text or "ADR-008 20-PDF" in text
    assert "Fulltext-Aware Hybrid Parser Routing" in text


def test_report_renderer_idempotent(tmp_path: Path) -> None:
    first = report.render_report(output_path=tmp_path / "REPORT.md")
    second = report.render_report(output_path=tmp_path / "REPORT.md")
    assert first == second
    assert "hybrid_percent" in first
