#!/usr/bin/env python3
"""Replay-safe verifier for the M027 conversion-quality boundary.

The verifier is fixed to the M027 mixed-source corpus by default and validates
metadata-only S03 conversion artifacts against the frozen S02 source-acquisition
handoff. It never fetches network sources, re-runs conversion, imports graph
state, or writes LadybugDB/production state.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections import Counter, defaultdict
from collections.abc import Mapping
from pathlib import Path, PurePosixPath
from typing import Any

MILESTONE_ID = "M027-aakeky"
SLICE_ID = "S03"
SOURCE_SLICE_ID = "S02"
SELECTION_ID = "m027-mixed-source-corpus-v1"
SCHEMA_VERSION = "m027-conversion-quality.v1"
VERIFIER_SCHEMA_VERSION = "m027-conversion-quality-verifier.v1"
ROOT = Path(__file__).resolve().parents[1]
CORPUS_DIR = ROOT / "data" / "article_corpora" / SELECTION_ID
CATALOG_ARTICLE_ROOT = ROOT / "data" / "article_catalog" / "article_catalog"
SOURCE_SUMMARY_PATH = CORPUS_DIR / "source-acquisition-summary.json"
SUMMARY_PATH = CORPUS_DIR / "conversion-quality-summary.json"
DIAGNOSTICS_PATH = CORPUS_DIR / "conversion-quality-diagnostics.jsonl"
REPORT_PATH = CORPUS_DIR / "conversion-quality-report.md"

EXPECTED_ARTICLE_COUNT = 6
EXPECTED_VARIANT_COUNT = 11
TERMINAL_STATUSES = {"converted", "metadata_only", "blocked", "failed", "low_quality"}
ARXIV_FULL_TEXT_ROLES = {"arxiv_pdf"}
ARXIV_METADATA_ROLES = {"arxiv_abs_page"}
NATURE_HTML_ROLES = {"nature_html"}

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
    if (
        normalized.is_absolute()
        or ".." in normalized.parts
        or any(part == "" for part in normalized.parts)
    ):
        raise ValueError(f"unsafe_{code_label}")
    return normalized


def safe_under_root(root: Path, relative_path: Any, *, code_label: str) -> Path:
    normalized = safe_relative_path(relative_path, code_label=code_label)
    root_resolved = root.resolve()
    resolved = (root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root_resolved):
        raise ValueError(f"{code_label}_escapes_root")
    return resolved


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


def validate_no_payload_leakage(value: Any, *, serialized: str, where: str) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []

    def walk(node: Any, path: str) -> None:
        if isinstance(node, dict):
            for key, item in node.items():
                if key in FORBIDDEN_PAYLOAD_KEYS:
                    diagnostics.append(
                        diagnostic(
                            "metadata_payload_key_leakage",
                            f"metadata artifact includes forbidden payload key {key!r}",
                            path=where,
                            json_path=f"{path}.{key}",
                        )
                    )
                walk(item, f"{path}.{key}")
        elif isinstance(node, list):
            for index, item in enumerate(node):
                walk(item, f"{path}[{index}]")

    walk(value, "$")
    lowered = serialized.lower()
    for snippet in FORBIDDEN_SNIPPETS:
        if snippet.lower() in lowered:
            diagnostics.append(
                diagnostic(
                    "metadata_payload_snippet_leakage",
                    f"metadata artifact includes forbidden raw payload snippet {snippet!r}",
                    path=where,
                )
            )
    return diagnostics


def false_flag_diagnostics(
    flags: Mapping[str, Any], *, where: str, row: Mapping[str, Any] | None = None
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    for key in sorted(UNSAFE_TRUE_FLAGS):
        if flags.get(key) is True:
            findings.append(
                diagnostic(
                    "unsafe_safety_flag_true",
                    f"unsafe fail-closed flag is true: {key}",
                    path=where,
                    json_path=f"$.{key}",
                    article_ref=str(row.get("article_ref"))
                    if row and row.get("article_ref")
                    else None,
                    variant_id=str(row.get("variant_id"))
                    if row and row.get("variant_id")
                    else None,
                    source_role=str(row.get("source_role"))
                    if row and row.get("source_role")
                    else None,
                )
            )
    return findings


def index_source_rows(
    source_summary: Mapping[str, Any],
) -> tuple[dict[tuple[str, str, str], dict[str, Any]], list[dict[str, Any]]]:
    rows = source_summary.get("results")
    if not isinstance(rows, list):
        return {}, [
            diagnostic(
                "missing_source_results",
                "S02 source summary is missing results list",
                path=SOURCE_SUMMARY_PATH,
            )
        ]
    indexed: dict[tuple[str, str, str], dict[str, Any]] = {}
    findings: list[dict[str, Any]] = []
    for index, row in enumerate(rows):
        if not isinstance(row, dict):
            findings.append(
                diagnostic(
                    "malformed_source_row",
                    "S02 source row is not an object",
                    json_path=f"$.results[{index}]",
                )
            )
            continue
        key = (str(row.get("article_ref")), str(row.get("variant_id")), str(row.get("source_role")))
        indexed[key] = row
    return indexed, findings


def validate_source_bytes(
    row: Mapping[str, Any],
    source_row: Mapping[str, Any],
    *,
    source_root: Path,
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    article_ref = str(row.get("article_ref")) if row.get("article_ref") else None
    variant_id = str(row.get("variant_id")) if row.get("variant_id") else None
    source_role = str(row.get("source_role")) if row.get("source_role") else None
    try:
        article_path = safe_relative_path(source_row.get("article_ref"), code_label="article_ref")
        local_path = safe_relative_path(source_row.get("local_path"), code_label="local_path")
        root = source_root.resolve()
        source_path = (root / article_path.as_posix() / local_path.as_posix()).resolve()
        if not source_path.is_relative_to(root):
            raise ValueError("source_path_escapes_root")
    except ValueError as exc:
        return [
            diagnostic(
                f"source_{exc}",
                f"unsafe S02 source locator: {exc}",
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            )
        ]
    if not source_path.exists() or not source_path.is_file():
        return [
            diagnostic(
                "missing_source_artifact",
                "captured S02 source artifact is missing",
                path=source_path,
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            )
        ]
    actual_hash = sha256_file(source_path)
    actual_size = source_path.stat().st_size
    expected_hash = source_row.get("sha256")
    expected_size = source_row.get("byte_size")
    if row.get("source_sha256") != expected_hash or actual_hash != expected_hash:
        findings.append(
            diagnostic(
                "source_sha256_mismatch",
                "source artifact hash no longer matches S02 handoff and S03 metadata",
                path=source_path,
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            )
        )
    if row.get("source_byte_size") != expected_size or actual_size != expected_size:
        findings.append(
            diagnostic(
                "source_byte_size_mismatch",
                "source artifact byte size no longer matches S02 handoff and S03 metadata",
                path=source_path,
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            )
        )
    if (
        row.get("source_sha256_verified") is not True
        or row.get("source_byte_size_verified") is not True
    ):
        findings.append(
            diagnostic(
                "source_verification_not_recorded",
                "S03 row does not record verified source hash and byte size",
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            )
        )
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
        if (
            converted_path_value not in {None, ""}
            or row.get("converted_text_sha256") is not None
            or row.get("converted_text_byte_size") not in {0, None}
        ):
            findings.append(
                diagnostic(
                    "non_parser_ready_has_converted_payload",
                    "non-parser-ready row points at converted text payload metadata",
                    article_ref=article_ref,
                    variant_id=variant_id,
                    source_role=source_role,
                )
            )
        return findings
    if status != "converted" or row.get("diagnostic_code") != "parser_ready_converted_text":
        findings.append(
            diagnostic(
                "parser_ready_without_conversion_quality",
                "parser-ready row lacks converted status and parser_ready_converted_text code",
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            )
        )
    try:
        if isinstance(converted_path_value, str) and Path(converted_path_value).is_absolute():
            converted_path = Path(converted_path_value).resolve()
        else:
            converted_path = safe_under_root(
                ROOT, converted_path_value, code_label="converted_text_path"
            )
    except ValueError as exc:
        return [
            *findings,
            diagnostic(
                f"unsafe_converted_text_path:{exc}",
                f"unsafe converted text locator: {exc}",
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            ),
        ]
    if not converted_path.exists() or not converted_path.is_file():
        findings.append(
            diagnostic(
                "missing_converted_text_artifact",
                "parser-ready converted text artifact is missing",
                path=converted_path,
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            )
        )
        return findings
    if not converted_path.resolve().is_relative_to(corpus_dir.resolve()):
        findings.append(
            diagnostic(
                "converted_text_outside_corpus",
                "converted text artifact is outside the M027 corpus directory",
                path=converted_path,
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            )
        )
    actual_hash = sha256_file(converted_path)
    actual_size = converted_path.stat().st_size
    if row.get("converted_text_sha256") != actual_hash:
        findings.append(
            diagnostic(
                "converted_text_sha256_mismatch",
                "converted text artifact hash is stale",
                path=converted_path,
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            )
        )
    if row.get("converted_text_byte_size") != actual_size:
        findings.append(
            diagnostic(
                "converted_text_byte_size_mismatch",
                "converted text artifact byte size is stale",
                path=converted_path,
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            )
        )
    if actual_size <= 0:
        findings.append(
            diagnostic(
                "empty_converted_text_artifact",
                "parser-ready converted text artifact is empty",
                path=converted_path,
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            )
        )
    return findings


def validate_row_semantics(row: Mapping[str, Any]) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    article_ref = str(row.get("article_ref")) if row.get("article_ref") else None
    variant_id = str(row.get("variant_id")) if row.get("variant_id") else None
    source_role = str(row.get("source_role")) if row.get("source_role") else None
    status = row.get("status")
    code = row.get("diagnostic_code")
    if status not in TERMINAL_STATUSES:
        findings.append(
            diagnostic(
                "non_terminal_conversion_status",
                f"row has non-terminal status {status!r}",
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            )
        )
    if not isinstance(code, str) or not code:
        findings.append(
            diagnostic(
                "missing_stable_diagnostic_code",
                "row lacks stable diagnostic code",
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            )
        )
    if row.get("network_fetch_attempted") is not False:
        findings.append(
            diagnostic(
                "network_fetch_attempted_during_replay",
                "S03 conversion row attempted network access",
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            )
        )
    findings.extend(false_flag_diagnostics(row, where="conversion row", row=row))
    flags = row.get("fail_closed_safety_flags")
    if isinstance(flags, dict):
        findings.extend(false_flag_diagnostics(flags, where="fail_closed_safety_flags", row=row))
    else:
        findings.append(
            diagnostic(
                "missing_fail_closed_safety_flags",
                "row lacks fail-closed safety flags",
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            )
        )
    context = row.get("safety_flag_context")
    if isinstance(context, dict):
        findings.extend(false_flag_diagnostics(context, where="safety_flag_context", row=row))
    else:
        findings.append(
            diagnostic(
                "missing_safety_flag_context",
                "row lacks safety flag context",
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            )
        )

    if source_role in ARXIV_METADATA_ROLES:
        if (
            row.get("parser_ready") is not False
            or status != "metadata_only"
            or code != "arxiv_abs_html_metadata_only"
        ):
            findings.append(
                diagnostic(
                    "arxiv_abs_parser_ready_claim",
                    "arXiv abs HTML must remain metadata-only and not parser-ready",
                    article_ref=article_ref,
                    variant_id=variant_id,
                    source_role=source_role,
                )
            )
    if source_role in NATURE_HTML_ROLES:
        structure = row.get("structure_counts")
        if (
            not isinstance(structure, dict)
            or structure.get("paragraph_count", 0) <= 0
            or (
                structure.get("article_tag_count", 0) <= 0
                and structure.get("main_tag_count", 0) <= 0
            )
        ):
            findings.append(
                diagnostic(
                    "nature_html_structure_signals_missing",
                    "Nature HTML row lacks article/body structure signals",
                    article_ref=article_ref,
                    variant_id=variant_id,
                    source_role=source_role,
                )
            )
    quality = row.get("quality")
    if not isinstance(quality, dict) or not isinstance(quality.get("status"), str):
        findings.append(
            diagnostic(
                "missing_quality_diagnosis",
                "row lacks quality status diagnosis",
                article_ref=article_ref,
                variant_id=variant_id,
                source_role=source_role,
            )
        )
    return findings


def verify(args: argparse.Namespace) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    diagnostics: list[dict[str, Any]] = []
    summary = load_json(args.summary)
    source_summary = load_json(args.source_summary)
    jsonl_rows = load_jsonl(args.diagnostics)
    report_text = args.report.read_text(encoding="utf-8")

    diagnostics.extend(
        validate_no_payload_leakage(
            summary, serialized=json.dumps(summary, sort_keys=True), where=rel(args.summary)
        )
    )
    diagnostics.extend(
        validate_no_payload_leakage(
            jsonl_rows,
            serialized=json.dumps(jsonl_rows, sort_keys=True),
            where=rel(args.diagnostics),
        )
    )
    diagnostics.extend(
        validate_no_payload_leakage(
            {"report": report_text}, serialized=report_text, where=rel(args.report)
        )
    )

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
            diagnostics.append(
                diagnostic(
                    "summary_contract_mismatch",
                    f"summary {key} is {summary.get(key)!r}, expected {expected!r}",
                    path=args.summary,
                    json_path=f"$.{key}",
                )
            )
    diagnostics.extend(false_flag_diagnostics(summary, where=rel(args.summary)))
    flags = summary.get("fail_closed_safety_flags")
    if isinstance(flags, dict):
        diagnostics.extend(false_flag_diagnostics(flags, where="$.fail_closed_safety_flags"))
    else:
        diagnostics.append(
            diagnostic(
                "missing_fail_closed_safety_flags",
                "summary lacks fail-closed safety flags",
                path=args.summary,
                json_path="$.fail_closed_safety_flags",
            )
        )

    if (
        source_summary.get("milestone_id") != MILESTONE_ID
        or source_summary.get("slice_id") != SOURCE_SLICE_ID
    ):
        diagnostics.append(
            diagnostic(
                "source_summary_contract_mismatch",
                "S02 source summary has unexpected milestone or slice",
                path=args.source_summary,
            )
        )
    if summary.get("source_summary_sha256") != sha256_file(args.source_summary):
        diagnostics.append(
            diagnostic(
                "source_summary_sha256_mismatch",
                "S03 summary source_summary_sha256 is stale",
                path=args.source_summary,
                json_path="$.source_summary_sha256",
            )
        )

    rows = summary.get("results")
    if not isinstance(rows, list):
        rows = []
        diagnostics.append(
            diagnostic(
                "missing_conversion_results",
                "conversion summary is missing results list",
                path=args.summary,
                json_path="$.results",
            )
        )
    if len(rows) != len(jsonl_rows):
        diagnostics.append(
            diagnostic(
                "diagnostic_row_count_mismatch",
                "summary results and JSONL diagnostics have different row counts",
                path=args.diagnostics,
            )
        )
    if json.dumps(rows, sort_keys=True) != json.dumps(jsonl_rows, sort_keys=True):
        diagnostics.append(
            diagnostic(
                "diagnostic_jsonl_summary_mismatch",
                "JSONL diagnostics differ from summary results",
                path=args.diagnostics,
            )
        )

    article_refs = {
        str(row.get("article_ref"))
        for row in rows
        if isinstance(row, dict) and row.get("article_ref")
    }
    if (
        summary.get("article_count") != len(article_refs)
        or len(article_refs) != args.expected_article_count
    ):
        diagnostics.append(
            diagnostic(
                "article_count_mismatch",
                f"expected {args.expected_article_count} selected articles",
                path=args.summary,
                json_path="$.article_count",
            )
        )
    if summary.get("variant_count") != len(rows) or len(rows) != args.expected_variant_count:
        diagnostics.append(
            diagnostic(
                "variant_count_mismatch",
                f"expected {args.expected_variant_count} source variants",
                path=args.summary,
                json_path="$.variant_count",
            )
        )
    counts = Counter(str(row.get("status")) for row in rows if isinstance(row, dict))
    if dict(sorted(counts.items())) != summary.get("counts"):
        diagnostics.append(
            diagnostic(
                "status_counts_mismatch",
                "summary counts do not match conversion rows",
                path=args.summary,
                json_path="$.counts",
            )
        )
    parser_ready_count = sum(
        1 for row in rows if isinstance(row, dict) and row.get("parser_ready") is True
    )
    if summary.get("parser_ready_count") != parser_ready_count:
        diagnostics.append(
            diagnostic(
                "parser_ready_count_mismatch",
                "summary parser_ready_count does not match conversion rows",
                path=args.summary,
                json_path="$.parser_ready_count",
            )
        )

    source_index, source_findings = index_source_rows(source_summary)
    diagnostics.extend(source_findings)
    by_article: dict[str, set[str]] = defaultdict(set)
    for row in rows:
        if not isinstance(row, dict):
            diagnostics.append(
                diagnostic(
                    "malformed_conversion_row", "conversion row is not an object", path=args.summary
                )
            )
            continue
        key = (str(row.get("article_ref")), str(row.get("variant_id")), str(row.get("source_role")))
        source_row = source_index.get(key)
        if source_row is None:
            diagnostics.append(
                diagnostic(
                    "source_row_missing_for_conversion",
                    "S03 row has no matching S02 source row",
                    article_ref=key[0],
                    variant_id=key[1],
                    source_role=key[2],
                )
            )
            continue
        by_article[key[0]].add(key[2])
        diagnostics.extend(validate_row_semantics(row))
        diagnostics.extend(validate_source_bytes(row, source_row, source_root=args.source_root))
        diagnostics.extend(validate_converted_text(row, corpus_dir=args.corpus_dir))

    for article_ref, roles in sorted(by_article.items()):
        if any(role in ARXIV_METADATA_ROLES for role in roles) and not any(
            role in ARXIV_FULL_TEXT_ROLES for role in roles
        ):
            diagnostics.append(
                diagnostic(
                    "missing_arxiv_pdf_fallback",
                    "arXiv abs capture lacks parser-ready PDF fallback variant",
                    article_ref=article_ref,
                )
            )
    for article_ref in sorted(article_refs):
        article_rows = [
            row for row in rows if isinstance(row, dict) and row.get("article_ref") == article_ref
        ]
        if not any(row.get("parser_ready") is True for row in article_rows):
            diagnostics.append(
                diagnostic(
                    "article_without_parser_ready_fallback",
                    "selected article has no parser-ready converted text fallback",
                    article_ref=article_ref,
                )
            )

    report_required = ["Failure Modes", "Load Profile", "Negative Tests"]
    for heading in report_required:
        if heading not in report_text:
            diagnostics.append(
                diagnostic(
                    "report_section_missing",
                    f"conversion report missing {heading} section",
                    path=args.report,
                )
            )

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
    parser.add_argument("--source-summary", type=Path, default=SOURCE_SUMMARY_PATH)
    parser.add_argument("--summary", type=Path, default=SUMMARY_PATH)
    parser.add_argument("--diagnostics", type=Path, default=DIAGNOSTICS_PATH)
    parser.add_argument("--report", type=Path, default=REPORT_PATH)
    parser.add_argument("--source-root", type=Path, default=CATALOG_ARTICLE_ROOT)
    parser.add_argument("--corpus-dir", type=Path, default=CORPUS_DIR)
    parser.add_argument("--expected-article-count", type=int, default=EXPECTED_ARTICLE_COUNT)
    parser.add_argument("--expected-variant-count", type=int, default=EXPECTED_VARIANT_COUNT)
    return parser.parse_args(argv[1:])


def main(argv: list[str] | None = None) -> int:
    argv = argv or sys.argv
    args = parse_args(argv)
    try:
        verifier_summary, diagnostics = verify(args)
    except Exception as exc:
        sys.stderr.write(f"M027 conversion quality verifier failed: {exc}\n")
        return 1
    if diagnostics:
        sys.stderr.write(
            json.dumps(
                {"summary": verifier_summary, "diagnostics": diagnostics}, indent=2, sort_keys=True
            )
            + "\n"
        )
        return 1
    sys.stdout.write(json.dumps(verifier_summary, indent=2, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
