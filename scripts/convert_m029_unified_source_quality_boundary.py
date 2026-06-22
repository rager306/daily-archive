#!/usr/bin/env python3
"""Local conversion-quality boundary for M029 unified captured source artifacts.

This command consumes the frozen S02 source-acquisition summary and performs
bounded, local-only conversion checks.  It emits metadata-first artifacts only:
converted text is written to deterministic payload files and referenced by path,
size, and hash; summary/diagnostic/report metadata never embeds raw article text,
HTML, PDF bytes, or converted text payloads.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import tempfile
from collections import Counter
from collections.abc import Iterable, Mapping
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

try:  # Imported lazily by tests/command; dependency is available in project env.
    import fitz  # type: ignore[import-untyped]  # ty:ignore[unresolved-import]
except Exception:  # pragma: no cover - exercised only when environment lacks PyMuPDF.
    fitz = None  # type: ignore[assignment]

from bs4 import BeautifulSoup

MILESTONE_ID = "M029-eb0ljz"
SLICE_ID = "S03"
SELECTION_ID = "m029-unified-corpus-v1"
SCHEMA_VERSION = "m029-conversion-quality.v1"
ROOT = Path(__file__).resolve().parents[1]
CORPUS_DIR = ROOT / "data" / "article_corpora" / SELECTION_ID
SOURCE_DIR = CORPUS_DIR / "source"
SUMMARY_INPUT_PATH = CORPUS_DIR / "source-acquisition-summary.json"
SELECTION_INPUT_PATH = CORPUS_DIR / "selection.json"
CONVERTED_TEXT_DIR = CORPUS_DIR / "conversion-quality"
SUMMARY_OUTPUT_PATH = CORPUS_DIR / "conversion-quality-summary.json"
DIAGNOSTICS_OUTPUT_PATH = CORPUS_DIR / "conversion-quality-diagnostics.jsonl"
REPORT_OUTPUT_PATH = CORPUS_DIR / "conversion-quality-report.md"

CAPTURED_STATUS = "captured"
SUPPORTED_ROLES = {
    "arxiv_abs_page",
    "arxiv_html",
    "arxiv_pdf",
    "nature_html",
    "publisher_html",
    "web_article_html",
}
PDF_ROLES = {"arxiv_pdf", "publisher_pdf"}
HTML_ROLES = {"arxiv_abs_page", "arxiv_html", "nature_html", "publisher_html", "web_article_html"}
MAX_PDF_PAGES = 8
MAX_TEXT_CHARS = 80_000
MIN_PARSER_READY_CHARS = 120

FORBIDDEN_PAYLOAD_KEYS = {
    "text",
    "raw_text",
    "html",
    "pdf",
    "binary",
    "bytes",
    "base64",
    "payload",
    "content",
    "body",
}

FAIL_CLOSED_SAFETY_FLAGS: dict[str, bool] = {
    "metadata_manifests_embed_raw_text": False,
    "metadata_manifests_embed_raw_binary": False,
    "graph_import_allowed": False,
    "production_ladybugdb_write_allowed": False,
    "trusted_kg_import_allowed": False,
    "production_import_attempted": False,
    "ladybugdb_written": False,
    "raw_text_embedded_in_metadata": False,
    "raw_binary_embedded_in_metadata": False,
    "parser_readiness_claimed_without_conversion_quality": False,
}


def utc_now() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


def atomic_write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    tmp_path = Path(tmp_name)
    try:
        with open(fd, "w", encoding="utf-8", closefd=True) as handle:
            handle.write(text)
            handle.flush()
        tmp_path.replace(path)
    except Exception:
        tmp_path.unlink(missing_ok=True)
        raise


def write_json(path: Path, payload: Mapping[str, Any]) -> None:
    atomic_write_text(path, json.dumps(payload, indent=2, sort_keys=False) + "\n")


def write_jsonl(path: Path, rows: Iterable[Mapping[str, Any]]) -> None:
    atomic_write_text(path, "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows))


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"expected JSON object: {path}")
    return value


def rel(path: Path, root: Path = ROOT) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return str(path)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def git_commit(root: Path) -> str | None:
    git_dir = root / ".git"
    head_path = git_dir / "HEAD"
    try:
        head = head_path.read_text(encoding="utf-8").strip()
    except OSError:
        return None
    if head.startswith("ref:"):
        ref = head.split(" ", 1)[1].strip()
        try:
            return (git_dir / ref).read_text(encoding="utf-8").strip() or None
        except OSError:
            try:
                for line in (git_dir / "packed-refs").read_text(encoding="utf-8").splitlines():
                    if line and not line.startswith("#") and line.endswith(f" {ref}"):
                        return line.split(" ", 1)[0]
            except OSError:
                return None
    return head or None


def validate_safe_relative_path(value: Any, *, label: str) -> PurePosixPath:
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"missing_{label}")
    if "://" in value:
        raise ValueError(f"url_not_allowed_as_{label}")
    normalized = PurePosixPath(value.replace("\\", "/"))
    if (
        normalized.is_absolute()
        or ".." in normalized.parts
        or any(part == "" for part in normalized.parts)
    ):
        raise ValueError(f"unsafe_{label}")
    return normalized


def safe_article_ref(value: Any) -> PurePosixPath:
    return validate_safe_relative_path(value, label="article_ref")


def safe_local_path(value: Any) -> PurePosixPath:
    return validate_safe_relative_path(value, label="local_path")


def source_artifact_path(
    source_dir: Path, article_ref: PurePosixPath, local_path: PurePosixPath
) -> Path:
    root = source_dir.resolve()
    local_parts = local_path.parts
    article_parts = article_ref.parts
    if local_parts[: len(article_parts)] == article_parts:
        candidate = root / local_path.as_posix()
    else:
        candidate = root / article_ref.as_posix() / local_path.as_posix()
    resolved = candidate.resolve()
    if not resolved.is_relative_to(root):
        raise ValueError("source_path_escapes_source_dir")
    return resolved


def slug(value: str) -> str:
    safe = re.sub(r"[^A-Za-z0-9._-]+", "_", value).strip("_")
    return safe or "article"


def converted_text_path(converted_text_dir: Path, article_ref: str, source_role: str) -> Path:
    root = converted_text_dir.resolve()
    path = (root / slug(article_ref) / f"{slug(source_role)}.txt").resolve()
    if not path.is_relative_to(root):
        raise ValueError("converted_text_path_escapes_root")
    return path


def file_hashes(paths: Iterable[Path]) -> dict[str, str | None]:
    hashes: dict[str, str | None] = {}
    for path in paths:
        try:
            hashes[rel(path)] = sha256_file(path)
        except OSError:
            hashes[rel(path)] = None
    return hashes


def diagnostic_base(
    row: Mapping[str, Any], *, status: str, diagnostic_code: str, failure_reason: str | None
) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "article_ref": row.get("article_ref"),
        "article_key": row.get("article_key"),
        "identity_key": row.get("identity_key"),
        "variant_id": row.get("variant_id"),
        "source_role": row.get("source_role"),
        "source_path": row.get("local_path"),
        "status": status,
        "diagnostic_code": diagnostic_code,
        "code": diagnostic_code,
        "failure_reason": failure_reason,
        "network_fetch_attempted": False,
        "conversion_attempted": status not in {"blocked"},
        "parser_ready": False,
        "safety_flag_context": dict(FAIL_CLOSED_SAFETY_FLAGS),
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "raw_payload_embedded_in_metadata": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "graph_import_allowed": False,
    }


def blocked_result(
    row: Mapping[str, Any], code: str, reason: str, *, source_path: Path | None = None
) -> dict[str, Any]:
    result = diagnostic_base(row, status="blocked", diagnostic_code=code, failure_reason=reason)
    result.update(
        {
            "source_path_resolved": rel(source_path) if source_path else None,
            "source_sha256_verified": False,
            "source_byte_size_verified": False,
            "converted_text_path": None,
            "converted_text_sha256": None,
            "converted_text_byte_size": 0,
            "semantic_body_detected": False,
            "parser_ready_gate": "blocked",
            "fallback_reason": reason,
            "quality": {
                "status": "blocked",
                "char_count": 0,
                "line_count": 0,
                "warnings": [reason],
            },
        }
    )
    return result


def verify_source_bytes(
    row: Mapping[str, Any], source_path: Path
) -> tuple[bool, str | None, int, str | None]:
    if not source_path.exists():
        return False, None, 0, "missing_source_artifact"
    if not source_path.is_file():
        return False, None, 0, "source_artifact_not_file"
    try:
        actual_size = source_path.stat().st_size
        actual_hash = sha256_file(source_path)
    except OSError:
        return False, None, 0, "source_artifact_unreadable"
    if row.get("byte_size") != actual_size:
        return False, actual_hash, actual_size, "source_byte_size_mismatch"
    if row.get("sha256") != actual_hash:
        return False, actual_hash, actual_size, "source_sha256_mismatch"
    return True, actual_hash, actual_size, None


def clean_text(value: str) -> str:
    return re.sub(r"\n{3,}", "\n\n", re.sub(r"[ \t]+", " ", value)).strip()


def text_quality(text: str, *, warning: str | None = None) -> dict[str, Any]:
    stripped = text.strip()
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


def classify_abs_html(source_path: Path) -> tuple[str | None, dict[str, Any], dict[str, Any]]:
    html = source_path.read_text(encoding="utf-8", errors="replace")
    soup = BeautifulSoup(html, "html.parser")
    title = soup.find("h1") or soup.find("title")
    abstract = soup.find(class_=re.compile("abstract", re.I))
    structure = {
        "title_count": 1 if title else 0,
        "abstract_like_count": 1 if abstract else 0,
        "paragraph_count": len(soup.find_all("p")),
        "section_count": len(soup.find_all(["section", "h2", "h3"])),
    }
    status = (
        "metadata_only"
        if structure["abstract_like_count"] or structure["title_count"]
        else "low_quality"
    )
    quality = {
        "status": status,
        "char_count": 0,
        "line_count": 0,
        "warnings": ["arxiv abs HTML is metadata-only and is not parser-ready full text"],
    }
    return None, quality, structure


def extract_article_html(
    source_path: Path, *, source_role: str
) -> tuple[str, dict[str, Any], dict[str, Any]]:
    html = source_path.read_text(encoding="utf-8", errors="replace")
    soup = BeautifulSoup(html, "html.parser")
    for tag in soup(["script", "style", "noscript", "svg", "nav", "header", "footer", "aside"]):
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
    text = clean_text("\n".join(part for part in text_parts if part))[:MAX_TEXT_CHARS]
    structure = {
        "article_tag_count": len(soup.find_all("article")),
        "main_tag_count": len(soup.find_all("main")),
        "paragraph_count": len(paragraphs),
        "heading_count": len(headings),
        # pyrefly: ignore [not-callable]
        "semantic_body_detected": bool(paragraphs and len(text.strip()) >= MIN_PARSER_READY_CHARS),
        "source_role": source_role,
    }
    quality = text_quality(text)
    if not structure["semantic_body_detected"]:
        quality["status"] = "low_quality" if text.strip() else "empty"
        quality["warnings"].append(
            "HTML lacks a semantic article body after navigation/header/footer removal"
        )
    return text, quality, structure


def extract_pdf_text(source_path: Path) -> tuple[str, dict[str, Any], dict[str, Any]]:
    if fitz is None:
        return (
            "",
            text_quality("", warning="PyMuPDF fitz is unavailable"),
            {"page_count": 0, "pages_processed": 0},
        )
    try:
        document = fitz.open(source_path)  # type: ignore[union-attr]
    except Exception as exc:
        return (
            "",
            text_quality("", warning=f"PyMuPDF failed to open PDF: {type(exc).__name__}"),
            {
                "page_count": 0,
                "pages_processed": 0,
            },
        )
    try:
        page_count = len(document)
        pages_processed = min(page_count, MAX_PDF_PAGES)
        parts: list[str] = []
        for page_index in range(pages_processed):
            page_text = document[page_index].get_text("text")
            parts.append(page_text)
            if sum(len(part) for part in parts) >= MAX_TEXT_CHARS:
                break
        text = clean_text("\n".join(parts))[:MAX_TEXT_CHARS]
        quality = text_quality(text)
        if page_count > MAX_PDF_PAGES:
            quality["warnings"].append(f"PDF extraction bounded to first {MAX_PDF_PAGES} pages")
        return text, quality, {"page_count": page_count, "pages_processed": pages_processed}
    finally:
        document.close()


def write_converted_text(path: Path, text: str) -> dict[str, Any]:
    atomic_write_text(path, text + ("\n" if text and not text.endswith("\n") else ""))
    return {
        "converted_text_path": rel(path),
        "converted_text_sha256": sha256_file(path),
        "converted_text_byte_size": path.stat().st_size,
    }


def convert_captured_row(
    row: Mapping[str, Any], *, output_dir: Path, converted_text_dir: Path
) -> dict[str, Any]:
    if row.get("status") != CAPTURED_STATUS:
        return blocked_result(row, "source_not_captured", "S02 row was not captured")
    source_role = row.get("source_role")
    if source_role not in SUPPORTED_ROLES:
        return blocked_result(
            row, "unsupported_source_role", f"unsupported source role: {source_role}"
        )
    try:
        article_ref_path = safe_article_ref(row.get("article_ref"))
        local_path = safe_local_path(row.get("local_path"))
        source_path = source_artifact_path(output_dir, article_ref_path, local_path)
    except ValueError as exc:
        return blocked_result(row, str(exc), f"unsafe source locator: {exc}")

    verified, actual_hash, actual_size, failure_code = verify_source_bytes(row, source_path)
    if not verified:
        return blocked_result(
            row,
            failure_code or "source_verification_failed",
            "captured source bytes do not match S02 metadata",
            source_path=source_path,
        )

    result = diagnostic_base(
        row, status="converted", diagnostic_code="converted_source_artifact", failure_reason=None
    )
    result.update(
        {
            "source_path_resolved": rel(source_path),
            "source_sha256": actual_hash,
            "source_byte_size": actual_size,
            "source_sha256_verified": True,
            "source_byte_size_verified": True,
            "source_media_type": row.get("media_type"),
            "converted_text_path": None,
            "converted_text_sha256": None,
            "converted_text_byte_size": 0,
            "extraction_method": None,
            "structure_counts": {},
        }
    )

    text: str | None
    quality: dict[str, Any]
    structure: dict[str, Any]
    if source_role == "arxiv_abs_page":
        text, quality, structure = classify_abs_html(source_path)
        result["status"] = quality["status"]
        result["diagnostic_code"] = (
            "arxiv_abs_html_metadata_only"
            if quality["status"] == "metadata_only"
            else "arxiv_abs_html_low_quality"
        )
        result["code"] = result["diagnostic_code"]
        result["failure_reason"] = "arxiv abstract/navigation page is not parser-ready full text"
        result["conversion_attempted"] = True
        result["terminal_state"] = result["status"]
        result["fallback_reason"] = "no_substantive_body"
        result["extraction_method"] = "beautifulsoup_metadata_probe"
    elif source_role in HTML_ROLES:
        text, quality, structure = extract_article_html(source_path, source_role=str(source_role))
        result["extraction_method"] = "beautifulsoup_semantic_body"
    elif source_role in PDF_ROLES:
        text, quality, structure = extract_pdf_text(source_path)
        result["extraction_method"] = "pymupdf_bounded_text"
    else:
        text, quality, structure = extract_article_html(source_path, source_role=str(source_role))
        result["extraction_method"] = "beautifulsoup_semantic_body"

    if source_role != "arxiv_abs_page":
        if quality["status"] == "ok" and text:
            text_path = converted_text_path(
                converted_text_dir, str(row.get("article_ref")), str(source_role)
            )
            result.update(write_converted_text(text_path, text))
            result["parser_ready"] = True
            result["semantic_body_detected"] = bool(structure.get("semantic_body_detected", True))
            result["parser_ready_gate"] = "passed"
            result["fallback_reason"] = None
            result["diagnostic_code"] = "parser_ready_converted_text"
            result["code"] = "parser_ready_converted_text"
        else:
            result["status"] = "low_quality" if quality["status"] != "empty" else "failed"
            result["diagnostic_code"] = (
                "converted_text_low_quality"
                if quality["status"] != "empty"
                else "empty_converted_text"
            )
            result["code"] = result["diagnostic_code"]
            result["failure_reason"] = (
                "; ".join(quality.get("warnings", [])) or "converted text is not parser-ready"
            )
            result["parser_ready"] = False
            result["semantic_body_detected"] = bool(structure.get("semantic_body_detected", False))
            result["parser_ready_gate"] = "failed"
            result["fallback_reason"] = result["failure_reason"]

    if source_role == "arxiv_abs_page":
        result["semantic_body_detected"] = False
        result["parser_ready_gate"] = "failed"
        result["fallback_reason"] = "no_substantive_body"
    result["quality"] = quality
    result["structure_counts"] = structure
    return result


def validate_no_payload_keys(
    value: Any, *, path: str = "$", errors: list[str] | None = None
) -> list[str]:
    found = [] if errors is None else errors
    if isinstance(value, dict):
        for key, item in value.items():
            if key in FORBIDDEN_PAYLOAD_KEYS:
                found.append(f"{path}.{key}")
            validate_no_payload_keys(item, path=f"{path}.{key}", errors=found)
    elif isinstance(value, list):
        for index, item in enumerate(value):
            validate_no_payload_keys(item, path=f"{path}[{index}]", errors=found)
    return found


def build_provenance(
    args: argparse.Namespace, *, exit_code: int, duration_ms: int
) -> dict[str, Any]:
    outputs = [args.output_summary, args.output_diagnostics, args.output_report]
    hashable_outputs = [args.output_diagnostics, args.output_report]
    return {
        "schema_version": SCHEMA_VERSION,
        "command": [
            "uv",
            "run",
            "python",
            "scripts/convert_m029_unified_source_quality_boundary.py",
        ],
        "argv": ["scripts/convert_m029_unified_source_quality_boundary.py"],
        "cwd": str(ROOT),
        "git_commit": git_commit(ROOT),
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "input_paths": [rel(args.selection), rel(args.source_summary)],
        "input_hashes": file_hashes([args.selection, args.source_summary]),
        "output_paths": [rel(path) for path in outputs],
        "output_hashes": file_hashes(hashable_outputs),
        "output_hash_note": "conversion-quality-summary.json is intentionally excluded to avoid self-referential stale hashes",
        "exit_code": exit_code,
        "duration_ms": duration_ms,
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "graph_import_allowed": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
    }


def render_report(summary: Mapping[str, Any]) -> str:
    counts = summary.get("counts", {})
    return "\n".join(
        [
            "# M029 Unified Conversion Quality Report",
            "",
            "This report is metadata-only and does not embed raw article text, HTML, PDF bytes, or converted text payloads.",
            "",
            f"- Schema: `{summary.get('schema_version')}`",
            f"- Status: `{summary.get('status')}`",
            "- Network fetch attempted: `False`",
            "- Production import attempted: `False`",
            "- LadybugDB written: `False`",
            f"- Article count: {summary.get('article_count')}",
            f"- Variant count: {summary.get('variant_count')}",
            f"- Parser ready count: {summary.get('parser_ready_count')}",
            f"- Counts: `{dict(counts)}`",
            f"- Diagnostics: `{summary.get('diagnostic_count')}`",
            f"- Command: `{summary.get('provenance', {}).get('command')}`",
            f"- CWD: `{summary.get('provenance', {}).get('cwd')}`",
            f"- Git commit: `{summary.get('provenance', {}).get('git_commit')}`",
            "",
            "## Failure Modes",
            "",
            "Filesystem read/hash failures, malformed S02 JSON, missing captured artifacts, unsafe paths, hash/size mismatches, PyMuPDF open failures, and empty/low-quality extraction all produce explicit blocked/failed/low_quality diagnostics with fail-closed flags.",
            "",
            "## Load Profile",
            "",
            f"PDF extraction is the first expected saturation point at 10x load; extraction is bounded to {MAX_PDF_PAGES} pages and {MAX_TEXT_CHARS} characters per variant, with streamed hash checks for source bytes.",
            "",
            "## Negative Tests",
            "",
            "Covered by the M029 converter/verifier replay contract: malformed/unsafe paths, missing artifacts, hash mismatches, non-captured rows, metadata redaction, abs-page non-readiness, PDF/HTML fallback conversion, semantic-body rejection, and fail-closed parser-ready gates.",
            "",
        ]
    )


def run_conversion(args: argparse.Namespace) -> tuple[int, dict[str, Any], list[dict[str, Any]]]:
    selection = load_json(args.selection)
    if selection.get("selection_id") != SELECTION_ID:
        raise ValueError(f"selection_id mismatch: {selection.get('selection_id')!r}")
    selection = load_json(args.selection)
    source_summary = load_json(args.source_summary)
    rows = source_summary.get("results")
    if not isinstance(rows, list):
        raise ValueError("source acquisition summary missing results list")
    selected_articles = selection.get("articles")
    selected_article_count = len(selected_articles) if isinstance(selected_articles, list) else 0

    diagnostics: list[dict[str, Any]] = []
    for row in rows:
        if not isinstance(row, dict):
            diagnostics.append(
                blocked_result({}, "malformed_result_row", "S02 result row is not an object")
            )
            continue
        diagnostics.append(
            convert_captured_row(
                row, output_dir=args.source_dir, converted_text_dir=args.output_dir
            )
        )

    counts = Counter(str(row.get("status")) for row in diagnostics)
    parser_ready_count = sum(1 for row in diagnostics if row.get("parser_ready") is True)
    article_refs = sorted(
        {str(row.get("article_ref")) for row in diagnostics if row.get("article_ref")}
    )
    status = (
        "completed_with_diagnostics"
        if any(row.get("status") in {"blocked", "failed", "low_quality"} for row in diagnostics)
        else "completed"
    )
    exit_code = 0
    summary = {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": status,
        "exit_code_style_status": exit_code,
        "article_count": selected_article_count or len(article_refs),
        "variant_count": len(diagnostics),
        "parser_ready_count": parser_ready_count,
        "counts": dict(sorted(counts.items())),
        "source_summary_path": rel(args.source_summary),
        "source_summary_sha256": sha256_file(args.source_summary),
        "output_summary_path": rel(args.output_summary),
        "output_diagnostics_path": rel(args.output_diagnostics),
        "output_report_path": rel(args.output_report),
        "converted_text_dir": rel(args.output_dir),
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "graph_import_allowed": False,
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "raw_payload_embedded_in_metadata": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
        "results": diagnostics,
        "diagnostic_count": len(diagnostics),
    }
    leakage = validate_no_payload_keys(summary)
    if leakage:
        raise ValueError(f"metadata artifact contains forbidden payload keys: {leakage}")
    return exit_code, summary, diagnostics


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", type=Path, default=SELECTION_INPUT_PATH)
    parser.add_argument("--source-summary", type=Path, default=SUMMARY_INPUT_PATH)
    parser.add_argument("--source-dir", type=Path, default=SOURCE_DIR)
    parser.add_argument("--output-dir", type=Path, default=CONVERTED_TEXT_DIR)
    parser.add_argument("--output-summary", type=Path, default=SUMMARY_OUTPUT_PATH)
    parser.add_argument("--output-diagnostics", type=Path, default=DIAGNOSTICS_OUTPUT_PATH)
    parser.add_argument("--output-report", type=Path, default=REPORT_OUTPUT_PATH)
    args = parser.parse_args(argv[1:])
    corpus_dir = args.output_dir.parent
    args.output_summary = args.output_summary or corpus_dir / "conversion-quality-summary.json"
    args.output_diagnostics = (
        args.output_diagnostics or corpus_dir / "conversion-quality-diagnostics.jsonl"
    )
    args.output_report = args.output_report or corpus_dir / "conversion-quality-report.md"
    return args


def main(argv: list[str] | None = None) -> int:
    argv = argv or sys.argv
    args = parse_args(argv)
    try:
        exit_code, summary, diagnostics = run_conversion(args)
        duration_ms = 0
        provenance = build_provenance(args, exit_code=exit_code, duration_ms=duration_ms)
        summary["completed_at"] = "deterministic-local-replay"
        summary["provenance"] = provenance
        write_jsonl(args.output_diagnostics, diagnostics)
        atomic_write_text(args.output_report, render_report(summary))
        provenance["output_hashes"] = file_hashes([args.output_diagnostics, args.output_report])
        summary["provenance"] = provenance
        write_json(args.output_summary, summary)
        return exit_code
    except Exception as exc:
        sys.stderr.write(f"M029 unified conversion quality boundary failed: {exc}\n")
        return 1


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
