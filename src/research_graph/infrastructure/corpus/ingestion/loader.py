"""Article ingestion loader stack.

This module owns local source loading, source-type classification, full-text
quality fallback selection, checksums, and provenance records.  Public legacy
modules delegate here during the migration.


Formerly: src/arxiv_archive/ingestion/loader.py"""

from __future__ import annotations

import hashlib
import re
import time
from dataclasses import dataclass
from html.parser import HTMLParser
from pathlib import Path
from typing import Literal

from research_graph.infrastructure.corpus.ingestion.logging import (
    LOADER_NAME,
    ArticleEventLogger,
    ArticleJsonlLogger,
    emit_load_started,
    emit_load_terminal,
)
from research_graph.infrastructure.validation.logging import ValidationLogger

FullTextSourceType = Literal["markdown", "text"]
ExtractionMode = Literal[
    "structured_markdown",
    "plain_text",
    "missing_source",
    "empty_source",
    "low_quality_source",
]
FullTextQualityStatus = Literal["ok", "missing_source", "empty_source", "no_substantive_body"]
ArticleSourceType = Literal["auto", "markdown", "html", "pdf", "text", "ocr"]
ArticleOutcome = Literal["loaded", "loaded_metadata_only", "failed"]

SUPPORTED_SOURCE_TYPES = {"markdown", "text"}
MIN_SUBSTANTIVE_BODY_LINES = 1
_DEFAULT_MEDIA_TYPE = "application/octet-stream"


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


@dataclass(frozen=True)
class ArticleLoadSource:
    """Caller-supplied local source descriptor for one article artifact."""

    source_path: Path
    paper_id: str | None = None
    source_type: str = "auto"

    def __post_init__(self) -> None:
        object.__setattr__(self, "source_path", Path(self.source_path))


@dataclass(frozen=True)
class ArticleSourceMetadata:
    """Deterministic source classification and checksum metadata."""

    source_path: Path
    source_type: str
    media_type: str
    sha256: str | None
    byte_size: int
    source_id: str
    parser_name: str
    paper_id: str | None = None


@dataclass(frozen=True)
class ArticleLoadResult:
    """Article loader result containing provenance, outcome, and optional text."""

    source_path: Path
    source_type: str
    media_type: str
    sha256: str | None
    byte_size: int
    source_id: str
    parser_name: str
    loader_name: str
    outcome: str
    failure_reason: str | None
    warnings: list[str]
    duration_ms: int
    paper_id: str | None = None
    text: str | None = None
    quality: FullTextQualityReport | None = None
    provenance: dict[str, str | int | None] | None = None

    @property
    def warning_count(self) -> int:
        """Return the machine-readable warning count used in events."""
        return len(self.warnings)


@dataclass(frozen=True)
class _Classification:
    source_path: Path
    source_type: str
    media_type: str
    parser_name: str
    sha256: str | None
    byte_size: int
    source_id: str
    paper_id: str | None = None


def full_text_source_for_paper(
    paper_id: str,
    papers_dir: Path,
    *,
    source_type: FullTextSourceType = "markdown",
    filename: str = "full_text.md",
) -> FullTextSource:
    """Build the deterministic local full-text source for a stored paper artifact."""
    return FullTextSource(
        paper_id=paper_id,
        source_type=source_type,
        source_path=Path(papers_dir) / paper_id / filename,
    )


def ingest_full_text(source: FullTextSource) -> FullTextIngestionResult:
    """Read a local markdown/text source and return typed ingestion diagnostics."""
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
        return _full_text_result(
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
        return _full_text_result(
            source=source,
            text="",
            extraction_mode="empty_source",
            warnings=quality.warnings,
            fallback_reason=quality.fallback_reason,
            quality=quality,
        )

    quality = assess_full_text_quality(text)
    if quality.status == "no_substantive_body":
        return _full_text_result(
            source=source,
            text=text,
            extraction_mode="low_quality_source",
            warnings=quality.warnings,
            fallback_reason=quality.fallback_reason,
            quality=quality,
        )

    if source.source_type == "markdown" and _has_markdown_section_structure(text):
        return _full_text_result(
            source=source,
            text=text,
            extraction_mode="structured_markdown",
            warnings=[],
            fallback_reason=None,
            quality=quality,
        )

    return _full_text_result(
        source=source,
        text=text,
        extraction_mode="plain_text",
        warnings=["source has no markdown section structure"],
        fallback_reason="unstructured_text",
        quality=quality,
    )


def assess_full_text_quality(text: str) -> FullTextQualityReport:
    """Classify whether converted full text has substantive body content."""
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


def classify_article_source(
    source: str | Path | ArticleLoadSource,
    *,
    source_type: ArticleSourceType | str = "auto",
    paper_id: str | None = None,
) -> ArticleSourceMetadata:
    """Classify a local article source and return deterministic metadata."""
    descriptor = _coerce_source(source, source_type=source_type, paper_id=paper_id)
    raw_bytes = descriptor.source_path.read_bytes() if descriptor.source_path.exists() else None
    classified = _classify_from_bytes(descriptor, raw_bytes)
    return ArticleSourceMetadata(**classified.__dict__)


def load_article_source(
    source: str | Path | ArticleLoadSource,
    *,
    logger: ValidationLogger | ArticleEventLogger | None = None,
    log_path: str | Path | None = None,
    source_type: ArticleSourceType | str = "auto",
    paper_id: str | None = None,
) -> ArticleLoadResult:
    """Load one local article source and emit redacted metadata-only events."""
    start = time.perf_counter()
    owns_logger = logger is None and log_path is not None
    active_logger = logger or (ArticleJsonlLogger(Path(log_path)) if log_path is not None else None)

    try:
        descriptor = _coerce_source(source, source_type=source_type, paper_id=paper_id)
        raw_bytes = _read_source_bytes(descriptor.source_path)
        classified = _classify_from_bytes(descriptor, raw_bytes)
        emit_load_started(
            active_logger,
            source_path=classified.source_path,
            paper_id=classified.paper_id,
            source_id=classified.source_id,
            source_type=classified.source_type,
            media_type=classified.media_type,
            sha256=classified.sha256,
            byte_size=classified.byte_size,
            parser_name=classified.parser_name,
            duration_ms=_duration_ms(start),
        )

        if raw_bytes is None:
            return _terminal_result(
                active_logger,
                classified=classified,
                start=start,
                outcome="failed",
                failure_reason="source_missing",
                warnings=["source path does not exist"],
            )

        if classified.source_type == "unsupported":
            suffix = classified.source_path.suffix.lower() or "<none>"
            return _terminal_result(
                active_logger,
                classified=classified,
                start=start,
                outcome="failed",
                failure_reason="unsupported_type",
                warnings=[f"unsupported source extension: {suffix}"],
            )

        if classified.source_type == "pdf":
            return _terminal_result(
                active_logger,
                classified=classified,
                start=start,
                outcome="loaded_metadata_only",
                failure_reason=None,
                warnings=[],
            )

        text = _decode_text(raw_bytes)
        if text is None and classified.source_type == "html":
            # Optional latin-1 only when the payload still looks like real HTML text
            # (rejects binary/control-heavy bytes; preserves decode_failed contract).
            text = _decode_html_latin1_if_plausible(raw_bytes)
        if text is None:
            return _terminal_result(
                active_logger,
                classified=classified,
                start=start,
                outcome="failed",
                failure_reason="decode_failed",
                warnings=["source could not be decoded as utf-8 text"],
            )

        warnings: list[str] = []
        if classified.source_type == "html":
            normalized, html_warnings = normalize_local_html(text)
            warnings.extend(html_warnings)
            stripped = normalized.strip()
        else:
            stripped = text.strip()

        if not stripped:
            return _terminal_result(
                active_logger,
                classified=classified,
                start=start,
                outcome="failed",
                failure_reason="source_empty",
                warnings=warnings + ["source file is empty after trimming whitespace"],
            )

        quality = _assess_quality_if_applicable(classified.source_type, stripped)
        if quality is not None and quality.status == "no_substantive_body":
            return _terminal_result(
                active_logger,
                classified=classified,
                start=start,
                outcome="failed",
                failure_reason=quality.fallback_reason,
                warnings=list(quality.warnings) + warnings,
                quality=quality,
            )

        return _terminal_result(
            active_logger,
            classified=classified,
            start=start,
            outcome="loaded",
            failure_reason=None,
            warnings=warnings,
            text=stripped,
            quality=quality,
        )
    finally:
        if owns_logger and active_logger is not None:
            active_logger.close()


def _terminal_result(
    logger: ValidationLogger | ArticleEventLogger | None,
    *,
    classified: _Classification,
    start: float,
    outcome: str,
    failure_reason: str | None,
    warnings: list[str],
    text: str | None = None,
    quality: FullTextQualityReport | None = None,
) -> ArticleLoadResult:
    result = _build_article_result(
        classified=classified,
        start=start,
        outcome=outcome,
        failure_reason=failure_reason,
        warnings=warnings,
        text=text,
        quality=quality,
    )
    emit_load_terminal(
        logger,
        source_path=result.source_path,
        paper_id=result.paper_id,
        source_id=result.source_id,
        source_type=result.source_type,
        media_type=result.media_type,
        sha256=result.sha256,
        byte_size=result.byte_size,
        parser_name=result.parser_name,
        outcome=result.outcome,
        failure_reason=result.failure_reason,
        duration_ms=result.duration_ms,
        warning_count=result.warning_count,
    )
    return result


def _coerce_source(
    source: str | Path | ArticleLoadSource,
    *,
    source_type: ArticleSourceType | str,
    paper_id: str | None,
) -> ArticleLoadSource:
    if isinstance(source, ArticleLoadSource):
        explicit_type = source.source_type if source_type == "auto" else source_type
        explicit_paper_id = source.paper_id if paper_id is None else paper_id
        return ArticleLoadSource(
            source.source_path, paper_id=explicit_paper_id, source_type=explicit_type
        )
    return ArticleLoadSource(Path(source), paper_id=paper_id, source_type=source_type)


def _read_source_bytes(source_path: Path) -> bytes | None:
    if not source_path.exists():
        return None
    return source_path.read_bytes()


def _classify_from_bytes(source: ArticleLoadSource, raw_bytes: bytes | None) -> _Classification:
    source_path = source.source_path
    sha256 = hashlib.sha256(raw_bytes).hexdigest() if raw_bytes is not None else None
    byte_size = len(raw_bytes) if raw_bytes is not None else 0
    explicit_type = source.source_type.lower()
    source_type = _sniff_source_type(source_path, raw_bytes, explicit_type)
    media_type, parser_name = _type_metadata(source_type)
    source_id = _source_id(source_path=source_path, sha256=sha256, source_type=source_type)
    return _Classification(
        source_path=source_path,
        source_type=source_type,
        media_type=media_type,
        parser_name=parser_name,
        sha256=sha256,
        byte_size=byte_size,
        source_id=source_id,
        paper_id=source.paper_id,
    )


def _sniff_source_type(source_path: Path, raw_bytes: bytes | None, explicit_type: str) -> str:
    if raw_bytes is None:
        return "unknown"
    if explicit_type in {"markdown", "html", "pdf", "text"}:
        return explicit_type
    if explicit_type == "ocr":
        return "text"
    if raw_bytes.startswith(b"%PDF-"):
        return "pdf"

    suffix = source_path.suffix.lower()
    if suffix == ".md":
        return "markdown"
    if suffix in {".html", ".htm"}:
        return "html"
    if suffix == ".pdf":
        return "pdf"
    if suffix == ".txt":
        return "text"
    return "unsupported"


def _type_metadata(source_type: str) -> tuple[str, str]:
    if source_type == "markdown":
        return "text/markdown", "markdown_loader"
    if source_type == "html":
        return "text/html", "html_loader"
    if source_type == "pdf":
        return "application/pdf", "pdf_metadata_probe"
    if source_type == "text":
        return "text/plain", "text_loader"
    if source_type == "unsupported":
        return _DEFAULT_MEDIA_TYPE, "unsupported_loader"
    return _DEFAULT_MEDIA_TYPE, "unknown_loader"


def _source_id(*, source_path: Path, sha256: str | None, source_type: str) -> str:
    identity = sha256 if sha256 is not None else f"missing:{source_path}"
    digest = hashlib.sha256(f"{source_type}:{source_path}:{identity}".encode()).hexdigest()
    return f"article-source:{digest[:24]}"


def _decode_text(raw_bytes: bytes) -> str | None:
    try:
        return raw_bytes.decode("utf-8")
    except UnicodeDecodeError:
        return None


def _decode_html_latin1_if_plausible(raw_bytes: bytes) -> str | None:
    """Decode non-UTF-8 HTML only when content is mostly printable HTML text."""
    try:
        candidate = raw_bytes.decode("latin-1")
    except Exception:
        return None
    if not candidate:
        return None
    controls = sum(1 for ch in candidate if ord(ch) < 9 or 13 < ord(ch) < 32)
    if controls / max(len(candidate), 1) > 0.02:
        return None
    if "\x00" in candidate:
        return None
    lower = candidate.casefold()
    if not any(marker in lower for marker in ("<html", "<body", "<article", "<p", "<!doctype")):
        return None
    return candidate


class _HTMLTextExtractor(HTMLParser):
    """Stdlib HTML → text/markdown-ish extractor (local only, no network)."""

    _BLOCK = {
        "p",
        "div",
        "section",
        "article",
        "li",
        "ul",
        "ol",
        "br",
        "tr",
        "table",
        "header",
        "footer",
        "main",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
    }
    _SKIP = {"script", "style", "noscript", "template"}
    _HEADING = {"h1": "#", "h2": "##", "h3": "###", "h4": "####", "h5": "#####", "h6": "######"}

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self._parts: list[str] = []
        self._skip_depth = 0
        self._pending_heading: str | None = None
        self.broken_anchors = 0

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        tag_l = tag.lower()
        if tag_l in self._SKIP:
            self._skip_depth += 1
            return
        if self._skip_depth:
            return
        if tag_l in self._HEADING:
            self._pending_heading = self._HEADING[tag_l]
            self._parts.append("\n")
        elif tag_l in self._BLOCK:
            self._parts.append("\n")
        if tag_l == "a":
            href = dict(attrs).get("href")
            if href in (None, "", "#") or str(href).startswith("javascript:"):
                self.broken_anchors += 1

    def handle_endtag(self, tag: str) -> None:
        tag_l = tag.lower()
        if tag_l in self._SKIP and self._skip_depth:
            self._skip_depth -= 1
            return
        if self._skip_depth:
            return
        if tag_l in self._HEADING:
            self._pending_heading = None
            self._parts.append("\n")
        elif tag_l in self._BLOCK:
            self._parts.append("\n")

    def handle_data(self, data: str) -> None:
        if self._skip_depth:
            return
        text = data.strip()
        if not text:
            return
        if self._pending_heading:
            self._parts.append(f"{self._pending_heading} {text}")
            self._pending_heading = None
        else:
            self._parts.append(text + " ")

    def text(self) -> str:
        joined = "".join(self._parts)
        joined = re.sub(r"[ \t]+", " ", joined)
        joined = re.sub(r"\n{3,}", "\n\n", joined)
        return joined.strip()


def normalize_local_html(raw_html: str) -> tuple[str, list[str]]:
    """Normalize local HTML to plain/markdown-like text without network fetches."""
    warnings: list[str] = []
    if not raw_html.strip():
        return "", ["html_empty"]
    # Boilerplate-only detection: mostly navigation chrome without article body.
    lower = raw_html.casefold()
    if "<body" not in lower and "<article" not in lower and "<p" not in lower:
        warnings.append("html_missing_body_markers")
    parser = _HTMLTextExtractor()
    try:
        parser.feed(raw_html)
        parser.close()
    except Exception as exc:  # noqa: BLE001 - fail-closed HTML parse
        return "", [f"html_parse_error:{type(exc).__name__}"]
    text = parser.text()
    if parser.broken_anchors:
        warnings.append(f"broken_anchors:{parser.broken_anchors}")
    if not text:
        warnings.append("html_boilerplate_or_empty")
    return text, warnings


def _assess_quality_if_applicable(source_type: str, text: str) -> FullTextQualityReport | None:
    if source_type in {"markdown", "text", "html"}:
        return assess_full_text_quality(text)
    return None


def _build_article_result(
    *,
    classified: _Classification,
    start: float,
    outcome: str,
    failure_reason: str | None,
    warnings: list[str],
    text: str | None = None,
    quality: FullTextQualityReport | None = None,
) -> ArticleLoadResult:
    duration_ms = _duration_ms(start)
    # source_kind is the universal provenance tag (M207); mirrors source_type for
    # loaded text-like sources and stays explicit for downstream projection/retrieval.
    source_kind = classified.source_type
    provenance: dict[str, str | int | None] = {
        "source_id": classified.source_id,
        "source_path": str(classified.source_path),
        "source_type": classified.source_type,
        "source_kind": source_kind,
        "media_type": classified.media_type,
        "sha256": classified.sha256,
        "byte_size": classified.byte_size,
        "parser_name": classified.parser_name,
        "loader_name": LOADER_NAME,
        "network_fetch_attempted": False,
    }
    if classified.paper_id is not None:
        provenance["paper_id"] = classified.paper_id
    return ArticleLoadResult(
        source_path=classified.source_path,
        source_type=classified.source_type,
        media_type=classified.media_type,
        sha256=classified.sha256,
        byte_size=classified.byte_size,
        source_id=classified.source_id,
        parser_name=classified.parser_name,
        loader_name=LOADER_NAME,
        outcome=outcome,
        failure_reason=failure_reason,
        warnings=warnings,
        duration_ms=duration_ms,
        paper_id=classified.paper_id,
        text=text,
        quality=quality,
        provenance=provenance,
    )


def _has_markdown_section_structure(text: str) -> bool:
    return any(line.lstrip().startswith("#") for line in text.splitlines())


def _full_text_result(
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
        "source_kind": source.source_type,
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


def _duration_ms(start: float) -> int:
    return max(0, int((time.perf_counter() - start) * 1000))


__all__ = [
    "ArticleLoadResult",
    "ArticleLoadSource",
    "ArticleOutcome",
    "ArticleSourceMetadata",
    "ArticleSourceType",
    "ExtractionMode",
    "FullTextIngestionResult",
    "FullTextQualityReport",
    "FullTextQualityStatus",
    "FullTextSource",
    "FullTextSourceType",
    "assess_full_text_quality",
    "classify_article_source",
    "full_text_source_for_paper",
    "ingest_full_text",
    "load_article_source",
    "normalize_local_html",
]
