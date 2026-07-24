"""Filesystem adapters for catalog ingest application ports.

These adapters keep filesystem, checksum, arXiv metadata, and canonical catalog
layout details outside the application use case introduced in M122 S02.
"""

from __future__ import annotations

import json
import shutil
import time
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, cast

from research_graph.application.corpus.catalog_ingest import (
    CatalogIngestResult,
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
from research_graph.infrastructure.corpus.ingestion.catalog_ingest import (
    CATALOG_ROOT_DEFAULT,
    M056_CUMULATIVE_CORPUS_PATH_DEFAULT,
    CANONICAL_CATALOG_INGEST_ROOT_DEFAULT,
    SAFETY_OVERRIDE_CANONICAL_CATALOG_INGEST,
    ApiMetrics,
    CumulativePdfRecord,
    RequestPacer,
    SafetyOverride,
    Sha256Mismatch,
    build_article_record,
    catalog_pdf_count,
    existing_catalog_pdf,
    fetch_arxiv_metadata,
    invert_anchor_membership,
    load_cumulative_offline_corpus,
    load_pdf_paths,
    load_selected_ids,
    normalize_category,
    sha256_file,
    update_index_if_exists,
    verify_offline_corpus_sha256,
    write_article_record,
)


@dataclass(frozen=True)
class CatalogAdapterDiagnostic:
    """Secret-free adapter diagnostic for logs or future event sinks."""

    operation: str
    path: str
    reason: str
    checksum_status: str | None = None


class CatalogAdapterError(RuntimeError):
    """Adapter failure carrying a structured diagnostic."""

    def __init__(self, diagnostic: CatalogAdapterDiagnostic) -> None:
        super().__init__(f"{diagnostic.operation} failed for {diagnostic.path}: {diagnostic.reason}")
        self.diagnostic = diagnostic


class CanonicalCatalogSourceAssetStore(SourceAssetStorePort):
    """Source asset adapter for M061-style anchor acquisition artifacts."""

    def __init__(self, catalog_ingest_root: Path | str = CANONICAL_CATALOG_INGEST_ROOT_DEFAULT) -> None:
        self.catalog_ingest_root = Path(catalog_ingest_root)

    def selected_article_membership(self) -> Mapping[str, Sequence[str]]:
        return invert_anchor_membership(load_selected_ids(self.catalog_ingest_root))

    def pdf_assets_by_article(self) -> Mapping[str, Sequence[SourceAsset]]:
        return {
            article_id: [self.source_asset_from_path(article_id, path) for path in paths]
            for article_id, paths in load_pdf_paths(self.catalog_ingest_root).items()
        }

    @staticmethod
    def source_asset_from_path(article_id: str, path: Path) -> SourceAsset:
        return SourceAsset(
            article_id=article_id,
            path=path.as_posix(),
            media_type="application/pdf",
            size_bytes=path.stat().st_size,
        )


class CumulativeCorpusSourceAssetStore(SourceAssetStorePort):
    """Source asset adapter for the M056 cumulative corpus artifact."""

    def __init__(
        self,
        cumulative_corpus_path: Path | str = M056_CUMULATIVE_CORPUS_PATH_DEFAULT,
        *,
        repo_root: Path | str = Path(),
    ) -> None:
        self.cumulative_corpus_path = Path(cumulative_corpus_path)
        self.repo_root = Path(repo_root)
        self.records = load_cumulative_offline_corpus(self.cumulative_corpus_path, repo_root=self.repo_root)

    def selected_article_membership(self) -> Mapping[str, Sequence[str]]:
        return {
            article_id: [record.source_milestone or "cumulative"]
            for article_id, record in self.records.items()
        }

    def pdf_assets_by_article(self) -> Mapping[str, Sequence[SourceAsset]]:
        return {
            article_id: [
                SourceAsset(
                    article_id=article_id,
                    path=record.pdf_path.as_posix(),
                    media_type="application/pdf",
                    size_bytes=record.size_bytes,
                    sha256=record.sha256,
                )
            ]
            for article_id, record in self.records.items()
        }

    def sha256_mismatches(self) -> list[Sha256Mismatch]:
        """Return SHA256 mismatches without exposing PDF payloads."""

        return verify_offline_corpus_sha256(self.records)


class OfflineCorpusMetadataProvider(MetadataProviderPort):
    """Synthetic metadata provider for SHA256-verified M056 local PDFs."""

    def __init__(self, records: Mapping[str, CumulativePdfRecord]) -> None:
        self.records = records

    def metadata_for(self, article_id: str) -> CatalogMetadata:
        record = self.records[article_id]
        return CatalogMetadata(
            article_id=article_id,
            category=record.category,
            title=f"arXiv {article_id} (M056 cumulative corpus, {record.source_milestone})",
            source="m056_cumulative_corpus_json",
            fallback=True,
            error=None,
        )


class Sha256ChecksumVerifier(ChecksumVerifierPort):
    """Checksum adapter backed by the existing chunked SHA256 helper."""

    def digest(self, path: str) -> str:
        return sha256_file(Path(path))


class ArxivCatalogMetadataProvider(MetadataProviderPort):
    """Metadata adapter backed by fail-closed arXiv metadata lookup."""

    def __init__(
        self,
        *,
        safety_override: SafetyOverride = SAFETY_OVERRIDE_CANONICAL_CATALOG_INGEST,
        metrics: ApiMetrics | None = None,
        sleep: Callable[[float], None] = time.sleep,
    ) -> None:
        self.safety_override = safety_override
        self.metrics = metrics if metrics is not None else ApiMetrics()
        self.sleep = sleep
        self.pacer = RequestPacer(sleep=sleep)

    @classmethod
    def offline(cls, *, metrics: ApiMetrics | None = None) -> ArxivCatalogMetadataProvider:
        return cls(
            safety_override=SafetyOverride(
                external_network_authorized=False,
                reason="offline catalog ingest adapter",
                scope="filesystem adapter verification",
            ),
            metrics=metrics,
            sleep=lambda _: None,
        )

    def metadata_for(self, article_id: str) -> CatalogMetadata:
        metadata = fetch_arxiv_metadata(
            article_id,
            pacer=self.pacer,
            metrics=self.metrics,
            sleep=self.sleep,
            safety_override=self.safety_override,
        )
        return CatalogMetadata(
            article_id=metadata.arxiv_id,
            category=metadata.category,
            title=metadata.title,
            source=metadata.source,
            fallback=metadata.fallback,
            error=metadata.error,
        )


class FilesystemCatalogRepository(CatalogRepositoryPort):
    """Canonical article catalog repository preserving the current file layout."""

    def __init__(
        self,
        catalog_root: Path | str = CATALOG_ROOT_DEFAULT,
        *,
        arxiv_root: Path | str | None = None,
    ) -> None:
        self.catalog_root = Path(catalog_root)
        self.arxiv_root = Path(arxiv_root) if arxiv_root is not None else self.catalog_root / "article_catalog" / "arxiv"

    def count_pdf_assets(self) -> int:
        return catalog_pdf_count(self.arxiv_root)

    def existing_asset(self, article_id: str, source_sha256: str) -> CatalogAsset | None:
        existing_pdf = existing_catalog_pdf(self.arxiv_root, article_id)
        if existing_pdf is None:
            return None
        existing_sha256 = sha256_file(existing_pdf)
        if existing_sha256 != source_sha256:
            return None
        return CatalogAsset(
            article_id=article_id,
            path=existing_pdf.as_posix(),
            sha256=existing_sha256,
            category=existing_pdf.parents[2].name,
        )

    def store_pdf_asset(
        self,
        source_asset: SourceAsset,
        metadata: CatalogMetadata,
        source_sha256: str,
    ) -> CatalogAsset:
        source_path = Path(source_asset.path)
        if not source_path.exists():
            raise CatalogAdapterError(
                CatalogAdapterDiagnostic(
                    operation="store_pdf_asset",
                    path=source_asset.path,
                    reason="source_missing",
                    checksum_status="not_checked",
                )
            )
        category = normalize_category(metadata.category)
        dest_pdf = self.arxiv_root / category / source_asset.article_id / "source" / f"{source_asset.article_id}.pdf"
        dest_pdf.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source_path, dest_pdf)
        dest_sha256 = sha256_file(dest_pdf)
        if dest_sha256 != source_sha256:
            raise CatalogAdapterError(
                CatalogAdapterDiagnostic(
                    operation="store_pdf_asset",
                    path=dest_pdf.as_posix(),
                    reason="checksum_mismatch_after_copy",
                    checksum_status="mismatch",
                )
            )
        return CatalogAsset(
            article_id=source_asset.article_id,
            path=dest_pdf.as_posix(),
            sha256=dest_sha256,
            category=category,
        )

    def write_article_record(
        self,
        metadata: CatalogMetadata,
        catalog_asset: CatalogAsset,
        anchor_ids: list[str],
    ) -> None:
        del anchor_ids  # current article.json schema does not persist anchor membership
        article = build_article_record(
            metadata.article_id,
            catalog_asset.category,
            metadata.title,
            Path(catalog_asset.path),
            catalog_root=self.catalog_root,
        )
        write_article_record(Path(catalog_asset.path).parents[1] / "article.json", article)

    def update_index(self) -> int | None:
        updated, entries, _diagnostics = update_index_if_exists(self.catalog_root)
        return entries if updated else None


def _article_claims_offline_corpus(article: dict[str, Any], expected_sha256: str) -> bool:
    identity = article.get("identity", {})
    if not isinstance(identity, dict):
        return False
    if identity.get("sha256") != expected_sha256:
        return False
    if identity.get("source_kind") != "m056_cumulative_corpus_local_pdf":
        return False
    variants = article.get("source_variants", [])
    if not isinstance(variants, list):
        return False
    return all(
        isinstance(variant, dict) and variant.get("network_fetch_attempted") is False
        for variant in variants
    )


def _patch_offline_corpus_article(
    *,
    article: dict[str, Any],
    record: CumulativePdfRecord,
    pdf_path: Path,
    catalog_root: Path,
    safety: SafetyOverride,
) -> dict[str, Any]:
    """Patch build_article_record output with M056 offline-corpus metadata."""

    arxiv_id = record.arxiv_id
    source_strategy = cast(dict[str, Any], article["source_strategy"])
    identity = cast(dict[str, Any], article["identity"])
    source_variants = cast(list[dict[str, Any]], article["source_variants"])
    expected_profile = cast(dict[str, Any], article["expected_profile"])
    safety_flags = cast(dict[str, Any], article["safety_flags"])

    source_strategy["primary_source_variant_id"] = f"{arxiv_id}:source:m056-cumulative-corpus"
    source_strategy["metadata_order"] = ["m056_cumulative_corpus_json"]
    source_strategy["pdf_policy"] = f"m056_{record.source_milestone}_sha256_verified"
    source_strategy["fallback_policy"] = (
        "use local PDF from M056 cumulative corpus only; no network, graph writes, "
        "or production import is authorized"
    )

    identity["source_kind"] = "m056_cumulative_corpus_local_pdf"
    identity["sha256"] = record.sha256
    identity["size_bytes"] = record.size_bytes
    identity["pages_estimate"] = record.pages_estimate
    identity["source_milestone"] = record.source_milestone

    source_variants[0].update(
        {
            "variant_id": f"{arxiv_id}:source:m056-cumulative-corpus",
            "source_role": "m056_cumulative_corpus_json",
            "source_origin": "local_artifact",
            "path": str(M056_CUMULATIVE_CORPUS_PATH_DEFAULT),
            "url": None,
            "capture_status": "captured_local",
            "capture_policy": "local_m056_cumulative_corpus_json_no_network",
            "loader_outcome": "loaded_metadata_from_cumulative_corpus_json",
            "network_fetch_attempted": False,
        }
    )
    source_variants[1].update(
        {
            "path": str(pdf_path.relative_to(catalog_root)),
            "source_origin": "m056_local_acquisition",
            "capture_policy": "local_copy_from_m056_cumulative_corpus_no_additional_pdf_download",
            "network_fetch_attempted": False,
        }
    )

    article["safety_override"] = {
        "external_network_authorized": False,
        "reason": safety.reason,
        "scope": safety.scope,
    }
    safety_flags["network_fetch_required_for_pipeline_phase"] = False
    expected_profile["synthetic_metadata"] = True
    return article


class OfflineFilesystemCatalogRepository(FilesystemCatalogRepository):
    """Catalog repository preserving the M056 offline article.json contract."""

    def __init__(
        self,
        catalog_root: Path | str = CATALOG_ROOT_DEFAULT,
        *,
        records: Mapping[str, CumulativePdfRecord],
        safety_override: SafetyOverride,
        arxiv_root: Path | str | None = None,
    ) -> None:
        super().__init__(catalog_root, arxiv_root=arxiv_root)
        self.records = records
        self.safety_override = safety_override

    def existing_asset(self, article_id: str, source_sha256: str) -> CatalogAsset | None:
        existing_pdf = existing_catalog_pdf(self.arxiv_root, article_id)
        if existing_pdf is None:
            return None
        existing_sha256 = sha256_file(existing_pdf)
        if existing_sha256 != source_sha256:
            return None
        article_path = existing_pdf.parents[1] / "article.json"
        if not article_path.exists():
            return None
        try:
            article = json.loads(article_path.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            return None
        if not _article_claims_offline_corpus(article, source_sha256):
            return None
        record = self.records.get(article_id)
        return CatalogAsset(
            article_id=article_id,
            path=existing_pdf.as_posix(),
            sha256=existing_sha256,
            category=record.category if record is not None else existing_pdf.parents[2].name,
        )

    def store_pdf_asset(
        self,
        source_asset: SourceAsset,
        metadata: CatalogMetadata,
        source_sha256: str,
    ) -> CatalogAsset:
        source_path = Path(source_asset.path)
        if not source_path.exists():
            raise CatalogAdapterError(
                CatalogAdapterDiagnostic(
                    operation="store_pdf_asset",
                    path=source_asset.path,
                    reason="source_missing",
                    checksum_status="not_checked",
                )
            )
        category = normalize_category(metadata.category)
        dest_pdf = self.arxiv_root / category / source_asset.article_id / "source" / f"{source_asset.article_id}.pdf"
        dest_pdf.parent.mkdir(parents=True, exist_ok=True)
        if source_path.resolve() != dest_pdf.resolve():
            shutil.copy2(source_path, dest_pdf)
        dest_sha256 = sha256_file(dest_pdf)
        if dest_sha256 != source_sha256:
            raise CatalogAdapterError(
                CatalogAdapterDiagnostic(
                    operation="store_pdf_asset",
                    path=dest_pdf.as_posix(),
                    reason="checksum_mismatch_after_copy",
                    checksum_status="mismatch",
                )
            )
        return CatalogAsset(
            article_id=source_asset.article_id,
            path=dest_pdf.as_posix(),
            sha256=dest_sha256,
            category=category,
        )

    def write_article_record(
        self,
        metadata: CatalogMetadata,
        catalog_asset: CatalogAsset,
        anchor_ids: list[str],
    ) -> None:
        del anchor_ids  # M056 article records are keyed by cumulative-corpus identity metadata
        record = self.records[metadata.article_id]
        article = build_article_record(
            metadata.article_id,
            catalog_asset.category,
            metadata.title,
            Path(catalog_asset.path),
            catalog_root=self.catalog_root,
        )
        article = _patch_offline_corpus_article(
            article=article,
            record=record,
            pdf_path=Path(catalog_asset.path),
            catalog_root=self.catalog_root,
            safety=self.safety_override,
        )
        write_article_record(Path(catalog_asset.path).parents[1] / "article.json", article)


def write_offline_corpus_ingest_events(events_log: Path, result: CatalogIngestResult) -> None:
    """Persist the legacy M056 ingest events log from application result diagnostics."""

    events_log.parent.mkdir(parents=True, exist_ok=True)
    with events_log.open("w", encoding="utf-8") as handle:
        for record in sorted(result.records, key=lambda item: item.article_id):
            if record.status == CatalogIngestStatus.SKIPPED:
                continue
            handle.write(
                json.dumps(
                    {
                        "event": "ingested",
                        "timestamp": datetime.now(UTC).isoformat(),
                        "arxiv_id": record.article_id,
                        "category": record.category,
                        "size_bytes": Path(record.catalog_asset_path).stat().st_size,
                        "sha256_verified": record.source_sha256 == record.catalog_sha256,
                    }
                )
                + "\n"
            )
        for failure in sorted(result.failures, key=lambda item: item.article_id):
            handle.write(
                json.dumps(
                    {
                        "event": "failed",
                        "timestamp": datetime.now(UTC).isoformat(),
                        "arxiv_id": failure.article_id,
                        "error": failure.message[:120],
                    }
                )
                + "\n"
            )


def write_offline_corpus_ingest_summary(summary_path: Path, result: CatalogIngestResult) -> None:
    """Persist the legacy M056 ingest summary JSON from application result diagnostics."""

    summary = {
        "schema_version": "r024-218-ingest-summary.v00.01",
        "generated_at": datetime.now(UTC).isoformat(),
        "corpus": "cumulative",
        "total_records": result.unique_article_ids,
        "ingested_count": sum(
            1 for record in result.records if record.status != CatalogIngestStatus.SKIPPED
        ),
        "skipped_count": result.status_counts.get(CatalogIngestStatus.SKIPPED.value, 0),
        "failed_count": len(result.failures),
        "index_updated": result.index_updated,
        "index_entries": result.index_entries,
        "fail_closed_invariants": {
            "network_fetch_attempted": False,
            "production_import_attempted": False,
            "graph_import_allowed": False,
            "ladybugdb_written": False,
            "trusted_kg_import_allowed": False,
            "graph_readiness_claim": False,
            "real_llm_extraction_used": False,
            "synthetic_only": True,
        },
    }
    summary_path.parent.mkdir(parents=True, exist_ok=True)
    summary_path.write_text(json.dumps(summary, indent=2), encoding="utf-8")


__all__ = [
    "ArxivCatalogMetadataProvider",
    "CatalogAdapterDiagnostic",
    "CatalogAdapterError",
    "FilesystemCatalogRepository",
    "CumulativeCorpusSourceAssetStore",
    "OfflineFilesystemCatalogRepository",
    "OfflineCorpusMetadataProvider",
    "CanonicalCatalogSourceAssetStore",
    "Sha256ChecksumVerifier",
    "write_offline_corpus_ingest_events",
    "write_offline_corpus_ingest_summary",
]
