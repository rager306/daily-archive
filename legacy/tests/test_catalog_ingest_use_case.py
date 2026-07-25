from __future__ import annotations

import ast
from pathlib import Path

from research_graph.application.corpus.catalog_ingest import (
    CatalogIngestRequest,
    CatalogIngestUseCase,
    CatalogRepositoryPort,
    ChecksumVerifierPort,
    MetadataProviderPort,
    SourceAssetStorePort,
)
from research_graph.domain.corpus import (
    CatalogAsset,
    CatalogIngestStatus,
    CatalogMetadata,
    SourceAsset,
)

REPO_ROOT = Path(__file__).resolve().parents[1]
APPLICATION_MODULE = REPO_ROOT / "src" / "research_graph" / "application" / "corpus" / "catalog_ingest.py"
DOMAIN_MODULE = REPO_ROOT / "src" / "research_graph" / "domain" / "corpus.py"


class FakeSourceAssetStore:
    def __init__(self) -> None:
        self.membership = {
            "2605.18747": ["anchor-a"],
            "2605.29548": ["anchor-a", "anchor-b"],
        }
        self.assets = {
            "2605.18747": [
                SourceAsset(
                    article_id="2605.18747",
                    path="fixtures/source/2605.18747.pdf",
                    media_type="application/pdf",
                )
            ],
            "2605.29548": [
                SourceAsset(
                    article_id="2605.29548",
                    path="fixtures/source/2605.29548.pdf",
                    media_type="application/pdf",
                )
            ],
        }

    def selected_article_membership(self) -> dict[str, list[str]]:
        return self.membership

    def pdf_assets_by_article(self) -> dict[str, list[SourceAsset]]:
        return self.assets


class FakeMetadataProvider:
    def metadata_for(self, article_id: str) -> CatalogMetadata:
        return CatalogMetadata(
            article_id=article_id,
            category="cs-cl" if article_id == "2605.18747" else "mixed-source",
            title=f"Title for {article_id}",
            source="fixture",
            fallback=article_id == "2605.29548",
        )


class FakeChecksumVerifier:
    def digest(self, path: str) -> str:
        return f"sha256:{Path(path).name}"


class FakeCatalogRepository:
    def __init__(self) -> None:
        self.records = []
        self.index_updated = False

    def count_pdf_assets(self) -> int:
        return len(self.records)

    def existing_asset(self, article_id: str, source_sha256: str) -> CatalogAsset | None:
        return None

    def store_pdf_asset(
        self,
        source_asset: SourceAsset,
        metadata: CatalogMetadata,
        source_sha256: str,
    ) -> CatalogAsset:
        return CatalogAsset(
            article_id=source_asset.article_id,
            path=f"catalog/arxiv/{metadata.category}/{source_asset.article_id}/source/{source_asset.article_id}.pdf",
            sha256=source_sha256,
            category=metadata.category,
        )

    def write_article_record(
        self,
        metadata: CatalogMetadata,
        catalog_asset: CatalogAsset,
        anchor_ids: list[str],
    ) -> None:
        self.records.append((metadata, catalog_asset, anchor_ids))

    def update_index(self) -> int | None:
        self.index_updated = True
        return len(self.records)


class FakeEventSink:
    def __init__(self) -> None:
        self.events = []

    def emit(self, event: dict[str, object]) -> None:
        self.events.append(event)


def test_ports_are_runtime_checkable_with_fake_adapters() -> None:
    assert isinstance(FakeSourceAssetStore(), SourceAssetStorePort)
    assert isinstance(FakeMetadataProvider(), MetadataProviderPort)
    assert isinstance(FakeChecksumVerifier(), ChecksumVerifierPort)
    assert isinstance(FakeCatalogRepository(), CatalogRepositoryPort)


def test_catalog_ingest_use_case_ingests_with_counts_and_events() -> None:
    repository = FakeCatalogRepository()
    events = FakeEventSink()
    use_case = CatalogIngestUseCase(
        source_assets=FakeSourceAssetStore(),
        metadata_provider=FakeMetadataProvider(),
        checksum_verifier=FakeChecksumVerifier(),
        catalog_repository=repository,
        event_sink=events,
    )

    result = use_case.run(CatalogIngestRequest(update_index=True))

    assert result.selected_total == 3
    assert result.discovered_pdf_total == 2
    assert result.unique_article_ids == 2
    assert result.before_catalog_pdf_count == 0
    assert result.after_catalog_pdf_count == 2
    assert result.index_updated is True
    assert result.index_entries == 2
    assert result.status_counts == {CatalogIngestStatus.INGESTED.value: 2}
    assert result.fallback_count == 1
    assert result.failures == []
    assert len(events.events) == 4
    assert events.events[0]["event"] == "catalog_ingest.started"
    assert events.events[-1]["event"] == "catalog_ingest.completed"


def test_catalog_ingest_use_case_reports_missing_pdf_without_payload_dump() -> None:
    source_store = FakeSourceAssetStore()
    source_store.assets.pop("2605.29548")
    use_case = CatalogIngestUseCase(
        source_assets=source_store,
        metadata_provider=FakeMetadataProvider(),
        checksum_verifier=FakeChecksumVerifier(),
        catalog_repository=FakeCatalogRepository(),
    )

    result = use_case.run(CatalogIngestRequest(update_index=False))

    assert result.status_counts == {CatalogIngestStatus.INGESTED.value: 1}
    assert len(result.failures) == 1
    failure = result.failures[0]
    assert failure.article_id == "2605.29548"
    assert failure.phase == "source_asset_lookup"
    assert failure.reason == "missing_pdf_asset"
    assert failure.path is None
    assert "PDF" not in failure.message


def test_catalog_ingest_use_case_skips_existing_matching_asset() -> None:
    class ExistingRepository(FakeCatalogRepository):
        def existing_asset(self, article_id: str, source_sha256: str) -> CatalogAsset | None:
            if article_id == "2605.18747":
                return CatalogAsset(
                    article_id=article_id,
                    path="catalog/arxiv/cs-cl/2605.18747/source/2605.18747.pdf",
                    sha256=source_sha256,
                    category="cs-cl",
                )
            return None

    result = CatalogIngestUseCase(
        source_assets=FakeSourceAssetStore(),
        metadata_provider=FakeMetadataProvider(),
        checksum_verifier=FakeChecksumVerifier(),
        catalog_repository=ExistingRepository(),
    ).run(CatalogIngestRequest(update_index=False))

    assert result.status_counts == {
        CatalogIngestStatus.SKIPPED.value: 1,
        CatalogIngestStatus.INGESTED.value: 1,
    }


def test_application_and_domain_modules_do_not_import_infrastructure() -> None:
    for module_path in (APPLICATION_MODULE, DOMAIN_MODULE):
        tree = ast.parse(module_path.read_text())
        imports = []
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                imports.extend(alias.name for alias in node.names)
            elif isinstance(node, ast.ImportFrom) and node.module:
                imports.append(node.module)
        assert not any("infrastructure" in name for name in imports), imports
