"""Contract tests for the S01 local full-text ingestion boundary.

These tests define the PageIndex-ready ingestion shape before implementation.
They must not fetch PDFs, call the network, or depend on live arXiv state.
"""

from __future__ import annotations

from pathlib import Path

import pytest

from arxiv_archive.full_text import FullTextSource, full_text_source_for_paper, ingest_full_text

FIXTURES_DIR = Path(__file__).parent / "fixtures" / "full_text"


def test_ingests_structured_markdown_with_provenance() -> None:
    source_path = FIXTURES_DIR / "structured_paper.md"
    source = FullTextSource(
        paper_id="2605.12345",
        source_type="markdown",
        source_path=source_path,
    )

    result = ingest_full_text(source)

    assert result.paper_id == "2605.12345"
    assert result.source_type == "markdown"
    assert result.source_path == source_path
    assert result.extraction_mode == "structured_markdown"
    assert result.fallback_reason is None
    assert result.warnings == []
    assert "Graph-Guided Retrieval for Scientific Agents" in result.text
    assert "Local markdown ingestion provides" in result.text
    assert result.provenance == {
        "paper_id": "2605.12345",
        "source_type": "markdown",
        "source_path": str(source_path),
        "extraction_mode": "structured_markdown",
    }


def test_ingests_plain_text_with_explicit_fallback_metadata() -> None:
    source_path = FIXTURES_DIR / "plain_fallback.txt"
    source = FullTextSource(
        paper_id="2605.99999",
        source_type="text",
        source_path=source_path,
    )

    result = ingest_full_text(source)

    assert result.paper_id == "2605.99999"
    assert result.source_type == "text"
    assert result.source_path == source_path
    assert result.extraction_mode == "plain_text"
    assert result.fallback_reason == "unstructured_text"
    assert result.warnings == ["source has no markdown section structure"]
    assert "plain text fallback" in result.text
    assert result.provenance["source_path"] == str(source_path)
    assert result.provenance["fallback_reason"] == "unstructured_text"


def test_missing_source_returns_typed_failure_without_empty_silent_text(tmp_path: Path) -> None:
    missing_path = tmp_path / "missing-paper.md"
    source = FullTextSource(
        paper_id="2605.missing",
        source_type="markdown",
        source_path=missing_path,
    )

    result = ingest_full_text(source)

    assert result.paper_id == "2605.missing"
    assert result.source_path == missing_path
    assert result.text == ""
    assert result.extraction_mode == "missing_source"
    assert result.fallback_reason == "source_missing"
    assert result.warnings == [f"source path does not exist: {missing_path}"]
    assert result.provenance["source_path"] == str(missing_path)
    assert result.provenance["fallback_reason"] == "source_missing"


def test_empty_or_malformed_source_returns_explicit_warning(tmp_path: Path) -> None:
    empty_path = tmp_path / "empty.md"
    empty_path.write_text("   \n\n", encoding="utf-8")
    source = FullTextSource(
        paper_id="2605.empty",
        source_type="markdown",
        source_path=empty_path,
    )

    result = ingest_full_text(source)

    assert result.paper_id == "2605.empty"
    assert result.source_path == empty_path
    assert result.text == ""
    assert result.extraction_mode == "empty_source"
    assert result.fallback_reason == "source_empty"
    assert result.warnings == ["source file is empty after trimming whitespace"]
    assert result.provenance["source_path"] == str(empty_path)
    assert result.provenance["fallback_reason"] == "source_empty"


def test_stored_paper_artifact_path_ingests_pageindex_ready_payload(tmp_path: Path) -> None:
    """Stored paper ids map to local full-text sources for S02 PageIndex consumers."""
    papers_dir = tmp_path / "papers"
    paper_dir = papers_dir / "2605.12345"
    paper_dir.mkdir(parents=True)
    full_text_path = paper_dir / "full_text.md"
    full_text_path.write_text((FIXTURES_DIR / "structured_paper.md").read_text(), encoding="utf-8")

    source = full_text_source_for_paper("2605.12345", papers_dir)
    result = ingest_full_text(source)

    assert result.paper_id == "2605.12345"
    assert result.source_path == full_text_path
    assert result.extraction_mode == "structured_markdown"
    assert result.text
    assert result.warnings == []
    assert result.fallback_reason is None
    assert result.provenance == {
        "paper_id": "2605.12345",
        "source_type": "markdown",
        "source_path": str(full_text_path),
        "extraction_mode": "structured_markdown",
    }


def test_rejects_unknown_source_type_before_parsing(tmp_path: Path) -> None:
    source_path = tmp_path / "paper.html"
    source_path.write_text("<html></html>", encoding="utf-8")
    source = FullTextSource(
        paper_id="2605.unsupported",
        source_type="html",
        source_path=source_path,
    )

    with pytest.raises(ValueError, match="unsupported full-text source type: html"):
        ingest_full_text(source)
