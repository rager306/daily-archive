"""Domain contracts for corpus catalog workflows.

These types are infrastructure-agnostic. They describe article identities,
source assets, catalog assets, metadata, and ingest diagnostics without knowing
about filesystem layouts, arXiv clients, JSON writers, or graph stores.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum


class CatalogIngestStatus(StrEnum):
    """Stable ingest statuses surfaced by catalog ingest use cases."""

    INGESTED = "ingested"
    UPDATED = "updated"
    SKIPPED = "skipped"
    METADATA_CREATED = "metadata_created"
    FAILED = "failed"


class ParserReplayStatus(StrEnum):
    """Stable article-level statuses surfaced by parser replay use cases."""

    COMPLETED = "completed"
    SKIPPED = "skipped"
    LOW_QUALITY = "low_quality"
    FAILED = "failed"


class ParserReplayDiagnosticCode(StrEnum):
    """Reason-coded parser replay diagnostics safe for logs and summaries."""

    METADATA_ONLY_NO_LOCAL_SOURCE = "metadata_only_no_local_source"
    LOW_QUALITY_SOURCE = "low_quality_source"
    SOURCE_LOAD_ERROR = "source_load_error"
    PARSE_ERROR = "parse_error"
    CHUNK_WRITE_ERROR = "chunk_write_error"


@dataclass(frozen=True)
class SourceAsset:
    """A locally available source asset selected for catalog ingestion."""

    article_id: str
    path: str
    media_type: str
    size_bytes: int | None = None
    sha256: str | None = None


@dataclass(frozen=True)
class CatalogMetadata:
    """Metadata needed to write a canonical catalog record."""

    article_id: str
    category: str
    title: str
    source: str
    fallback: bool = False
    error: str | None = None


@dataclass(frozen=True)
class CatalogAsset:
    """A source asset after it has been placed in the canonical catalog."""

    article_id: str
    path: str
    sha256: str
    category: str


@dataclass(frozen=True)
class CatalogIngestRecord:
    """One article-level ingest outcome."""

    article_id: str
    anchor_ids: list[str]
    source_asset_path: str
    catalog_asset_path: str
    category: str
    title: str
    status: CatalogIngestStatus
    fallback: bool
    source_sha256: str
    catalog_sha256: str
    message: str


@dataclass(frozen=True)
class CatalogIngestFailure:
    """Secret-free failure diagnostic for one ingest phase."""

    article_id: str
    phase: str
    reason: str
    message: str
    path: str | None = None


@dataclass(frozen=True)
class ParserReplayArticle:
    """Catalog article selected for parser replay without embedding article body text."""

    article_id: str
    article_ref: str
    article_path: str
    source_kind_hint: str | None = None
    text_path_hint: str | None = None


@dataclass(frozen=True)
class ParserReplaySource:
    """Resolved local source for parser replay without storing source text."""

    article_id: str
    source_kind: str
    source_path: str
    text_chars: int
    cache_reused: bool = False
    pdf_pages: int = 0
    paper_id: str | None = None


@dataclass(frozen=True)
class ParserReplaySourceOutcome:
    """Source-loading outcome for completed, skipped, or low-quality inputs."""

    status: ParserReplayStatus
    source: ParserReplaySource | None = None
    reason: ParserReplayDiagnosticCode | None = None
    message: str = ""
    path: str | None = None


@dataclass(frozen=True)
class ParserReplayParsedArticle:
    """Parser output wrapper passed to chunk writers without infrastructure imports."""

    article_id: str
    payload: object
    section_count: int = 0
    parser_warnings: list[str] | None = None


@dataclass(frozen=True)
class ParserReplayChunkWriteResult:
    """Chunk writer outcome summarized without chunk text payloads."""

    chunk_count: int
    output_paths: list[str]
    warnings: list[str] | None = None


@dataclass(frozen=True)
class ParserReplayRecord:
    """One article-level parser replay outcome safe for summaries."""

    article_id: str
    article_ref: str
    status: ParserReplayStatus
    source_kind: str | None
    source_path: str | None
    chunk_count: int
    reason: ParserReplayDiagnosticCode | None = None
    message: str = ""
    text_chars: int = 0
    pdf_pages: int = 0
    cache_reused: bool = False


@dataclass(frozen=True)
class ParserReplayDiagnostic:
    """Secret-free parser replay diagnostic with reason code and optional path."""

    article_id: str
    phase: str
    reason: ParserReplayDiagnosticCode
    message: str
    path: str | None = None


__all__ = [
    "CatalogAsset",
    "CatalogIngestFailure",
    "CatalogIngestRecord",
    "CatalogIngestStatus",
    "CatalogMetadata",
    "ParserReplayArticle",
    "ParserReplayChunkWriteResult",
    "ParserReplayDiagnostic",
    "ParserReplayDiagnosticCode",
    "ParserReplayParsedArticle",
    "ParserReplayRecord",
    "ParserReplaySource",
    "ParserReplaySourceOutcome",
    "ParserReplayStatus",
    "SourceAsset",
]
