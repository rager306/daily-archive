"""Tests for M056 cumulative corpus loader (M121 S01)."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from research_graph.application.corpus.catalog_ingest import (
    CatalogIngestRequest,
    CatalogIngestUseCase,
)
from research_graph.domain.corpus import CatalogIngestStatus
from research_graph.infrastructure.corpus.ingestion.catalog_adapters import (
    M056CumulativeCorpusSourceAssetStore,
    M056FilesystemCatalogRepository,
    M056OfflineMetadataProvider,
    Sha256ChecksumVerifier,
)
from research_graph.infrastructure.corpus.ingestion.catalog_ingest import (
    M056_CUMULATIVE_CORPUS_PATH_DEFAULT,
    CumulativePdfRecord,
    SafetyOverride,
    Sha256Mismatch,
    load_m056_corpus,
    sha256_file,
    verify_m056_sha256,
)

REPO_ROOT = Path(__file__).resolve().parents[1]


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
        record.__setattr__("arxiv_id", "y")


def _write_m056_adapter_fixture(tmp_path: Path) -> tuple[Path, Path, str]:
    repo_root = tmp_path
    article_id = "2605.18747"
    pdf = (
        repo_root
        / "data"
        / "article_catalog"
        / "article_catalog"
        / "arxiv"
        / "cs-lg"
        / article_id
        / "source"
        / f"{article_id}.pdf"
    )
    pdf.parent.mkdir(parents=True)
    pdf.write_bytes(b"%PDF-1.4 fixture")
    checksum = sha256_file(pdf)
    corpus = repo_root / "cumulative-corpus.json"
    corpus.write_text(
        json.dumps(
            {
                "pdf_count": 1,
                "pdfs": [
                    {
                        "arxiv_id": article_id,
                        "path": str(pdf.relative_to(repo_root)),
                        "sha256": checksum,
                        "size_bytes": pdf.stat().st_size,
                        "pages_estimate": 7,
                        "source_milestone": "wave-1",
                    }
                ],
            }
        )
    )
    return repo_root, corpus, article_id


def test_m056_adapters_rewrite_stale_article_then_skip_matching_offline_record(
    tmp_path: Path,
) -> None:
    repo_root, corpus, article_id = _write_m056_adapter_fixture(tmp_path)
    source_assets = M056CumulativeCorpusSourceAssetStore(corpus, repo_root=repo_root)
    assert source_assets.sha256_mismatches() == []

    record = source_assets.records[article_id]
    article_path = record.pdf_path.parents[1] / "article.json"
    article_path.write_text(
        json.dumps(
            {
                "identity": {"sha256": record.sha256},
                "source_variants": [{"network_fetch_attempted": True}],
            }
        )
    )

    safety = SafetyOverride(
        external_network_authorized=False,
        reason="test offline M056 ingest",
        scope="test",
    )
    repository = M056FilesystemCatalogRepository(
        repo_root / "data" / "article_catalog",
        records=source_assets.records,
        safety_override=safety,
    )

    result = CatalogIngestUseCase(
        source_assets=source_assets,
        metadata_provider=M056OfflineMetadataProvider(source_assets.records),
        checksum_verifier=Sha256ChecksumVerifier(),
        catalog_repository=repository,
    ).run(CatalogIngestRequest(update_index=False))

    assert result.succeeded is True
    assert result.status_counts == {CatalogIngestStatus.INGESTED.value: 1}
    article = json.loads(article_path.read_text())
    assert article["identity"]["source_kind"] == "m056_cumulative_corpus_local_pdf"
    assert article["identity"]["sha256"] == record.sha256
    assert article["identity"]["pages_estimate"] == 7
    assert article["source_variants"][0]["network_fetch_attempted"] is False
    assert article["source_variants"][1]["network_fetch_attempted"] is False
    assert article["expected_profile"]["synthetic_metadata"] is True

    rerun = CatalogIngestUseCase(
        source_assets=source_assets,
        metadata_provider=M056OfflineMetadataProvider(source_assets.records),
        checksum_verifier=Sha256ChecksumVerifier(),
        catalog_repository=repository,
    ).run(CatalogIngestRequest(update_index=False))

    assert rerun.status_counts == {CatalogIngestStatus.SKIPPED.value: 1}
