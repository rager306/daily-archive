"""Application-layer catalog ingest use case and ports.

The use case coordinates catalog ingest policy without importing concrete
filesystem, arXiv, PDF, JSON, or graph implementations. Infrastructure adapters
implement these ports in later M122 slices.
"""

from __future__ import annotations

from collections import Counter
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from typing import Protocol, runtime_checkable

from research_graph.domain.corpus import (
    CatalogAsset,
    CatalogIngestFailure,
    CatalogIngestRecord,
    CatalogIngestStatus,
    CatalogMetadata,
    SourceAsset,
)


@dataclass(frozen=True)
class CatalogIngestRequest:
    """Application-level options for one catalog ingest run."""

    update_index: bool = True


@dataclass(frozen=True)
class CatalogIngestResult:
    """Aggregate ingest outcome with observability-friendly counts."""

    records: list[CatalogIngestRecord]
    failures: list[CatalogIngestFailure]
    selected_total: int
    discovered_pdf_total: int
    unique_article_ids: int
    before_catalog_pdf_count: int
    after_catalog_pdf_count: int
    index_updated: bool
    index_entries: int | None
    status_counts: dict[str, int] = field(default_factory=dict)
    fallback_count: int = 0

    @property
    def succeeded(self) -> bool:
        """True when no per-article failures were recorded."""

        return not self.failures


@runtime_checkable
class SourceAssetStorePort(Protocol):
    """Boundary for selected article IDs and locally available source PDFs."""

    def selected_article_membership(self) -> Mapping[str, Sequence[str]]:
        """Return article_id -> anchor IDs that selected the article."""
        ...

    def pdf_assets_by_article(self) -> Mapping[str, Sequence[SourceAsset]]:
        """Return article_id -> local PDF source assets."""
        ...


@runtime_checkable
class MetadataProviderPort(Protocol):
    """Boundary for article metadata lookup or fallback metadata."""

    def metadata_for(self, article_id: str) -> CatalogMetadata:
        """Return metadata for ``article_id`` without leaking provider details."""
        ...


@runtime_checkable
class ChecksumVerifierPort(Protocol):
    """Boundary for asset checksum calculation."""

    def digest(self, path: str) -> str:
        """Return a stable digest for the asset at ``path``."""
        ...


@runtime_checkable
class CatalogRepositoryPort(Protocol):
    """Boundary for canonical catalog persistence."""

    def count_pdf_assets(self) -> int:
        """Return the current number of catalog PDF assets."""
        ...

    def existing_asset(self, article_id: str, source_sha256: str) -> CatalogAsset | None:
        """Return an existing matching catalog asset when one is already present."""
        ...

    def store_pdf_asset(
        self,
        source_asset: SourceAsset,
        metadata: CatalogMetadata,
        source_sha256: str,
    ) -> CatalogAsset:
        """Store ``source_asset`` in the canonical catalog and return its catalog asset."""
        ...

    def write_article_record(
        self,
        metadata: CatalogMetadata,
        catalog_asset: CatalogAsset,
        anchor_ids: list[str],
    ) -> None:
        """Write or update the catalog article record for ``catalog_asset``."""
        ...

    def update_index(self) -> int | None:
        """Update the catalog index and return entry count when available."""
        ...


@runtime_checkable
class CatalogIngestEventSinkPort(Protocol):
    """Optional boundary for ingest event emission."""

    def emit(self, event: dict[str, object]) -> None:
        """Emit a compact structured event."""
        ...


class CatalogIngestUseCase:
    """Coordinate catalog ingest through application ports."""

    def __init__(
        self,
        *,
        source_assets: SourceAssetStorePort,
        metadata_provider: MetadataProviderPort,
        checksum_verifier: ChecksumVerifierPort,
        catalog_repository: CatalogRepositoryPort,
        event_sink: CatalogIngestEventSinkPort | None = None,
    ) -> None:
        self._source_assets = source_assets
        self._metadata_provider = metadata_provider
        self._checksum_verifier = checksum_verifier
        self._catalog_repository = catalog_repository
        self._event_sink = event_sink

    def run(self, request: CatalogIngestRequest | None = None) -> CatalogIngestResult:
        """Run the ingest use case and return aggregate counts plus diagnostics."""

        req = request or CatalogIngestRequest()
        membership = {
            article_id: list(anchor_ids)
            for article_id, anchor_ids in self._source_assets.selected_article_membership().items()
        }
        pdf_assets = {
            article_id: list(assets)
            for article_id, assets in self._source_assets.pdf_assets_by_article().items()
        }
        before_count = self._catalog_repository.count_pdf_assets()
        self._emit(
            "catalog_ingest.started",
            unique_article_ids=len(membership),
            selected_total=sum(len(anchor_ids) for anchor_ids in membership.values()),
        )

        records: list[CatalogIngestRecord] = []
        failures: list[CatalogIngestFailure] = []

        for article_id in sorted(membership):
            anchor_ids = membership[article_id]
            assets = pdf_assets.get(article_id, [])
            if not assets:
                failure = CatalogIngestFailure(
                    article_id=article_id,
                    phase="source_asset_lookup",
                    reason="missing_pdf_asset",
                    message="missing source asset",
                )
                failures.append(failure)
                self._emit_failure(failure)
                continue

            source_asset = sorted(assets, key=lambda asset: asset.path)[0]
            try:
                record = self._ingest_one(article_id, anchor_ids, source_asset)
            except Exception as exc:  # noqa: BLE001 - use case converts adapter failures to diagnostics
                failure = CatalogIngestFailure(
                    article_id=article_id,
                    phase="catalog_ingest",
                    reason=exc.__class__.__name__,
                    message=str(exc),
                    path=source_asset.path,
                )
                failures.append(failure)
                self._emit_failure(failure)
                continue
            records.append(record)
            self._emit(
                "catalog_ingest.article_completed",
                article_id=article_id,
                status=record.status.value,
                fallback=record.fallback,
            )

        index_entries = self._catalog_repository.update_index() if req.update_index else None
        after_count = self._catalog_repository.count_pdf_assets()
        result = CatalogIngestResult(
            records=records,
            failures=failures,
            selected_total=sum(len(anchor_ids) for anchor_ids in membership.values()),
            discovered_pdf_total=sum(len(assets) for assets in pdf_assets.values()),
            unique_article_ids=len(membership),
            before_catalog_pdf_count=before_count,
            after_catalog_pdf_count=after_count,
            index_updated=req.update_index,
            index_entries=index_entries,
            status_counts=dict(Counter(record.status.value for record in records)),
            fallback_count=sum(1 for record in records if record.fallback),
        )
        self._emit(
            "catalog_ingest.completed",
            records=len(records),
            failures=len(failures),
            status_counts=result.status_counts,
        )
        return result

    def _ingest_one(
        self,
        article_id: str,
        anchor_ids: list[str],
        source_asset: SourceAsset,
    ) -> CatalogIngestRecord:
        source_sha256 = source_asset.sha256 or self._checksum_verifier.digest(source_asset.path)
        existing_asset = self._catalog_repository.existing_asset(article_id, source_sha256)
        if existing_asset is not None:
            metadata = self._metadata_provider.metadata_for(article_id)
            return CatalogIngestRecord(
                article_id=article_id,
                anchor_ids=anchor_ids,
                source_asset_path=source_asset.path,
                catalog_asset_path=existing_asset.path,
                category=existing_asset.category,
                title=metadata.title,
                status=CatalogIngestStatus.SKIPPED,
                fallback=False,
                source_sha256=source_sha256,
                catalog_sha256=existing_asset.sha256,
                message="already present with matching SHA256",
            )

        metadata = self._metadata_provider.metadata_for(article_id)
        catalog_asset = self._catalog_repository.store_pdf_asset(
            source_asset,
            metadata,
            source_sha256,
        )
        self._catalog_repository.write_article_record(metadata, catalog_asset, anchor_ids)
        return CatalogIngestRecord(
            article_id=article_id,
            anchor_ids=anchor_ids,
            source_asset_path=source_asset.path,
            catalog_asset_path=catalog_asset.path,
            category=metadata.category,
            title=metadata.title,
            status=CatalogIngestStatus.INGESTED,
            fallback=metadata.fallback,
            source_sha256=source_sha256,
            catalog_sha256=catalog_asset.sha256,
            message=metadata.error or metadata.source,
        )

    def _emit(self, event: str, **payload: object) -> None:
        if self._event_sink is None:
            return
        self._event_sink.emit({"event": event, **payload})

    def _emit_failure(self, failure: CatalogIngestFailure) -> None:
        self._emit(
            "catalog_ingest.article_failed",
            article_id=failure.article_id,
            phase=failure.phase,
            reason=failure.reason,
            path=failure.path,
        )


__all__ = [
    "CatalogIngestEventSinkPort",
    "CatalogIngestRequest",
    "CatalogIngestResult",
    "CatalogIngestUseCase",
    "CatalogRepositoryPort",
    "ChecksumVerifierPort",
    "MetadataProviderPort",
    "SourceAssetStorePort",
]
