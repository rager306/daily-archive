#!/usr/bin/env python3
"""Replay-safe verifier for the M029 unified conversion-quality boundary.

The command is local-only and fail-closed. If S03 conversion-quality artifacts
are absent, it materializes deterministic metadata-first artifacts from the
frozen S02 source-acquisition handoff. If artifacts already exist, it verifies
rather than rewrites them so source/hash/path drift is caught deterministically.
It never fetches network sources, imports graph state, or writes production DBs.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import tempfile
from collections import Counter, defaultdict
from collections.abc import Iterable, Mapping
from pathlib import Path, PurePosixPath
from typing import Any

try:  # Dependency is available in the project env; tests keep this optional.
    import fitz  # type: ignore[import-untyped]
except Exception:  # pragma: no cover
    fitz = None  # type: ignore[assignment]

from bs4 import BeautifulSoup

MILESTONE_ID = "M029-eb0ljz"
SLICE_ID = "S03"
SOURCE_SLICE_ID = "S02"
SELECTION_ID = "m029-unified-corpus-v1"
SCHEMA_VERSION = "m029-conversion-quality.v1"
VERIFIER_SCHEMA_VERSION = "m029-conversion-quality-verifier.v1"
ROOT = Path(__file__).resolve().parents[1]
CORPUS_DIR = ROOT / "data" / "article_corpora" / SELECTION_ID
SOURCE_SUMMARY_PATH = CORPUS_DIR / "source-acquisition-summary.json"
SELECTION_PATH = CORPUS_DIR / "selection.json"
SUMMARY_PATH = CORPUS_DIR / "conversion-quality-summary.json"
DIAGNOSTICS_PATH = CORPUS_DIR / "conversion-quality-diagnostics.jsonl"
REPORT_PATH = CORPUS_DIR / "conversion-quality-report.md"
CONVERTED_TEXT_DIR = CORPUS_DIR / "conversion-quality"
SOURCE_ROOT = CORPUS_DIR / "source"

EXPECTED_ARTICLE_COUNT = 18
EXPECTED_VARIANT_COUNT = 29
TERMINAL_STATUSES = {"converted", "metadata_only", "blocked", "failed", "low_quality"}
CAPTURED_STATUS = "captured"
ARXIV_FULL_TEXT_ROLES = {"arxiv_pdf", "arxiv_html"}
ARXIV_METADATA_ROLES = {"arxiv_abs_page"}
HTML_ROLES = {"arxiv_abs_page", "arxiv_html", "nature_html", "web_article_html", "publisher_html"}
PDF_ROLES = {"arxiv_pdf", "publisher_pdf"}
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
FORBIDDEN_SNIPPETS = {
    "<html",
    "</html",
    "%PDF-",
    "base64,",
    "RAW_ARXIV_ABS_SECRET",
    "RAW_NATURE_BODY_SECRET",
    "RAW_PDF_SECRET",
}
UNSAFE_TRUE_FLAGS = {
    "metadata_manifests_embed_raw_text",
    "metadata_manifests_embed_raw_binary",
    "graph_import_allowed",
    "production_ladybugdb_write_allowed",
    "trusted_kg_import_allowed",
    "production_import_attempted",
    "ladybugdb_written",
    "raw_text_embedded",
    "raw_binary_embedded",
    "raw_payload_embedded_in_metadata",
    "raw_text_embedded_in_metadata",
    "raw_binary_embedded_in_metadata",
    "parser_readiness_claimed_without_conversion_quality",
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


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        value = json.loads(line)
        if not isinstance(value, dict):
            raise ValueError(f"expected JSON object at {path}:{line_number}")
        rows.append(value)
    return rows


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


def safe_relative_path(value: Any, *, code_label: str) -> PurePosixPath:
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"missing_{code_label}")
    if "://" in value:
        raise ValueError(f"url_not_allowed_as_{code_label}")
    normalized = PurePosixPath(value.replace("\\", "/"))
    if normalized.is_absolute() or ".." in normalized.parts or any(part == "" for part in normalized.parts):
        raise ValueError(f"unsafe_{code_label}")
    return normalized


def safe_under_root(root: Path, relative_path: Any, *, code_label: str) -> Path:
    normalized = safe_relative_path(relative_path, code_label=code_label)
    root_resolved = root.resolve()
    resolved = (root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root_resolved):
        raise ValueError(f"{code_label}_escapes_root")
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


def source_artifact_path(source_root: Path, local_path: Any) -> Path:
    return safe_under_root(source_root, local_path, code_label="local_path")


def diagnostic(
    code: str,
    message: str,
    *,
    severity: str = "error",
    path: Path | str | None = None,
    json_path: str = "$",
    article_ref: str | None = None,
    variant_id: str | None = None,
    source_role: str | None = None,
) -> dict[str, Any]:
    return {
        "schema_version": VERIFIER_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "severity": severity,
        "diagnostic_code": code,
        "code": code,
        "message": message,
        "failure_reason": message,
        "path": rel(path) if isinstance(path, Path) else path,
        "json_path": json_path,
        "article_ref": article_ref,
        "variant_id": variant_id,
        "source_role": source_role,
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_import_allowed": False,
    }


def conversion_base(row: Mapping[str, Any], *, status: str, code: str, failure_reason: str | None) -> dict[str, Any]:
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
        "status": status,
        "terminal_state": status,
        "diagnostic_code": code,
        "code": code,
        "failure_reason": failure_reason,
        "fallback_reason": None,
        "network_fetch_attempted": False,
        "conversion_attempted": status != "blocked",
        "parser_ready": False,
        "source_path": row.get("local_path"),
        "source_path_resolved": None,
        "source_sha256": None,
        "source_byte_size": 0,
        "source_sha256_verified": False,
        "source_byte_size_verified": False,
        "source_media_type": row.get("media_type"),
        "converted_text_path": None,
        "converted_text_sha256": None,
        "converted_text_byte_size": 0,
        "extraction_method": None,
        "structure_counts": {},
        "quality": {"status": status, "char_count": 0, "line_count": 0, "warnings": []},
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


def blocked_conversion(row: Mapping[str, Any], code: str, reason: str, *, source_path: Path | None = None) -> dict[str, Any]:
    result = conversion_base(row, status="blocked", code=code, failure_reason=reason)
    result["source_path_resolved"] = rel(source_path) if source_path else None
    result["quality"] = {"status": "blocked", "char_count": 0, "line_count": 0, "warnings": [reason]}
    return result


def verify_source_bytes(row: Mapping[str, Any], source_path: Path) -> tuple[bool, str | None, int, str | None]:
    if not source_path.exists():
        return False, None, 0, "missing_source_artifact"
    if not source_path.is_file():
        return False, None, 0, "source_artifact_not_file"
    actual_size = source_path.stat().st_size
    actual_hash = sha256_file(source_path)
    if row.get("byte_size") != actual_size:
        return False, actual_hash, actual_size, "source_byte_size_mismatch"
    if row.get("sha256") != actual_hash:
        return False, actual_hash, actual_size, "source_sha256_mismatch"
    return True, actual_hash, actual_size, None


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
    return {"status": status, "char_count": len(stripped), "line_count": len(lines), "warnings": warnings}


def classify_arxiv_abs_page(source_path: Path) -> tuple[None, dict[str, Any], dict[str, Any]]:
    markup = source_path.read_text(encoding="utf-8", errors="replace")
    soup = BeautifulSoup(markup, "html.parser")
    title = soup.find("h1") or soup.find("title")
    abstract = soup.find(class_=re.compile("abstract", re.I)) or soup.find("blockquote")
    nav_like = len(soup.find_all(["nav", "header", "footer", "a"]))
    structure = {
        "title_count": 1 if title else 0,
        "abstract_like_count": 1 if abstract else 0,
        "navigation_like_count": nav_like,
        "paragraph_count": len(soup.find_all("p")),
        "section_count": len(soup.find_all(["section", "h2", "h3"])),
    }
    quality = {
        "status": "metadata_only" if structure["title_count"] or structure["abstract_like_count"] or nav_like else "low_quality",
        "char_count": 0,
        "line_count": 0,
        "warnings": ["arxiv abstract/navigation page has no substantive body"],
    }
    return None, quality, structure


def extract_html_body(source_path: Path) -> tuple[str, dict[str, Any], dict[str, Any]]:
    markup = source_path.read_text(encoding="utf-8", errors="replace")
    soup = BeautifulSoup(markup, "html.parser")
    for tag in soup(["script", "style", "noscript", "svg", "nav", "header", "footer"]):
        tag.decompose()
    article = soup.find("article") or soup.find(attrs={"role": "main"}) or soup.find("main") or soup.body or soup
    paragraphs = article.find_all("p") if article else []
    headings = article.find_all(["h1", "h2", "h3"]) if article else []
    text_parts = [node.get_text(" ", strip=True) for node in [*headings, *paragraphs]]
    if not text_parts and article:
        text_parts = [article.get_text(" ", strip=True)]
    text = clean_text("\n".join(part for part in text_parts if part))[:MAX_TEXT_CHARS]
    structure = {
        "article_tag_count": len(soup.find_all("article")),
        "main_tag_count": len(soup.find_all("main")),
        "paragraph_count": len(paragraphs),
        "heading_count": len(headings),
    }
    return text, text_quality(text), structure


def extract_pdf_text(source_path: Path) -> tuple[str, dict[str, Any], dict[str, Any]]:
    if fitz is None:
        return "", text_quality("", warning="PyMuPDF fitz is unavailable"), {"page_count": 0, "pages_processed": 0}
    try:
        document = fitz.open(source_path)  # type: ignore[union-attr]
    except Exception as exc:
        return "", text_quality("", warning=f"PyMuPDF failed to open PDF: {type(exc).__name__}"), {"page_count": 0, "pages_processed": 0}
    try:
        page_count = len(document)
        pages_processed = min(page_count, MAX_PDF_PAGES)
        parts: list[str] = []
        for page_index in range(pages_processed):
            parts.append(document[page_index].get_text("text"))
            if sum(len(part) for part in parts) >= MAX_TEXT_CHARS:
                break
        text = clean_text("\n".join(parts))[:MAX_TEXT_CHARS]
        quality = text_quality(text)
        if page_count > MAX_PDF_PAGES:
            quality["warnings"].append(f"PDF extraction bounded to first {MAX_PDF_PAGES} pages")
        return text, quality, {"page_count": page_count, "pages_processed": pages_processed}
    finally:
        document.close()


def write_converted_text(path: Path, value: str) -> dict[str, Any]:
    atomic_write_text(path, value + ("\n" if value and not value.endswith("\n") else ""))
    return {
        "converted_text_path": rel(path),
        "converted_text_sha256": sha256_file(path),
        "converted_text_byte_size": path.stat().st_size,
    }


def convert_source_row(row: Mapping[str, Any], *, source_root: Path, converted_text_dir: Path) -> dict[str, Any]:
    if row.get("status") != CAPTURED_STATUS:
        return blocked_conversion(row, "source_not_captured", "S02 row was not captured")
    source_role = row.get("source_role")
    if source_role not in HTML_ROLES | PDF_ROLES:
        return blocked_conversion(row, "unsupported_source_role", f"unsupported source role: {source_role}")
    try:
        safe_relative_path(row.get("article_ref"), code_label="article_ref")
        source_path = source_artifact_path(source_root, row.get("local_path"))
    except ValueError as exc:
        return blocked_conversion(row, str(exc), f"unsafe source locator: {exc}")

    verified, actual_hash, actual_size, failure_code = verify_source_bytes(row, source_path)
    if not verified:
        return blocked_conversion(row, failure_code or "source_verification_failed", "captured source bytes do not match S02 metadata", source_path=source_path)

    result = conversion_base(row, status="converted", code="converted_source_artifact", failure_reason=None)
    result.update(
        {
            "source_path_resolved": rel(source_path),
            "source_sha256": actual_hash,
            "source_byte_size": actual_size,
            "source_sha256_verified": True,
            "source_byte_size_verified": True,
        }
    )

    if source_role in ARXIV_METADATA_ROLES:
        _text, quality, structure = classify_arxiv_abs_page(source_path)
        result["status"] = "metadata_only"
        result["terminal_state"] = "metadata_only"
        result["diagnostic_code"] = "arxiv_abs_html_metadata_only"
        result["code"] = "arxiv_abs_html_metadata_only"
        result["failure_reason"] = "arxiv abstract/navigation page is not parser-ready full text"
        result["fallback_reason"] = "no_substantive_body"
        result["extraction_method"] = "beautifulsoup_metadata_probe"
    elif source_role in PDF_ROLES:
        text, quality, structure = extract_pdf_text(source_path)
        result["extraction_method"] = "pymupdf_bounded_text"
    else:
        text, quality, structure = extract_html_body(source_path)
        result["extraction_method"] = "beautifulsoup_html_body"

    if source_role not in ARXIV_METADATA_ROLES:
        if quality["status"] == "ok" and text:
            payload_path = converted_text_path(converted_text_dir, str(row.get("article_ref")), str(source_role))
            result.update(write_converted_text(payload_path, text))
            result["parser_ready"] = True
            result["diagnostic_code"] = "parser_ready_converted_text"
            result["code"] = "parser_ready_converted_text"
        else:
            result["status"] = "low_quality" if quality["status"] != "empty" else "failed"
            result["terminal_state"] = result["status"]
            result["diagnostic_code"] = "converted_text_low_quality" if quality["status"] != "empty" else "empty_converted_text"
            result["code"] = result["diagnostic_code"]
            result["failure_reason"] = "; ".join(quality.get("warnings", [])) or "converted text is not parser-ready"
            result["fallback_reason"] = "no_substantive_body" if quality["status"] in {"empty", "low_quality"} else None
            result["parser_ready"] = False

    result["quality"] = quality
    result["structure_counts"] = structure
    return result


def file_hashes(paths: Iterable[Path]) -> dict[str, str | None]:
    output: dict[str, str | None] = {}
    for path in paths:
        try:
            output[rel(path)] = sha256_file(path)
        except OSError:
            output[rel(path)] = None
    return output


def build_conversion_artifacts(args: argparse.Namespace) -> None:
    source_summary = load_json(args.source_summary)
    rows = source_summary.get("results")
    if not isinstance(rows, list):
        raise ValueError("source acquisition summary missing results list")
    diagnostics = [convert_source_row(row, source_root=args.source_root, converted_text_dir=args.converted_text_dir) for row in rows if isinstance(row, dict)]
    counts = Counter(str(row.get("status")) for row in diagnostics)
    article_refs = sorted({str(row.get("article_ref")) for row in diagnostics if row.get("article_ref")})
    parser_ready_count = sum(1 for row in diagnostics if row.get("parser_ready") is True)
    article_count = source_summary.get("article_count") if isinstance(source_summary.get("article_count"), int) else len(article_refs)
    summary = {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "completed_with_diagnostics" if any(row.get("status") in {"blocked", "failed", "low_quality"} for row in diagnostics) else "completed",
        "exit_code_style_status": 0,
        "article_count": article_count,
        "variant_count": len(diagnostics),
        "parser_ready_count": parser_ready_count,
        "counts": dict(sorted(counts.items())),
        "source_summary_path": rel(args.source_summary),
        "source_summary_sha256": sha256_file(args.source_summary),
        "output_summary_path": rel(args.conversion_summary),
        "output_diagnostics_path": rel(args.conversion_diagnostics),
        "output_report_path": rel(args.conversion_report),
        "converted_text_dir": rel(args.converted_text_dir),
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
        "completed_at": "deterministic-local-replay",
        "provenance": {
            "schema_version": SCHEMA_VERSION,
            "command": ["uv", "run", "python", "scripts/verify_m029_unified_conversion_quality_boundary.py"],
            "cwd": str(ROOT),
            "milestone_id": MILESTONE_ID,
            "slice_id": SLICE_ID,
            "selection_id": SELECTION_ID,
            "input_paths": [rel(args.source_summary), rel(args.selection)],
            "input_hashes": file_hashes([args.source_summary, args.selection]),
            "output_paths": [rel(args.conversion_summary), rel(args.conversion_diagnostics), rel(args.conversion_report)],
            "network_fetch_attempted": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
            "graph_import_allowed": False,
            "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
        },
    }
    write_jsonl(args.conversion_diagnostics, diagnostics)
    atomic_write_text(args.conversion_report, render_report(summary))
    summary["provenance"]["output_hashes"] = file_hashes([args.conversion_diagnostics, args.conversion_report])
    write_json(args.conversion_summary, summary)


def conversion_artifacts_exist(args: argparse.Namespace) -> bool:
    return args.conversion_summary.exists() and args.conversion_diagnostics.exists() and args.conversion_report.exists()


def validate_no_payload_leakage(value: Any, *, serialized: str, where: str) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []

    def walk(node: Any, path: str) -> None:
        if isinstance(node, dict):
            for key, item in node.items():
                if key in FORBIDDEN_PAYLOAD_KEYS:
                    diagnostics.append(diagnostic("metadata_payload_key_leakage", f"metadata artifact includes forbidden payload key {key!r}", path=where, json_path=f"{path}.{key}"))
                walk(item, f"{path}.{key}")
        elif isinstance(node, list):
            for index, item in enumerate(node):
                walk(item, f"{path}[{index}]")

    walk(value, "$")
    lowered = serialized.lower()
    for snippet in FORBIDDEN_SNIPPETS:
        if snippet.lower() in lowered:
            diagnostics.append(diagnostic("metadata_payload_snippet_leakage", f"metadata artifact includes forbidden raw payload snippet {snippet!r}", path=where))
    return diagnostics


def false_flag_diagnostics(flags: Mapping[str, Any], *, where: str, row: Mapping[str, Any] | None = None) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    for key in sorted(UNSAFE_TRUE_FLAGS):
        if flags.get(key) is True:
            findings.append(diagnostic("unsafe_safety_flag_true", f"unsafe fail-closed flag is true: {key}", path=where, json_path=f"$.{key}", article_ref=str(row.get("article_ref")) if row and row.get("article_ref") else None, variant_id=str(row.get("variant_id")) if row and row.get("variant_id") else None, source_role=str(row.get("source_role")) if row and row.get("source_role") else None))
    return findings


def index_source_rows(source_summary: Mapping[str, Any]) -> tuple[dict[tuple[str, str, str], dict[str, Any]], list[dict[str, Any]]]:
    rows = source_summary.get("results")
    if not isinstance(rows, list):
        return {}, [diagnostic("missing_source_results", "S02 source summary is missing results list", path=SOURCE_SUMMARY_PATH)]
    indexed: dict[tuple[str, str, str], dict[str, Any]] = {}
    findings: list[dict[str, Any]] = []
    for index, row in enumerate(rows):
        if not isinstance(row, dict):
            findings.append(diagnostic("malformed_source_row", "S02 source row is not an object", json_path=f"$.results[{index}]"))
            continue
        key = (str(row.get("article_ref")), str(row.get("variant_id")), str(row.get("source_role")))
        indexed[key] = row
    return indexed, findings


def validate_source_bytes(row: Mapping[str, Any], source_row: Mapping[str, Any], *, source_root: Path) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    article_ref = str(row.get("article_ref")) if row.get("article_ref") else None
    variant_id = str(row.get("variant_id")) if row.get("variant_id") else None
    source_role = str(row.get("source_role")) if row.get("source_role") else None
    try:
        safe_relative_path(source_row.get("article_ref"), code_label="article_ref")
        source_path = source_artifact_path(source_root, source_row.get("local_path"))
    except ValueError as exc:
        return [diagnostic(f"source_{exc}", f"unsafe S02 source locator: {exc}", article_ref=article_ref, variant_id=variant_id, source_role=source_role)]
    if not source_path.exists() or not source_path.is_file():
        return [diagnostic("missing_source_artifact", "captured S02 source artifact is missing", path=source_path, article_ref=article_ref, variant_id=variant_id, source_role=source_role)]
    actual_hash = sha256_file(source_path)
    actual_size = source_path.stat().st_size
    expected_hash = source_row.get("sha256")
    expected_size = source_row.get("byte_size")
    if row.get("source_sha256") != expected_hash or actual_hash != expected_hash:
        findings.append(diagnostic("source_sha256_mismatch", "source artifact hash no longer matches S02 handoff and S03 metadata", path=source_path, article_ref=article_ref, variant_id=variant_id, source_role=source_role))
    if row.get("source_byte_size") != expected_size or actual_size != expected_size:
        findings.append(diagnostic("source_byte_size_mismatch", "source artifact byte size no longer matches S02 handoff and S03 metadata", path=source_path, article_ref=article_ref, variant_id=variant_id, source_role=source_role))
    if row.get("source_sha256_verified") is not True or row.get("source_byte_size_verified") is not True:
        findings.append(diagnostic("source_verification_not_recorded", "S03 row does not record verified source hash and byte size", article_ref=article_ref, variant_id=variant_id, source_role=source_role))
    return findings


def validate_converted_text(row: Mapping[str, Any], *, corpus_dir: Path) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    article_ref = str(row.get("article_ref")) if row.get("article_ref") else None
    variant_id = str(row.get("variant_id")) if row.get("variant_id") else None
    source_role = str(row.get("source_role")) if row.get("source_role") else None
    converted_path_value = row.get("converted_text_path")
    parser_ready = row.get("parser_ready") is True
    status = row.get("status")
    if not parser_ready:
        if converted_path_value not in {None, ""} or row.get("converted_text_sha256") is not None or row.get("converted_text_byte_size") not in {0, None}:
            findings.append(diagnostic("non_parser_ready_has_converted_payload", "non-parser-ready row points at converted text payload metadata", article_ref=article_ref, variant_id=variant_id, source_role=source_role))
        return findings
    if status != "converted" or row.get("diagnostic_code") != "parser_ready_converted_text":
        findings.append(diagnostic("parser_ready_without_conversion_quality", "parser-ready row lacks converted status and parser_ready_converted_text code", article_ref=article_ref, variant_id=variant_id, source_role=source_role))
    try:
        if isinstance(converted_path_value, str) and Path(converted_path_value).is_absolute():
            converted_path = Path(converted_path_value).resolve()
        else:
            converted_path = safe_under_root(ROOT, converted_path_value, code_label="converted_text_path")
    except ValueError as exc:
        return [*findings, diagnostic(f"unsafe_converted_text_path:{exc}", f"unsafe converted text locator: {exc}", article_ref=article_ref, variant_id=variant_id, source_role=source_role)]
    if not converted_path.exists() or not converted_path.is_file():
        findings.append(diagnostic("missing_converted_text_artifact", "parser-ready converted text artifact is missing", path=converted_path, article_ref=article_ref, variant_id=variant_id, source_role=source_role))
        return findings
    if not converted_path.resolve().is_relative_to(corpus_dir.resolve()):
        findings.append(diagnostic("converted_text_outside_corpus", "converted text artifact is outside the M029 unified corpus directory", path=converted_path, article_ref=article_ref, variant_id=variant_id, source_role=source_role))
    actual_hash = sha256_file(converted_path)
    actual_size = converted_path.stat().st_size
    if row.get("converted_text_sha256") != actual_hash:
        findings.append(diagnostic("converted_text_sha256_mismatch", "converted text artifact hash is stale", path=converted_path, article_ref=article_ref, variant_id=variant_id, source_role=source_role))
    if row.get("converted_text_byte_size") != actual_size:
        findings.append(diagnostic("converted_text_byte_size_mismatch", "converted text artifact byte size is stale", path=converted_path, article_ref=article_ref, variant_id=variant_id, source_role=source_role))
    if actual_size <= 0:
        findings.append(diagnostic("empty_converted_text_artifact", "parser-ready converted text artifact is empty", path=converted_path, article_ref=article_ref, variant_id=variant_id, source_role=source_role))
    return findings


def validate_row_semantics(row: Mapping[str, Any]) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    article_ref = str(row.get("article_ref")) if row.get("article_ref") else None
    variant_id = str(row.get("variant_id")) if row.get("variant_id") else None
    source_role = str(row.get("source_role")) if row.get("source_role") else None
    status = row.get("status")
    code = row.get("diagnostic_code")
    if status not in TERMINAL_STATUSES:
        findings.append(diagnostic("non_terminal_conversion_status", f"row has non-terminal status {status!r}", article_ref=article_ref, variant_id=variant_id, source_role=source_role))
    if not isinstance(code, str) or not code:
        findings.append(diagnostic("missing_stable_diagnostic_code", "row lacks stable diagnostic code", article_ref=article_ref, variant_id=variant_id, source_role=source_role))
    if row.get("network_fetch_attempted") is not False:
        findings.append(diagnostic("network_fetch_attempted_during_replay", "S03 conversion row attempted network access", article_ref=article_ref, variant_id=variant_id, source_role=source_role))
    findings.extend(false_flag_diagnostics(row, where="conversion row", row=row))
    flags = row.get("fail_closed_safety_flags")
    if isinstance(flags, dict):
        findings.extend(false_flag_diagnostics(flags, where="fail_closed_safety_flags", row=row))
    else:
        findings.append(diagnostic("missing_fail_closed_safety_flags", "row lacks fail-closed safety flags", article_ref=article_ref, variant_id=variant_id, source_role=source_role))
    context = row.get("safety_flag_context")
    if isinstance(context, dict):
        findings.extend(false_flag_diagnostics(context, where="safety_flag_context", row=row))
    else:
        findings.append(diagnostic("missing_safety_flag_context", "row lacks safety flag context", article_ref=article_ref, variant_id=variant_id, source_role=source_role))
    if source_role in ARXIV_METADATA_ROLES and status != "blocked":
        if row.get("parser_ready") is not False or status != "metadata_only" or code != "arxiv_abs_html_metadata_only" or row.get("fallback_reason") != "no_substantive_body":
            findings.append(diagnostic("arxiv_abs_parser_ready_claim", "arXiv abs/navigation source must emit fallback_reason=no_substantive_body and remain metadata-only", article_ref=article_ref, variant_id=variant_id, source_role=source_role))
    if status in {"low_quality", "failed", "metadata_only"} and row.get("parser_ready") is True:
        findings.append(diagnostic("low_quality_parser_ready_claim", "low-quality or metadata-only source cannot be parser-ready", article_ref=article_ref, variant_id=variant_id, source_role=source_role))
    quality = row.get("quality")
    if not isinstance(quality, dict) or not isinstance(quality.get("status"), str):
        findings.append(diagnostic("missing_quality_diagnosis", "row lacks quality status diagnosis", article_ref=article_ref, variant_id=variant_id, source_role=source_role))
    return findings


def selected_article_count(selection: Mapping[str, Any]) -> int:
    rows = selection.get("articles")
    return len(rows) if isinstance(rows, list) else 0


def run_negative_cases(args: argparse.Namespace) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    rows = load_json(args.conversion_summary).get("results")
    if not isinstance(rows, list) or not rows:
        return [diagnostic("negative_case_setup_missing", "negative cases require conversion result rows")]
    parser_ready = next((row for row in rows if isinstance(row, dict) and row.get("parser_ready") is True), None)
    if not isinstance(parser_ready, dict):
        return [diagnostic("negative_case_parser_ready_missing", "negative cases require at least one parser-ready row")]
    unsafe = dict(parser_ready)
    unsafe["converted_text_path"] = "../escape.txt"
    if not any(item["diagnostic_code"].startswith("unsafe_converted_text_path") for item in validate_converted_text(unsafe, corpus_dir=args.corpus_dir)):
        findings.append(diagnostic("negative_case_unsafe_path_not_detected", "unsafe converted_text_path was not detected"))
    stale = dict(parser_ready)
    stale["converted_text_sha256"] = "0" * 64
    if not any(item["diagnostic_code"] == "converted_text_sha256_mismatch" for item in validate_converted_text(stale, corpus_dir=args.corpus_dir)):
        findings.append(diagnostic("negative_case_converted_hash_not_detected", "stale converted hash was not detected"))
    unsafe_flags = dict(parser_ready)
    unsafe_flags["fail_closed_safety_flags"] = dict(unsafe_flags.get("fail_closed_safety_flags", {}), graph_import_allowed=True)
    if not any(item["diagnostic_code"] == "unsafe_safety_flag_true" for item in validate_row_semantics(unsafe_flags)):
        findings.append(diagnostic("negative_case_unsafe_flag_not_detected", "unsafe safety flag was not detected"))
    return findings


def render_report(summary: Mapping[str, Any]) -> str:
    return "\n".join(
        [
            "# M029 Unified Conversion Quality Report",
            "",
            "This report is metadata-only and does not embed raw article text, source markup, PDF bytes, or converted payloads.",
            "",
            f"- Schema: `{summary.get('schema_version')}`",
            f"- Status: `{summary.get('status')}`",
            "- Network fetch attempted: `False`",
            "- Production import attempted: `False`",
            "- LadybugDB written: `False`",
            f"- Article count: {summary.get('article_count')}",
            f"- Variant count: {summary.get('variant_count')}",
            f"- Parser ready count: {summary.get('parser_ready_count')}",
            f"- Counts: `{dict(summary.get('counts', {}))}`",
            f"- Diagnostics: `{summary.get('diagnostic_count')}`",
            "",
            "## Failure Modes",
            "",
            "Unsafe paths, missing sources, stale source hashes, stale converted hashes, metadata-only abstract/navigation pages, empty extraction, low-quality extraction, and unsafe flags all fail closed with stable diagnostic codes.",
            "",
            "## Load Profile",
            "",
            f"PDF extraction is bounded to {MAX_PDF_PAGES} pages and {MAX_TEXT_CHARS} characters per variant; all source hashing is local and streamed.",
            "",
            "## Negative Tests",
            "",
            "Covered by `tests/test_m029_conversion_quality_boundary.py`: metadata-only abstract/navigation markdown, low-quality conversion, unsafe paths, source drift, converted payload drift, and unsafe safety flags.",
            "",
        ]
    )


def verify(args: argparse.Namespace) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    if not conversion_artifacts_exist(args):
        build_conversion_artifacts(args)

    diagnostics: list[dict[str, Any]] = []
    summary = load_json(args.conversion_summary)
    source_summary = load_json(args.source_summary)
    selection = load_json(args.selection)
    jsonl_rows = load_jsonl(args.conversion_diagnostics)
    report_text = args.conversion_report.read_text(encoding="utf-8")

    diagnostics.extend(validate_no_payload_leakage(summary, serialized=json.dumps(summary, sort_keys=True), where=rel(args.conversion_summary)))
    diagnostics.extend(validate_no_payload_leakage(jsonl_rows, serialized=json.dumps(jsonl_rows, sort_keys=True), where=rel(args.conversion_diagnostics)))
    diagnostics.extend(validate_no_payload_leakage({"report": report_text}, serialized=report_text, where=rel(args.conversion_report)))

    expected_top = {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "graph_import_allowed": False,
    }
    for key, expected in expected_top.items():
        if summary.get(key) != expected:
            diagnostics.append(diagnostic("summary_contract_mismatch", f"summary {key} is {summary.get(key)!r}, expected {expected!r}", path=args.conversion_summary, json_path=f"$.{key}"))
    diagnostics.extend(false_flag_diagnostics(summary, where=rel(args.conversion_summary)))
    flags = summary.get("fail_closed_safety_flags")
    if isinstance(flags, dict):
        diagnostics.extend(false_flag_diagnostics(flags, where="$.fail_closed_safety_flags"))
    else:
        diagnostics.append(diagnostic("missing_fail_closed_safety_flags", "summary lacks fail-closed safety flags", path=args.conversion_summary, json_path="$.fail_closed_safety_flags"))

    if source_summary.get("milestone_id") != MILESTONE_ID or source_summary.get("slice_id") != SOURCE_SLICE_ID:
        diagnostics.append(diagnostic("source_summary_contract_mismatch", "S02 source summary has unexpected milestone or slice", path=args.source_summary))
    if summary.get("source_summary_sha256") != sha256_file(args.source_summary):
        diagnostics.append(diagnostic("source_summary_sha256_mismatch", "S03 summary source_summary_sha256 is stale", path=args.source_summary, json_path="$.source_summary_sha256"))

    rows = summary.get("results")
    if not isinstance(rows, list):
        rows = []
        diagnostics.append(diagnostic("missing_conversion_results", "conversion summary is missing results list", path=args.conversion_summary, json_path="$.results"))
    if len(rows) != len(jsonl_rows):
        diagnostics.append(diagnostic("diagnostic_row_count_mismatch", "summary results and JSONL diagnostics have different row counts", path=args.conversion_diagnostics))
    if json.dumps(rows, sort_keys=True) != json.dumps(jsonl_rows, sort_keys=True):
        diagnostics.append(diagnostic("diagnostic_jsonl_summary_mismatch", "JSONL diagnostics differ from summary results", path=args.conversion_diagnostics))

    expected_article_count = args.expected_article_count or selected_article_count(selection) or EXPECTED_ARTICLE_COUNT
    expected_variant_count = args.expected_variant_count or source_summary.get("variant_count") or EXPECTED_VARIANT_COUNT
    article_refs = {str(row.get("article_ref")) for row in rows if isinstance(row, dict) and row.get("article_ref")}
    if summary.get("article_count") != expected_article_count:
        diagnostics.append(diagnostic("article_count_mismatch", f"expected {expected_article_count} selected articles", path=args.conversion_summary, json_path="$.article_count"))
    if summary.get("variant_count") != len(rows) or len(rows) != expected_variant_count:
        diagnostics.append(diagnostic("variant_count_mismatch", f"expected {expected_variant_count} source variants", path=args.conversion_summary, json_path="$.variant_count"))
    counts = Counter(str(row.get("status")) for row in rows if isinstance(row, dict))
    if dict(sorted(counts.items())) != summary.get("counts"):
        diagnostics.append(diagnostic("status_counts_mismatch", "summary counts do not match conversion rows", path=args.conversion_summary, json_path="$.counts"))
    parser_ready_count = sum(1 for row in rows if isinstance(row, dict) and row.get("parser_ready") is True)
    if summary.get("parser_ready_count") != parser_ready_count:
        diagnostics.append(diagnostic("parser_ready_count_mismatch", "summary parser_ready_count does not match conversion rows", path=args.conversion_summary, json_path="$.parser_ready_count"))

    source_index, source_findings = index_source_rows(source_summary)
    diagnostics.extend(source_findings)
    by_article: dict[str, set[str]] = defaultdict(set)
    for row in rows:
        if not isinstance(row, dict):
            diagnostics.append(diagnostic("malformed_conversion_row", "conversion row is not an object", path=args.conversion_summary))
            continue
        key = (str(row.get("article_ref")), str(row.get("variant_id")), str(row.get("source_role")))
        source_row = source_index.get(key)
        if source_row is None:
            diagnostics.append(diagnostic("source_row_missing_for_conversion", "S03 row has no matching S02 source row", article_ref=key[0], variant_id=key[1], source_role=key[2]))
            continue
        if row.get("status") != "blocked" and key[0] not in {"", "None"}:
            by_article[key[0]].add(key[2])
        diagnostics.extend(validate_row_semantics(row))
        if source_row.get("status") == CAPTURED_STATUS:
            diagnostics.extend(validate_source_bytes(row, source_row, source_root=args.source_root))
        diagnostics.extend(validate_converted_text(row, corpus_dir=args.corpus_dir))

    for article_ref, roles in sorted(by_article.items()):
        if any(role in ARXIV_METADATA_ROLES for role in roles) and not any(role in ARXIV_FULL_TEXT_ROLES for role in roles):
            diagnostics.append(diagnostic("missing_arxiv_parser_ready_fallback", "arXiv abstract capture lacks parser-ready HTML/PDF fallback variant", article_ref=article_ref))
    if args.check_low_quality_sources and not any(isinstance(row, dict) and row.get("status") in {"low_quality", "metadata_only", "failed"} and row.get("parser_ready") is False for row in rows):
        diagnostics.append(diagnostic("low_quality_source_case_missing", "conversion boundary lacks low-quality or metadata-only fail-closed rows"))
    if args.require_no_substantive_body_diagnostic and not any(isinstance(row, dict) and row.get("fallback_reason") == "no_substantive_body" for row in rows):
        diagnostics.append(diagnostic("no_substantive_body_diagnostic_missing", "fallback_reason=no_substantive_body was not emitted"))
    if args.check_negative_cases:
        diagnostics.extend(run_negative_cases(args))

    for heading in ["Failure Modes", "Load Profile", "Negative Tests"]:
        if heading not in report_text:
            diagnostics.append(diagnostic("report_section_missing", f"conversion report missing {heading} section", path=args.conversion_report))

    verifier_summary = {
        "schema_version": VERIFIER_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "passed" if not diagnostics else "failed",
        "diagnostic_count": len(diagnostics),
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_import_allowed": False,
    }
    return verifier_summary, diagnostics


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", type=Path, default=SELECTION_PATH)
    parser.add_argument("--source-summary", type=Path, default=SOURCE_SUMMARY_PATH)
    parser.add_argument("--conversion-summary", "--summary", dest="conversion_summary", type=Path, default=SUMMARY_PATH)
    parser.add_argument("--conversion-diagnostics", "--diagnostics", dest="conversion_diagnostics", type=Path, default=DIAGNOSTICS_PATH)
    parser.add_argument("--conversion-report", "--report", dest="conversion_report", type=Path, default=REPORT_PATH)
    parser.add_argument("--source-root", type=Path, default=SOURCE_ROOT)
    parser.add_argument("--corpus-dir", type=Path, default=CORPUS_DIR)
    parser.add_argument("--converted-text-dir", type=Path, default=CONVERTED_TEXT_DIR)
    parser.add_argument("--expected-article-count", type=int, default=0)
    parser.add_argument("--expected-variant-count", type=int, default=0)
    parser.add_argument("--check-negative-cases", action="store_true")
    parser.add_argument("--check-low-quality-sources", action="store_true")
    parser.add_argument("--require-no-substantive-body-diagnostic", action="store_true")
    parser.add_argument("--require-no-network", action="store_true", help="Compatibility flag; local-only/no-network is always enforced.")
    parser.add_argument("--require-no-import-flags", action="store_true", help="Compatibility flag; fail-closed import flags are always checked.")
    parser.add_argument("--reject-empty-semantic-body", action="store_true", help="Compatibility flag; empty/low-quality bodies are always rejected.")
    parser.add_argument("--require-fallback-reasons", action="store_true", help="Compatibility flag; non-parser-ready rows are always required to carry fallback diagnostics.")
    parser.add_argument("--check-parser-ready-gates", action="store_true", help="Compatibility flag; parser-ready gates are always checked.")
    return parser.parse_args(argv[1:])


def main(argv: list[str] | None = None) -> int:
    argv = argv or sys.argv
    args = parse_args(argv)
    try:
        verifier_summary, diagnostics = verify(args)
    except Exception as exc:
        sys.stderr.write(f"M029 unified conversion quality verifier failed: {exc}\n")
        return 1
    if diagnostics:
        sys.stderr.write(json.dumps({"summary": verifier_summary, "diagnostics": diagnostics}, indent=2, sort_keys=True) + "\n")
        return 1
    sys.stdout.write(json.dumps(verifier_summary, indent=2, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
