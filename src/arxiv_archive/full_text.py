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
ExtractionMode = Literal["structured_markdown", "plain_text", "missing_source", "empty_source"]

SUPPORTED_SOURCE_TYPES = {"markdown", "text"}


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
    provenance: dict[str, str]


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
        return _result(
            source=source,
            text="",
            extraction_mode="missing_source",
            warnings=[f"source path does not exist: {source.source_path}"],
            fallback_reason="source_missing",
        )

    raw_text = source.source_path.read_text(encoding="utf-8")
    text = raw_text.strip()
    if not text:
        return _result(
            source=source,
            text="",
            extraction_mode="empty_source",
            warnings=["source file is empty after trimming whitespace"],
            fallback_reason="source_empty",
        )

    if source.source_type == "markdown" and _has_markdown_section_structure(text):
        return _result(
            source=source,
            text=text,
            extraction_mode="structured_markdown",
            warnings=[],
            fallback_reason=None,
        )

    return _result(
        source=source,
        text=text,
        extraction_mode="plain_text",
        warnings=["source has no markdown section structure"],
        fallback_reason="unstructured_text",
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
        provenance=provenance,
    )


__all__ = [
    "ExtractionMode",
    "FullTextIngestionResult",
    "FullTextSource",
    "FullTextSourceType",
    "ingest_full_text",
]
