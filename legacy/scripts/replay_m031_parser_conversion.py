#!/usr/bin/env python3
"""Replay M031 parser conversion for local loader artifacts only.

The command consumes the M031 S02 loader evidence summary and converts only
parser-ready local source artifacts. Every non-convertible row is preserved as a
stable refusal diagnostic. It never fetches network sources, invokes arxiv2md or
``src/arxiv_archive/md_converter.py``, writes graph data, or claims LadybugDB/KG
readiness.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import sys
import tempfile
import time
from collections import Counter, defaultdict
from collections.abc import Iterable, Mapping
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any, cast

try:  # Tests monkeypatch this to prove absent PyMuPDF fails closed.
    import fitz  # type: ignore[import-untyped]  # ty:ignore[unresolved-import]
except Exception:  # pragma: no cover - environment dependent
    fitz = None  # type: ignore[assignment]

from bs4 import BeautifulSoup

MILESTONE_ID = "M031-vwpd8e"
SLICE_ID = "S03"
SOURCE_SLICE_ID = "S02"
SELECTION_ID = "m031-catalog-backed-replay-v1"
SCHEMA_VERSION = "m031-parser-conversion-replay.v1"
DIAGNOSTIC_SCHEMA_VERSION = "m031-parser-conversion-diagnostic.v1"

SUMMARY_NAME = "conversion-quality-summary.json"
DIAGNOSTICS_NAME = "conversion-quality-diagnostics.jsonl"
REPORT_NAME = "conversion-quality-report.md"
CONVERTED_TEXT_DIR_NAME = "converted-text"

PDF_ROLES = {"arxiv_pdf", "publisher_pdf", "external_pdf"}
HTML_ROLES = {"arxiv_html", "publisher_html", "web_article_html", "nature_html"}
METADATA_ONLY_ROLES = {"arxiv_abs_page", "arxiv_abs_url"}
MAX_PDF_PAGES = 8
MAX_TEXT_CHARS = 80_000
MIN_PARSER_READY_CHARS = 120

FAIL_CLOSED_SAFETY_FLAGS: dict[str, bool] = {
    "network_fetch_attempted": False,
    "arxiv2md_invoked": False,
    "md_converter_invoked": False,
    "external_cache_read": False,
    "external_cache_written": False,
    "raw_article_text_embedded": False,
    "raw_article_html_embedded": False,
    "raw_pdf_bytes_embedded": False,
    "binary_payload_embedded": False,
    "base64_payload_embedded": False,
    "parser_ready_claimed_without_conversion": False,
    "chunk_ready_claimed": False,
    "kg_readiness_claimed": False,
    "graph_import_allowed": False,
    "trusted_kg_import_allowed": False,
    "production_import_attempted": False,
    "ladybugdb_written": False,
    "graph_write_attempted": False,
    "production_persistence_attempted": False,
}

FORBIDDEN_SNIPPETS = ("<html", "</html", "%PDF-", "base64,")


class ParserConversionError(ValueError):
    """Typed CLI/setup error emitted as a deterministic diagnostic."""

    def __init__(self, code: str, message: str, *, json_path: str = "$") -> None:
        super().__init__(message)
        self.code = code
        self.json_path = json_path


def utc_now() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


def atomic_write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    tmp_path = Path(tmp_name)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        tmp_path.replace(path)
    except Exception:
        tmp_path.unlink(missing_ok=True)
        raise


def write_json(path: Path, payload: Mapping[str, Any]) -> None:
    atomic_write_text(path, json.dumps(payload, indent=2, sort_keys=False) + "\n")


def write_jsonl(path: Path, rows: Iterable[Mapping[str, Any]]) -> None:
    atomic_write_text(path, "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows))


def load_json_object(path: Path) -> dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            payload = json.load(handle)
    except json.JSONDecodeError as exc:
        raise ParserConversionError(
            "malformed_json", f"malformed JSON at {path}: {exc}", json_path=str(path)
        ) from exc
    except OSError as exc:
        raise ParserConversionError(
            "json_read_failed", f"failed to read {path}: {exc}", json_path=str(path)
        ) from exc
    if not isinstance(payload, dict):
        raise ParserConversionError(
            "malformed_json_object", f"expected JSON object at {path}", json_path=str(path)
        )
    return payload


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_child_path(
    root: Path, rel_path: Any, *, code: str = "unsafe_relative_path"
) -> tuple[Path | None, str | None, str | None]:
    if not isinstance(rel_path, str) or not rel_path.strip():
        return None, None, "missing_local_source_path"
    if "://" in rel_path:
        return None, None, "url_not_allowed_as_local_path"
    normalized = PurePosixPath(rel_path.replace("\\", "/"))
    if (
        normalized.is_absolute()
        or ".." in normalized.parts
        or any(part in ("", ".") for part in normalized.parts)
    ):
        return None, None, code
    root_resolved = root.resolve()
    resolved = (root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root_resolved):
        return None, None, code
    return resolved, normalized.as_posix(), None


def slug(value: str | None) -> str:
    safe = re.sub(r"[^A-Za-z0-9._-]+", "_", value or "missing").strip("_")
    return safe or "missing"


def converted_text_path(output_dir: Path, row: Mapping[str, Any]) -> Path:
    root = (output_dir / CONVERTED_TEXT_DIR_NAME).resolve()
    article_ref = str(row.get("article_ref") or row.get("identity") or "unknown")
    source_role = str(row.get("source_role") or "source")
    path = (root / slug(article_ref) / f"{slug(source_role)}.txt").resolve()
    if not path.is_relative_to(root):
        raise ParserConversionError(
            "converted_text_path_escape", "converted text path escapes output dir"
        )
    return path


def row_string(row: Mapping[str, Any], key: str) -> str | None:
    value = row.get(key)
    return value if isinstance(value, str) else None


def base_row(
    row: Mapping[str, Any], *, status: str, code: str, reason: str | None, safe_path: str | None
) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "identity": row_string(row, "identity"),
        "article_ref": row_string(row, "article_ref"),
        "article_key": row_string(row, "article_key"),
        "variant_id": row_string(row, "variant_id"),
        "source_role": row_string(row, "source_role"),
        "status": status,
        "terminal_state": status,
        "diagnostic_code": code,
        "code": code,
        "severity": "info" if status == "converted" else "warning",
        "refusal_code": None if status == "converted" else code,
        "failure_reason": reason,
        "json_path": "$",
        "safe_path": safe_path,
        "local_path": row_string(row, "local_path"),
        "source_media_type": row_string(row, "media_type"),
        "source_loader_status": row_string(row, "status"),
        "source_loader_diagnostic_code": row_string(row, "diagnostic_code"),
        "source_sha256": None,
        "source_byte_size": 0,
        "source_sha256_verified": False,
        "source_byte_size_verified": False,
        "converted_text_path": None,
        "converted_text_sha256": None,
        "converted_text_byte_size": 0,
        "parser_ready": False,
        "extraction_method": None,
        "bounded_extraction": {
            "max_pdf_pages": MAX_PDF_PAGES,
            "max_text_chars": MAX_TEXT_CHARS,
            "pages_processed": 0,
        },
        "quality": {
            "status": status,
            "char_count": 0,
            "line_count": 0,
            "warnings": [] if reason is None else [reason],
        },
        "network_fetch_attempted": False,
        "arxiv2md_invoked": False,
        "md_converter_invoked": False,
        "graph_import_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_write_attempted": False,
        "production_persistence_attempted": False,
        "chunk_ready_claimed": False,
        "kg_readiness_claimed": False,
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "raw_payload_embedded_in_metadata": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
    }


def diagnostic_from_row(row: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": DIAGNOSTIC_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "code": row.get("diagnostic_code"),
        "diagnostic_code": row.get("diagnostic_code"),
        "severity": row.get("severity") or "warning",
        "json_path": row.get("json_path") or "$",
        "identity": row.get("identity"),
        "article_ref": row.get("article_ref"),
        "source_role": row.get("source_role"),
        "safe_path": row.get("safe_path"),
        "status": row.get("status"),
        "refusal_code": row.get("refusal_code"),
        "message": row.get("failure_reason") or row.get("diagnostic_code"),
        "network_fetch_attempted": False,
        "graph_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
    }


def clean_text(value: str) -> str:
    return re.sub(r"\n{3,}", "\n\n", re.sub(r"[ \t]+", " ", value)).strip()


def text_quality(value: str, *, warning: str | None = None) -> dict[str, Any]:
    stripped = value.strip()
    lines = [line for line in stripped.splitlines() if line.strip()]
    warnings = [] if warning is None else [warning]
    if not stripped:
        status = "empty"
        warnings.append("converted text is empty")
    elif len(stripped) < MIN_PARSER_READY_CHARS:
        status = "low_quality"
        warnings.append("converted text is below parser-ready character threshold")
    else:
        status = "ok"
    return {
        "status": status,
        "char_count": len(stripped),
        "line_count": len(lines),
        "warnings": warnings,
    }


def extract_pdf_text(path: Path) -> tuple[str, dict[str, Any], dict[str, int], str | None]:
    if fitz is None:
        return (
            "",
            text_quality("", warning="PyMuPDF fitz is unavailable"),
            {"page_count": 0, "pages_processed": 0},
            "pymupdf_unavailable",
        )
    try:
        document = fitz.open(path)  # type: ignore[union-attr]
    except Exception as exc:
        return (
            "",
            text_quality("", warning=f"PyMuPDF failed to open PDF: {type(exc).__name__}"),
            {"page_count": 0, "pages_processed": 0},
            "pdf_open_failed",
        )
    try:
        page_count = len(document)
        pages_processed = min(page_count, MAX_PDF_PAGES)
        parts: list[str] = []
        for index in range(pages_processed):
            parts.append(cast(str, document[index].get_text("text")))
            if sum(len(part) for part in parts) >= MAX_TEXT_CHARS:
                break
        text = clean_text("\n".join(parts))[:MAX_TEXT_CHARS]
        quality = text_quality(text)
        if page_count > MAX_PDF_PAGES:
            quality["warnings"].append(f"PDF extraction bounded to first {MAX_PDF_PAGES} pages")
        return text, quality, {"page_count": page_count, "pages_processed": pages_processed}, None
    finally:
        document.close()


def extract_html_text(path: Path) -> tuple[str, dict[str, Any], dict[str, int]]:
    markup = path.read_text(encoding="utf-8", errors="replace")[:MAX_TEXT_CHARS]
    fallback_stub = "deterministic fallback capture" in markup.lower()
    soup = BeautifulSoup(markup, "html.parser")
    for tag in soup(["script", "style", "noscript", "svg", "nav", "header", "footer"]):
        tag.decompose()
    article = (
        soup.find("article")
        or soup.find(attrs={"role": "main"})  # pyrefly: ignore [bad-assignment, no-matching-overload]  # ty:ignore[invalid-argument-type]
        or soup.find("main")
        or soup.body
        or soup
    )
    paragraphs = article.find_all("p") if article else []
    headings = article.find_all(["h1", "h2", "h3"]) if article else []
    text_parts = [node.get_text(" ", strip=True) for node in [*headings, *paragraphs]]
    if not text_parts and article:
        text_parts = [article.get_text(" ", strip=True)]
    text = clean_text("\n".join(part for part in text_parts if part))[:MAX_TEXT_CHARS]
    quality = text_quality(
        text,
        warning="captured fallback HTML stub is not parser-ready content"
        if fallback_stub
        else None,
    )
    if fallback_stub:
        quality["status"] = "low_quality"
    structure = {
        "paragraph_count": len(paragraphs),
        "heading_count": len(headings),
        "article_tag_count": len(soup.find_all("article")),
        "fallback_stub_detected": int(fallback_stub),
    }
    return text, quality, structure


def verify_source(
    row: Mapping[str, Any], source_path: Path
) -> tuple[bool, str | None, int, str | None]:
    if not source_path.exists():
        return False, None, 0, "missing_source_artifact"
    if not source_path.is_file():
        return False, None, 0, "source_artifact_not_file"
    actual_size = source_path.stat().st_size
    actual_hash = sha256_file(source_path)
    expected_hash = row_string(row, "sha256")
    expected_size = row.get("byte_size") if isinstance(row.get("byte_size"), int) else None
    if expected_hash and actual_hash != expected_hash:
        return False, actual_hash, actual_size, "source_sha256_mismatch"
    if expected_size is not None and actual_size != expected_size:
        return False, actual_hash, actual_size, "source_byte_size_mismatch"
    return True, actual_hash, actual_size, None


def write_converted_text(path: Path, value: str) -> dict[str, Any]:
    atomic_write_text(path, value + ("\n" if value and not value.endswith("\n") else ""))
    return {
        "converted_text_path": path.as_posix(),
        "converted_text_sha256": sha256_file(path),
        "converted_text_byte_size": path.stat().st_size,
    }


def convert_row(
    row: Mapping[str, Any], *, source_dir: Path, output_dir: Path, index: int
) -> dict[str, Any]:
    source_role = row_string(row, "source_role") or "<missing-role>"
    loader_status = row_string(row, "status")
    local_path = row.get("local_path")
    source_path, safe_path, unsafe_code = safe_child_path(source_dir, local_path)

    if row.get("is_metadata_only") is True:
        result = base_row(
            row,
            status="metadata_only",
            code="metadata_only_refused",
            reason="metadata-only source is not parser-ready content",
            safe_path=safe_path,
        )
        result["json_path"] = f"$.results[{index}]"
        return result

    if loader_status == "blocked":
        code = (
            row_string(row, "blocker_code")
            or row_string(row, "diagnostic_code")
            or "loader_blocked"
        )
        result = base_row(
            row,
            status="blocked",
            code=code,
            reason=row_string(row, "failure_reason") or "loader row was blocked before conversion",
            safe_path=safe_path,
        )
        result["json_path"] = f"$.results[{index}]"
        return result

    if source_role in METADATA_ONLY_ROLES:
        result = base_row(
            row,
            status="metadata_only",
            code="metadata_only_refused",
            reason="metadata-only source is not parser-ready content",
            safe_path=safe_path,
        )
        result["json_path"] = f"$.results[{index}]"
        return result

    if unsafe_code is not None or source_path is None:
        code = unsafe_code or "missing_local_source_path"
        result = base_row(
            row,
            status="blocked",
            code=code,
            reason=f"unsafe or missing local source path: {code}",
            safe_path=safe_path,
        )
        result["json_path"] = f"$.results[{index}]"
        return result

    verified, actual_hash, actual_size, failure_code = verify_source(row, source_path)
    if not verified:
        result = base_row(
            row,
            status="blocked",
            code=failure_code or "source_verification_failed",
            reason="source artifact is absent or no longer matches loader evidence",
            safe_path=safe_path,
        )
        result.update({"source_sha256": actual_hash, "source_byte_size": actual_size})
        result["json_path"] = f"$.results[{index}]"
        return result

    media_type = (row_string(row, "media_type") or "").lower()
    suffix = source_path.suffix.lower()
    if source_role in PDF_ROLES or media_type == "application/pdf" or suffix == ".pdf":
        text, quality, bounds, extraction_code = extract_pdf_text(source_path)
        method = "pymupdf_bounded_text"
    elif (
        source_role in HTML_ROLES
        or media_type in {"text/html", "application/xhtml+xml"}
        or suffix in {".html", ".htm"}
    ):
        text, quality, structure = extract_html_text(source_path)
        bounds = {"page_count": 0, "pages_processed": 0, **structure}
        extraction_code = None
        method = "beautifulsoup_bounded_html_text"
    else:
        result = base_row(
            row,
            status="blocked",
            code="unsupported_media_type",
            reason=f"unsupported source role/media type: {source_role}/{media_type or suffix}",
            safe_path=safe_path,
        )
        result.update(
            {
                "source_sha256": actual_hash,
                "source_byte_size": actual_size,
                "source_sha256_verified": True,
                "source_byte_size_verified": True,
            }
        )
        result["json_path"] = f"$.results[{index}]"
        return result

    if extraction_code:
        result = base_row(
            row,
            status="blocked",
            code=extraction_code,
            reason="local extraction dependency failed closed",
            safe_path=safe_path,
        )
    elif quality["status"] == "ok" and text:
        result = base_row(
            row,
            status="converted",
            code="parser_ready_converted_text",
            reason=None,
            safe_path=safe_path,
        )
        result.update(write_converted_text(converted_text_path(output_dir, row), text))
        result["parser_ready"] = True
        result["severity"] = "info"
    else:
        code = (
            "empty_converted_text" if quality["status"] == "empty" else "converted_text_low_quality"
        )
        result = base_row(
            row,
            status="failed" if quality["status"] == "empty" else "low_quality",
            code=code,
            reason="; ".join(quality.get("warnings", [])) or "converted text is not parser-ready",
            safe_path=safe_path,
        )
    result.update(
        {
            "json_path": f"$.results[{index}]",
            "source_sha256": actual_hash,
            "source_byte_size": actual_size,
            "source_sha256_verified": True,
            "source_byte_size_verified": True,
            "extraction_method": method,
            "bounded_extraction": {
                "max_pdf_pages": MAX_PDF_PAGES,
                "max_text_chars": MAX_TEXT_CHARS,
                **bounds,
            },
            "quality": quality,
        }
    )
    return result


def validate_inputs(
    selection: Mapping[str, Any], loader_summary: Mapping[str, Any]
) -> list[Mapping[str, Any]]:
    if selection.get("selection_id") != loader_summary.get("selection_id"):
        raise ParserConversionError(
            "selection_loader_mismatch",
            "selection_id mismatch between selection and loader summary",
            json_path="$.selection_id",
        )
    if loader_summary.get("schema_version") != "m031-catalog-backed-loader-evidence.v1":
        raise ParserConversionError(
            "unexpected_loader_schema",
            "loader summary schema is not m031-catalog-backed-loader-evidence.v1",
            json_path="$.schema_version",
        )
    rows = loader_summary.get("results")
    if not isinstance(rows, list):
        raise ParserConversionError(
            "malformed_loader_results",
            "loader summary results must be a list",
            json_path="$.results",
        )
    for index, row in enumerate(rows):
        if not isinstance(row, Mapping):
            raise ParserConversionError(
                "malformed_loader_result",
                "loader result rows must be objects",
                json_path=f"$.results[{index}]",
            )
    return rows


def replay_conversion(
    *, selection_path: Path, loader_summary_path: Path, source_dir: Path, output_dir: Path
) -> list[dict[str, Any]]:
    selection = load_json_object(selection_path)
    loader_summary = load_json_object(loader_summary_path)
    rows = validate_inputs(selection, loader_summary)
    output_dir.mkdir(parents=True, exist_ok=True)
    converted_root = output_dir / CONVERTED_TEXT_DIR_NAME
    if converted_root.exists():
        shutil.rmtree(converted_root)
    return [
        convert_row(row, source_dir=source_dir, output_dir=output_dir, index=index)
        for index, row in enumerate(rows)
    ]


def build_summary(
    rows: list[dict[str, Any]],
    *,
    selection_path: Path,
    loader_summary_path: Path,
    source_dir: Path,
    output_dir: Path,
    duration_ms: int,
) -> dict[str, Any]:
    counts = Counter(str(row.get("status")) for row in rows)
    per_identity: dict[str, dict[str, int]] = defaultdict(
        lambda: {"converted": 0, "metadata_only": 0, "low_quality": 0, "blocked": 0, "failed": 0}
    )
    per_role: dict[str, dict[str, int]] = defaultdict(
        lambda: {"converted": 0, "metadata_only": 0, "low_quality": 0, "blocked": 0, "failed": 0}
    )
    for row in rows:
        identity = str(row.get("identity") or "<missing-identity>")
        role = str(row.get("source_role") or "<missing-role>")
        status = str(row.get("status"))
        if status in per_identity[identity]:
            per_identity[identity][status] += 1
            per_role[role][status] += 1
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "completed_with_diagnostics"
        if any(row.get("status") != "converted" for row in rows)
        else "completed",
        "row_count": len(rows),
        "parser_ready_count": sum(1 for row in rows if row.get("parser_ready") is True),
        "counts": dict(sorted(counts.items())),
        "per_identity_conversion_state_counts": {
            key: dict(value) for key, value in sorted(per_identity.items())
        },
        "per_role_conversion_state_counts": {
            key: dict(value) for key, value in sorted(per_role.items())
        },
        "results": rows,
        "input_paths": {
            "selection": selection_path.as_posix(),
            "loader_summary": loader_summary_path.as_posix(),
            "source_dir": source_dir.as_posix(),
        },
        "output_paths": {
            "output_dir": output_dir.as_posix(),
            "summary": (output_dir / SUMMARY_NAME).as_posix(),
            "diagnostics": (output_dir / DIAGNOSTICS_NAME).as_posix(),
            "report": (output_dir / REPORT_NAME).as_posix(),
            "converted_text_dir": (output_dir / CONVERTED_TEXT_DIR_NAME).as_posix(),
        },
        "duration_ms": duration_ms,
        "network_fetch_attempted": False,
        "arxiv2md_invoked": False,
        "md_converter_invoked": False,
        "graph_import_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_write_attempted": False,
        "production_persistence_attempted": False,
        "chunk_ready_claimed": False,
        "kg_readiness_claimed": False,
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "raw_payload_embedded_in_metadata": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
        "generated_at": utc_now(),
    }


def render_report(summary: Mapping[str, Any]) -> str:
    counts = summary.get("counts") if isinstance(summary.get("counts"), Mapping) else {}
    lines = [
        "# M031 Parser Conversion Replay Report",
        "",
        "This report is metadata-only. It does not embed source HTML, PDF bytes, converted text snippets, base64 payloads, network fetch results, graph facts, or LadybugDB readiness claims.",
        "",
        f"- Schema: `{summary.get('schema_version')}`",
        f"- Status: `{summary.get('status')}`",
        f"- Row count: {summary.get('row_count')}",
        f"- Parser-ready converted rows: {summary.get('parser_ready_count')}",
        f"- Counts: `{dict(counts)}`",  # pyrefly: ignore [bad-assignment, no-matching-overload]  # ty:ignore[no-matching-overload]
        "- Network fetch attempted: `False`",
        "- arxiv2md invoked: `False`",
        "- md_converter invoked: `False`",
        "- Graph/import/LadybugDB writes: `False`",
        "",
        "## Failure Modes",
        "",
        "Malformed JSON/setup exits with a typed CLI diagnostic. Row-level missing files, unsafe paths, unsupported media types, absent PyMuPDF, loader blockers, and extraction failures become non-parser-ready diagnostics rather than silent success.",
        "",
        "## Load Profile",
        "",
        f"The replay is bounded by S02 loader result rows, first {MAX_PDF_PAGES} PDF pages, and {MAX_TEXT_CHARS} extracted characters per source. At 10x the expected four-ref corpus, local disk reads/PDF parsing saturate first; there is no network, subprocess, graph import, or cache path.",
        "",
        "## Negative Tests",
        "",
        "Covered by `tests/test_m031_parser_conversion_replay.py`: unsafe `../` paths, missing local source, fallback/short HTML, metadata-only abs page, typed loader blockers, absent PyMuPDF, malformed JSON, metadata/report redaction, and fail-closed graph/import flags.",
        "",
        "## Results",
        "",
    ]
    for row in summary.get("results", []):
        if isinstance(row, Mapping):
            lines.append(
                f"- `{row.get('identity')}` `{row.get('source_role')}`: {row.get('status')} "
                f"({row.get('diagnostic_code')}) safe_path=`{row.get('safe_path') or '<none>'}` parser_ready={row.get('parser_ready')}"
            )
    return "\n".join(lines) + "\n"


def assert_redacted_text(text: str, *, path: Path) -> None:
    lowered = text.lower()
    found = [snippet for snippet in FORBIDDEN_SNIPPETS if snippet.lower() in lowered]
    if found:
        raise ParserConversionError(
            "raw_payload_artifact_snippet", f"metadata artifact is not redacted: {path}: {found}"
        )


def assert_fail_closed(summary: Mapping[str, Any]) -> None:
    flags = summary.get("fail_closed_safety_flags")
    if not isinstance(flags, Mapping):
        raise ParserConversionError(
            "missing_fail_closed_flags", "summary is missing fail_closed_safety_flags"
        )
    for flag, expected in FAIL_CLOSED_SAFETY_FLAGS.items():
        if flags.get(flag) is not expected:
            raise ParserConversionError(
                "unsafe_safety_flag", f"summary safety flag {flag}={flags.get(flag)!r}"
            )
    for row in summary.get("results", []):
        if not isinstance(row, Mapping):
            continue
        row_flags = row.get("fail_closed_safety_flags")
        if not isinstance(row_flags, Mapping):
            raise ParserConversionError(
                "missing_row_fail_closed_flags",
                "conversion row is missing fail_closed_safety_flags",
            )
        for flag, expected in FAIL_CLOSED_SAFETY_FLAGS.items():
            if row_flags.get(flag) is not expected:
                raise ParserConversionError(
                    "unsafe_safety_flag", f"row safety flag {flag}={row_flags.get(flag)!r}"
                )


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--loader-summary", required=True, type=Path)
    parser.add_argument("--source-dir", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    started = time.perf_counter()
    try:
        for cli_path in (args.selection, args.loader_summary, args.source_dir, args.output_dir):
            if (
                not cli_path.is_absolute()
                and ".." in PurePosixPath(str(cli_path).replace("\\", "/")).parts
            ):
                raise ParserConversionError("unsafe_cli_path", f"unsafe CLI path: {cli_path}")
        output_dir = args.output_dir.resolve()
        rows = replay_conversion(
            selection_path=args.selection,
            loader_summary_path=args.loader_summary,
            source_dir=args.source_dir,
            output_dir=output_dir,
        )
        summary = build_summary(
            rows,
            selection_path=args.selection,
            loader_summary_path=args.loader_summary,
            source_dir=args.source_dir,
            output_dir=output_dir,
            duration_ms=int((time.perf_counter() - started) * 1000),
        )
        diagnostics = [diagnostic_from_row(row) for row in rows]
        report = render_report(summary)
        assert_fail_closed(summary)
        write_json(output_dir / SUMMARY_NAME, summary)
        write_jsonl(output_dir / DIAGNOSTICS_NAME, diagnostics)
        atomic_write_text(output_dir / REPORT_NAME, report)
        for artifact_path in (
            output_dir / SUMMARY_NAME,
            output_dir / DIAGNOSTICS_NAME,
            output_dir / REPORT_NAME,
        ):
            assert_redacted_text(artifact_path.read_text(encoding="utf-8"), path=artifact_path)
        sys.stdout.write(
            json.dumps(
                {
                    "status": summary["status"],
                    "counts": summary["counts"],
                    "summary": (output_dir / SUMMARY_NAME).as_posix(),
                },
                sort_keys=True,
            )
            + "\n"
        )
        return 0
    except ParserConversionError as exc:
        sys.stderr.write(
            json.dumps(
                {
                    "status": "failed",
                    "code": exc.code,
                    "message": str(exc),
                    "json_path": exc.json_path,
                },
                sort_keys=True,
            )
            + "\n"
        )
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
