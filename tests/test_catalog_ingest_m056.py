"""Tests for M056 cumulative corpus loader (M121 S01)."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from research_graph.infrastructure.corpus.ingestion.catalog_ingest import (
    M056_CUMULATIVE_CORPUS_PATH_DEFAULT,
    CumulativePdfRecord,
    Sha256Mismatch,
    load_m056_corpus,
    sha256_file,
    verify_m056_sha256,
)

REPO_ROOT = Path("/root/daily-archive")


def test_m056_corpus_path_default() -> None:
    assert M056_CUMULATIVE_CORPUS_PATH_DEFAULT == Path(
        "artifacts/m056-bfs-graph/cumulative-corpus.json"
    )


def test_load_m056_corpus_returns_166_records() -> None:
    records = load_m056_corpus(repo_root=REPO_ROOT)
    assert len(records) == 166


def test_load_m056_corpus_record_fields() -> None:
    records = load_m056_corpus(repo_root=REPO_ROOT)
    sample = records["1703.04247"]
    assert isinstance(sample, CumulativePdfRecord)
    assert sample.arxiv_id == "1703.04247"
    assert sample.category == "mixed-source"
    assert sample.size_bytes > 0
    assert sample.pages_estimate > 0
    assert len(sample.sha256) == 64
    assert sample.source_milestone in {
        "wave-1",
        "wave-2",
        "wave-3",
        "wave-4",
        "wave-5",
        "wave-6",
        "anchor",
        "pre-existing",
    }


def test_load_m056_corpus_all_categories_distributed() -> None:
    records = load_m056_corpus(repo_root=REPO_ROOT)
    categories = {r.category for r in records.values()}
    assert "cs-cl" in categories
    assert "cs-lg" in categories
    assert "cs-ai" in categories
    assert "mixed-source" in categories


def test_load_m056_corpus_missing_path(tmp_path: Path) -> None:
    """If cumulative-corpus.json points to a missing PDF, raise FileNotFoundError."""
    bad_corpus = tmp_path / "bad-corpus.json"
    bad_corpus.write_text(
        json.dumps(
            {
                "pdf_count": 1,
                "pdfs": [
                    {
                        "arxiv_id": "9999.9999",
                        "path": "data/article_catalog/article_catalog/arxiv/cs-lg/9999.9999/source/9999.9999.pdf",
                        "sha256": "0" * 64,
                        "size_bytes": 100,
                        "pages_estimate": 1,
                        "source_milestone": "wave-1",
                    }
                ],
            }
        )
    )
    with pytest.raises(FileNotFoundError, match="missing PDFs"):
        load_m056_corpus(bad_corpus, repo_root=tmp_path)


def test_load_m056_corpus_missing_corpus_file(tmp_path: Path) -> None:
    with pytest.raises(FileNotFoundError, match="not found"):
        load_m056_corpus(tmp_path / "nonexistent.json")


def test_load_m056_corpus_empty_pdfs(tmp_path: Path) -> None:
    empty = tmp_path / "empty.json"
    empty.write_text(json.dumps({"pdf_count": 0, "pdfs": []}))
    with pytest.raises(ValueError, match="empty"):
        load_m056_corpus(empty)


def test_load_m056_corpus_skips_non_conforming_paths(tmp_path: Path) -> None:
    """Entries with paths not containing 'arxiv' are skipped silently."""
    mixed_corpus = tmp_path / "mixed.json"
    (tmp_path / "data" / "article_catalog" / "article_catalog" / "arxiv" / "cs-lg").mkdir(
        parents=True
    )
    (
        tmp_path / "data" / "article_catalog" / "article_catalog" / "arxiv" / "cs-lg" / "2605.18747"
    ).mkdir()
    (
        tmp_path
        / "data"
        / "article_catalog"
        / "article_catalog"
        / "arxiv"
        / "cs-lg"
        / "2605.18747"
        / "source"
    ).mkdir()
    (
        tmp_path
        / "data"
        / "article_catalog"
        / "article_catalog"
        / "arxiv"
        / "cs-lg"
        / "2605.18747"
        / "source"
        / "2605.18747.pdf"
    ).write_bytes(b"%PDF-fake")
    mixed_corpus.write_text(
        json.dumps(
            {
                "pdf_count": 2,
                "pdfs": [
                    {
                        "arxiv_id": "2605.18747",
                        "path": str(
                            tmp_path
                            / "data/article_catalog/article_catalog/arxiv/cs-lg/2605.18747/source/2605.18747.pdf"
                        ),
                        "sha256": sha256_file(
                            tmp_path
                            / "data/article_catalog/article_catalog/arxiv/cs-lg/2605.18747/source/2605.18747.pdf"
                        ),
                        "size_bytes": 100,
                        "pages_estimate": 1,
                        "source_milestone": "wave-1",
                    },
                    {
                        "arxiv_id": "skip_me",
                        "path": "some/other/path/skip_me.pdf",
                        "sha256": "0" * 64,
                        "size_bytes": 100,
                        "pages_estimate": 1,
                        "source_milestone": "wave-1",
                    },
                ],
            }
        )
    )
    records = load_m056_corpus(mixed_corpus, repo_root=tmp_path)
    assert len(records) == 1
    assert "2605.18747" in records


def test_verify_m056_sha256_all_match() -> None:
    """166 PDFs all have matching SHA256."""
    records = load_m056_corpus(repo_root=REPO_ROOT)
    mismatches = verify_m056_sha256(records)
    assert mismatches == []


def test_verify_m056_sha256_detects_mismatch(tmp_path: Path) -> None:
    """If a PDF is corrupted (wrong content), verify_m056_sha256 detects mismatch."""
    # create a PDF whose sha256 doesn't match declared sha256
    pdf = tmp_path / "fake.pdf"
    pdf.write_bytes(b"%PDF-1.4 different content")
    record = CumulativePdfRecord(
        arxiv_id="2605.18747",
        pdf_path=pdf,
        sha256="0" * 64,
        size_bytes=pdf.stat().st_size,
        pages_estimate=1,
        source_milestone="wave-1",
        category="cs-lg",
    )
    mismatches = verify_m056_sha256({"2605.18747": record})
    assert len(mismatches) == 1
    m = mismatches[0]
    assert isinstance(m, Sha256Mismatch)
    assert m.arxiv_id == "2605.18747"
    assert m.expected_sha256 == "0" * 64
    assert m.actual_sha256 != "0" * 64


def test_verify_m056_sha256_empty_records() -> None:
    assert verify_m056_sha256({}) == []


def test_cumulative_pdf_record_is_frozen() -> None:
    record = CumulativePdfRecord(
        arxiv_id="x",
        pdf_path=Path("/tmp"),
        sha256="0" * 64,
        size_bytes=0,
        pages_estimate=0,
        source_milestone="w",
        category="c",
    )
    import dataclasses

    with pytest.raises(dataclasses.FrozenInstanceError):
        record.arxiv_id = "y"  # type: ignore[misc]
