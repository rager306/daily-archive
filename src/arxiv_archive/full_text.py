"""Local full-text ingestion boundary for scientific paper artifacts.

This module is intentionally local-only: it reads deterministic markdown or
plain-text files and returns code-readable diagnostics for downstream PageIndex
construction. It does not fetch PDFs, call arXiv, or use the network.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Literal

FullTextSourceType = Literal["markdown", "text"]
ExtractionMode = Literal[
    "structured_markdown",
    "plain_text",
    "missing_source",
    "empty_source",
    "low_quality_source",
]
FullTextQualityStatus = Literal["ok", "missing_source", "empty_source", "no_substantive_body"]

SUPPORTED_SOURCE_TYPES = {"markdown", "text"}
MIN_SUBSTANTIVE_BODY_LINES = 1


@dataclass(frozen=True)
class FullTextQualityReport:
    """Machine-readable quality signal for local full-text artifacts."""

    status: str
    char_count: int
    line_count: int
    heading_count: int
    non_heading_nonempty_line_count: int
    warnings: list[str]
    fallback_reason: str | None


@dataclass(frozen=True)
class FullTextSource:
    """Local source descriptor for one paper's full-text artifact."""

    paper_id: str
    source_type: str
    source_path: Path

    def __post_init__(self) -> None:
        object.__setattr__(self, "source_path", Path(self.source_path))


@dataclass(frozen=True)
class FullTextIngestionResult:
    """PageIndex-ready full-text payload plus parser diagnostics."""

    paper_id: str
    source_type: str
    source_path: Path
    text: str
    extraction_mode: str
    warnings: list[str]
    fallback_reason: str | None
    quality: FullTextQualityReport
    provenance: dict[str, str]


def full_text_source_for_paper(
    paper_id: str,
    papers_dir: Path,
    *,
    source_type: FullTextSourceType = "markdown",
    filename: str = "full_text.md",
) -> FullTextSource:
    """Build the deterministic local full-text source for a stored paper artifact.

    Existing daily artifacts use a `papers/{paper_id}/` directory. S01 keeps the
    full-text boundary compatible with that layout by deriving the local source
    path without importing CLI code or changing the public cron surface.
    """
    return FullTextSource(
        paper_id=paper_id,
        source_type=source_type,
        source_path=Path(papers_dir) / paper_id / filename,
    )


def ingest_full_text(source: FullTextSource) -> FullTextIngestionResult:
    """Read a local markdown/text source and return typed ingestion diagnostics.

    Args:
        source: Local paper full-text source descriptor.

    Returns:
        A deterministic ingestion result containing text, provenance, warnings,
        extraction mode, and fallback reason.

    Raises:
        ValueError: If the source type is unsupported.
    """
    if source.source_type not in SUPPORTED_SOURCE_TYPES:
        raise ValueError(f"unsupported full-text source type: {source.source_type}")

    if not source.source_path.exists():
        quality = FullTextQualityReport(
            status="missing_source",
            char_count=0,
            line_count=0,
            heading_count=0,
            non_heading_nonempty_line_count=0,
            warnings=[f"source path does not exist: {source.source_path}"],
            fallback_reason="source_missing",
        )
        return _result(
            source=source,
            text="",
            extraction_mode="missing_source",
            warnings=quality.warnings,
            fallback_reason=quality.fallback_reason,
            quality=quality,
        )

    raw_text = source.source_path.read_text(encoding="utf-8")
    text = raw_text.strip()
    if not text:
        quality = FullTextQualityReport(
            status="empty_source",
            char_count=0,
            line_count=0,
            heading_count=0,
            non_heading_nonempty_line_count=0,
            warnings=["source file is empty after trimming whitespace"],
            fallback_reason="source_empty",
        )
        return _result(
            source=source,
            text="",
            extraction_mode="empty_source",
            warnings=quality.warnings,
            fallback_reason=quality.fallback_reason,
            quality=quality,
        )

    quality = assess_full_text_quality(text)
    if quality.status == "no_substantive_body":
        return _result(
            source=source,
            text=text,
            extraction_mode="low_quality_source",
            warnings=quality.warnings,
            fallback_reason=quality.fallback_reason,
            quality=quality,
        )

    if source.source_type == "markdown" and _has_markdown_section_structure(text):
        return _result(
            source=source,
            text=text,
            extraction_mode="structured_markdown",
            warnings=[],
            fallback_reason=None,
            quality=quality,
        )

    return _result(
        source=source,
        text=text,
        extraction_mode="plain_text",
        warnings=["source has no markdown section structure"],
        fallback_reason="unstructured_text",
        quality=quality,
    )


def assess_full_text_quality(text: str) -> FullTextQualityReport:
    """Classify whether converted full text has substantive body content.

    arxiv2md can return arXiv abstract-page navigation as markdown with headings
    but no paper body. That shape must not be treated as PageIndex-ready full
    text because it produces heading nodes with empty text and zero chunks.
    """
    stripped = text.strip()
    if not stripped:
        return FullTextQualityReport(
            status="empty_source",
            char_count=0,
            line_count=0,
            heading_count=0,
            non_heading_nonempty_line_count=0,
            warnings=["source file is empty after trimming whitespace"],
            fallback_reason="source_empty",
        )

    lines = stripped.splitlines()
    heading_count = sum(1 for line in lines if line.lstrip().startswith("#"))
    non_heading_nonempty_line_count = sum(
        1 for line in lines if line.strip() and not line.lstrip().startswith("#")
    )
    if non_heading_nonempty_line_count < MIN_SUBSTANTIVE_BODY_LINES:
        return FullTextQualityReport(
            status="no_substantive_body",
            char_count=len(stripped),
            line_count=len(lines),
            heading_count=heading_count,
            non_heading_nonempty_line_count=non_heading_nonempty_line_count,
            warnings=[
                "source has markdown headings but no substantive non-heading body text; likely arXiv landing/navigation page"
            ],
            fallback_reason="no_substantive_body",
        )

    return FullTextQualityReport(
        status="ok",
        char_count=len(stripped),
        line_count=len(lines),
        heading_count=heading_count,
        non_heading_nonempty_line_count=non_heading_nonempty_line_count,
        warnings=[],
        fallback_reason=None,
    )


def _has_markdown_section_structure(text: str) -> bool:
    """Return true when text includes at least one markdown heading."""
    return any(line.lstrip().startswith("#") for line in text.splitlines())


def _result(
    *,
    source: FullTextSource,
    text: str,
    extraction_mode: str,
    warnings: list[str],
    fallback_reason: str | None,
    quality: FullTextQualityReport,
) -> FullTextIngestionResult:
    provenance = {
        "paper_id": source.paper_id,
        "source_type": source.source_type,
        "source_path": str(source.source_path),
        "extraction_mode": extraction_mode,
    }
    if fallback_reason is not None:
        provenance["fallback_reason"] = fallback_reason

    return FullTextIngestionResult(
        paper_id=source.paper_id,
        source_type=source.source_type,
        source_path=source.source_path,
        text=text,
        extraction_mode=extraction_mode,
        warnings=warnings,
        fallback_reason=fallback_reason,
        quality=quality,
        provenance=provenance,
    )


__all__ = [
    "ExtractionMode",
    "FullTextIngestionResult",
    "FullTextQualityReport",
    "FullTextQualityStatus",
    "FullTextSource",
    "FullTextSourceType",
    "assess_full_text_quality",
    "full_text_source_for_paper",
    "ingest_full_text",
]
