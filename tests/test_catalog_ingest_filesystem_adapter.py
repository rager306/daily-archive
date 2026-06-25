from __future__ import annotations

import json
from pathlib import Path

import pytest

from research_graph.application.corpus.catalog_ingest import (
    CatalogIngestRequest,
    CatalogIngestUseCase,
)
from research_graph.domain.corpus import CatalogIngestStatus
from research_graph.infrastructure.corpus.ingestion import catalog_adapters
from research_graph.infrastructure.corpus.ingestion.catalog_adapters import (
    ArxivCatalogMetadataProvider,
    FilesystemCatalogRepository,
    M061SourceAssetStore,
    Sha256ChecksumVerifier,
)
from research_graph.infrastructure.corpus.ingestion.catalog_ingest import ApiMetrics


def _write_m061_fixture(root: Path, article_id: str = "2605.18747") -> Path:
    acquisition = root / "anchor-test" / "acquisition"
    pdf_dir = acquisition / "pdfs"
    pdf_dir.mkdir(parents=True)
    (acquisition / "selected-2hop-papers.json").write_text(
        json.dumps({"selected_arxiv_ids": [article_id]})
    )
    pdf = pdf_dir / f"{article_id}.pdf"
    pdf.write_bytes(b"%PDF-1.4 fixture")
    return pdf


def test_m061_source_asset_store_reads_membership_and_pdf_assets(tmp_path: Path) -> None:
    source_pdf = _write_m061_fixture(tmp_path / "m061")
    store = M061SourceAssetStore(tmp_path / "m061")

    assert store.selected_article_membership() == {"2605.18747": ["test"]}
    assets = store.pdf_assets_by_article()

    assert list(assets) == ["2605.18747"]
    asset = assets["2605.18747"][0]
    assert asset.article_id == "2605.18747"
    assert asset.path == source_pdf.as_posix()
    assert asset.media_type == "application/pdf"
    assert asset.size_bytes == source_pdf.stat().st_size


def test_checksum_verifier_matches_file_digest(tmp_path: Path) -> None:
    path = tmp_path / "paper.pdf"
    path.write_bytes(b"abc")

    assert Sha256ChecksumVerifier().digest(path.as_posix()) == (
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    )


def test_filesystem_catalog_repository_preserves_layout_and_article_record(tmp_path: Path) -> None:
    source_pdf = tmp_path / "source" / "2605.18747.pdf"
    source_pdf.parent.mkdir()
    source_pdf.write_bytes(b"%PDF-1.4 fixture")
    checksum = Sha256ChecksumVerifier().digest(source_pdf.as_posix())
    repository = FilesystemCatalogRepository(tmp_path / "data" / "article_catalog")
    metadata = ArxivCatalogMetadataProvider.offline().metadata_for("2605.18747")

    catalog_asset = repository.store_pdf_asset(
        source_asset=M061SourceAssetStore.source_asset_from_path("2605.18747", source_pdf),
        metadata=metadata,
        source_sha256=checksum,
    )
    repository.write_article_record(metadata, catalog_asset, ["test"])

    expected_pdf = (
        tmp_path
        / "data"
        / "article_catalog"
        / "article_catalog"
        / "arxiv"
        / "mixed-source"
        / "2605.18747"
        / "source"
        / "2605.18747.pdf"
    )
    expected_article = expected_pdf.parents[1] / "article.json"
    assert Path(catalog_asset.path) == expected_pdf
    assert catalog_asset.sha256 == checksum
    assert expected_pdf.read_bytes() == source_pdf.read_bytes()
    article = json.loads(expected_article.read_text())
    assert article["article_key"] == "2605.18747"
    assert article["source_variants"][1]["path"] == (
        "article_catalog/arxiv/mixed-source/2605.18747/source/2605.18747.pdf"
    )


def test_filesystem_catalog_repository_write_article_record_uses_catalog_helper(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source_pdf = tmp_path / "source" / "2605.18747.pdf"
    source_pdf.parent.mkdir()
    source_pdf.write_bytes(b"%PDF-1.4 fixture")
    checksum = Sha256ChecksumVerifier().digest(source_pdf.as_posix())
    repository = FilesystemCatalogRepository(tmp_path / "data" / "article_catalog")
    metadata = ArxivCatalogMetadataProvider.offline().metadata_for("2605.18747")
    catalog_asset = repository.store_pdf_asset(
        source_asset=M061SourceAssetStore.source_asset_from_path("2605.18747", source_pdf),
        metadata=metadata,
        source_sha256=checksum,
    )
    calls = []

    def fake_write_article_record(path: Path, article: dict) -> None:
        calls.append((path, article["article_key"]))
        path.write_text(json.dumps(article), encoding="utf-8")

    monkeypatch.setattr(catalog_adapters, "write_article_record", fake_write_article_record)

    repository.write_article_record(metadata, catalog_asset, ["test"])

    assert calls == [(Path(catalog_asset.path).parents[1] / "article.json", "2605.18747")]


def test_filesystem_catalog_repository_detects_existing_matching_asset(tmp_path: Path) -> None:
    source_pdf = tmp_path / "source" / "2605.18747.pdf"
    source_pdf.parent.mkdir()
    source_pdf.write_bytes(b"%PDF-1.4 fixture")
    checksum = Sha256ChecksumVerifier().digest(source_pdf.as_posix())
    repository = FilesystemCatalogRepository(tmp_path / "data" / "article_catalog")
    metadata = ArxivCatalogMetadataProvider.offline().metadata_for("2605.18747")
    source_asset = M061SourceAssetStore.source_asset_from_path("2605.18747", source_pdf)

    repository.store_pdf_asset(source_asset, metadata, checksum)

    existing = repository.existing_asset("2605.18747", checksum)
    assert existing is not None
    assert existing.article_id == "2605.18747"
    assert existing.category == "mixed-source"
    assert repository.existing_asset("2605.18747", "not-the-same") is None


def test_catalog_ingest_use_case_runs_with_filesystem_adapters_offline(tmp_path: Path) -> None:
    _write_m061_fixture(tmp_path / "m061")
    metrics = ApiMetrics()
    source_assets = M061SourceAssetStore(tmp_path / "m061")
    metadata_provider = ArxivCatalogMetadataProvider.offline(metrics=metrics)
    repository = FilesystemCatalogRepository(tmp_path / "data" / "article_catalog")

    result = CatalogIngestUseCase(
        source_assets=source_assets,
        metadata_provider=metadata_provider,
        checksum_verifier=Sha256ChecksumVerifier(),
        catalog_repository=repository,
    ).run(CatalogIngestRequest(update_index=False))

    assert result.succeeded is True
    assert result.status_counts == {CatalogIngestStatus.INGESTED.value: 1}
    assert result.fallback_count == 1
    assert result.selected_total == 1
    assert result.discovered_pdf_total == 1
    assert result.unique_article_ids == 1
    assert result.before_catalog_pdf_count == 0
    assert result.after_catalog_pdf_count == 1
    assert metrics.requests_made == 0
    assert metrics.failures == 1

    rerun = CatalogIngestUseCase(
        source_assets=source_assets,
        metadata_provider=metadata_provider,
        checksum_verifier=Sha256ChecksumVerifier(),
        catalog_repository=repository,
    ).run(CatalogIngestRequest(update_index=False))
    assert rerun.status_counts == {CatalogIngestStatus.SKIPPED.value: 1}
    assert rerun.before_catalog_pdf_count == 1
    assert rerun.after_catalog_pdf_count == 1
