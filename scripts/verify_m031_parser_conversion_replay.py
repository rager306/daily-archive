#!/usr/bin/env python3
"""Validate-only closeout verifier for M031 parser conversion replay artifacts.

The verifier consumes materialized S03 conversion artifacts and independently
checks linkage to S02 loader evidence, local path confinement, source and
converted-text hashes, refusal/status contracts, redaction, and fail-closed
safety flags. It never converts content, fetches network sources, imports graph
state, or writes LadybugDB/production state.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections import Counter
from collections.abc import Mapping
from pathlib import Path, PurePosixPath
from typing import Any

MILESTONE_ID = "M031-vwpd8e"
SLICE_ID = "S03"
SOURCE_SLICE_ID = "S02"
SELECTION_ID = "m031-catalog-backed-replay-v1"
CONVERSION_SCHEMA_VERSION = "m031-parser-conversion-replay.v1"
VERIFIER_SCHEMA_VERSION = "m031-parser-conversion-closeout-verifier.v1"

TERMINAL_STATUSES = {"converted", "metadata_only", "blocked", "failed", "low_quality"}
METADATA_ONLY_ROLES = {"arxiv_abs_page", "arxiv_abs_url"}
HTML_ROLES = {"arxiv_html", "publisher_html", "web_article_html", "nature_html"}
EXPECTED_FALSE_FLAGS = {
    "network_fetch_attempted",
    "arxiv2md_invoked",
    "md_converter_invoked",
    "external_cache_read",
    "external_cache_written",
    "raw_article_text_embedded",
    "raw_article_html_embedded",
    "raw_pdf_bytes_embedded",
    "binary_payload_embedded",
    "base64_payload_embedded",
    "parser_ready_claimed_without_conversion",
    "chunk_ready_claimed",
    "kg_readiness_claimed",
    "graph_import_allowed",
    "trusted_kg_import_allowed",
    "production_import_attempted",
    "ladybugdb_written",
    "graph_write_attempted",
    "production_persistence_attempted",
    "raw_text_embedded",
    "raw_binary_embedded",
    "raw_payload_embedded_in_metadata",
    "production_ladybugdb_write_allowed",
}
FORBIDDEN_PAYLOAD_KEYS = {
    "text",
    "raw_text",
    "raw_html",
    "html",
    "pdf",
    "raw_pdf",
    "bytes",
    "binary",
    "base64",
    "payload",
    "body",
    "content",
    "source_payload",
    "converted_text",
}
FORBIDDEN_SNIPPETS = {
    "<html",
    "</html",
    "%PDF-",
    "base64,",
    "RAW_PDF_SECRET",
    "RAW_HTML_SECRET",
    "RAW_ARXIV_ABS_SECRET",
}
REQUIRED_REPORT_SECTIONS = ("## Failure Modes", "## Load Profile", "## Negative Tests")


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"expected JSON object at {path}")
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


def atomic_write_text(path: Path, value: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(value, encoding="utf-8")


def write_json(path: Path, payload: Mapping[str, Any]) -> None:
    atomic_write_text(path, json.dumps(payload, indent=2, sort_keys=False) + "\n")


def write_jsonl(path: Path, rows: list[Mapping[str, Any]]) -> None:
    atomic_write_text(path, "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows))


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
        or any(part in ("", ".") for part in normalized.parts)
    ):
        raise ValueError(f"unsafe_{code_label}")
    return normalized


def safe_under_root(root: Path, value: Any, *, code_label: str) -> Path:
    root_resolved = root.resolve()
    if isinstance(value, str) and Path(value).is_absolute():
        resolved = Path(value).resolve()
    else:
        resolved = (
            root_resolved / safe_relative_path(value, code_label=code_label).as_posix()
        ).resolve()
    if not resolved.is_relative_to(root_resolved):
        raise ValueError(f"{code_label}_escapes_root")
    return resolved


def diagnostic(
    code: str,
    message: str,
    *,
    severity: str = "error",
    row: Mapping[str, Any] | None = None,
    json_path: str = "$",
    path: str | Path | None = None,
) -> dict[str, Any]:
    return {
        "schema_version": VERIFIER_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "severity": severity,
        "diagnostic_code": code,
        "code": code,
        "message": message,
        "failure_reason": message,
        "identity": row.get("identity") if row else None,
        "article_ref": row.get("article_ref") if row else None,
        "source_role": row.get("source_role") if row else None,
        "variant_id": row.get("variant_id") if row else None,
        "json_path": row.get("json_path")
        if row and isinstance(row.get("json_path"), str) and json_path == "$"
        else json_path,
        "safe_path": row.get("safe_path") if row else None,
        "path": path.as_posix() if isinstance(path, Path) else path,
        "network_fetch_attempted": False,
        "graph_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
    }


def row_key(row: Mapping[str, Any]) -> tuple[str, str, str]:
    return (
        str(row.get("identity") or ""),
        str(row.get("source_role") or ""),
        str(row.get("variant_id") or ""),
    )


def index_loader_rows(
    loader_summary: Mapping[str, Any],
) -> dict[tuple[str, str, str], Mapping[str, Any]]:
    rows = loader_summary.get("results")
    if not isinstance(rows, list):
        return {}
    indexed: dict[tuple[str, str, str], Mapping[str, Any]] = {}
    for row in rows:
        if isinstance(row, Mapping):
            indexed[row_key(row)] = row
    return indexed


def validate_no_payload_leakage(value: Any, *, serialized: str, where: str) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []

    def walk(node: Any, path: str) -> None:
        if isinstance(node, dict):
            for key, item in node.items():
                if key in FORBIDDEN_PAYLOAD_KEYS:
                    findings.append(
                        diagnostic(
                            "metadata_payload_key_leakage",
                            f"metadata contains forbidden payload key {key!r}",
                            json_path=f"{path}.{key}",
                            path=where,
                        )
                    )
                walk(item, f"{path}.{key}")
        elif isinstance(node, list):
            for index, item in enumerate(node):
                walk(item, f"{path}[{index}]")

    walk(value, "$")
    lowered = serialized.lower()
    for snippet in sorted(FORBIDDEN_SNIPPETS):
        if snippet.lower() in lowered:
            findings.append(
                diagnostic(
                    "metadata_payload_snippet_leakage",
                    f"metadata contains forbidden raw payload snippet {snippet!r}",
                    path=where,
                )
            )
    return findings


def flag_findings(
    flags: Mapping[str, Any], *, where: str, row: Mapping[str, Any] | None = None
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    for flag in sorted(EXPECTED_FALSE_FLAGS):
        if flags.get(flag) is True:
            findings.append(
                diagnostic(
                    "unsafe_safety_flag_true",
                    f"fail-closed safety flag is true: {flag}",
                    row=row,
                    json_path=f"$.{flag}",
                    path=where,
                )
            )
    return findings


def validate_schema_and_counts(
    conversion_summary: Mapping[str, Any], diagnostics_rows: list[Mapping[str, Any]], report: str
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    rows = conversion_summary.get("results")
    if conversion_summary.get("schema_version") != CONVERSION_SCHEMA_VERSION:
        findings.append(
            diagnostic(
                "unexpected_conversion_schema",
                "conversion summary schema is not the M031 parser conversion schema",
                json_path="$.schema_version",
            )
        )
    if conversion_summary.get("selection_id") != SELECTION_ID:
        findings.append(
            diagnostic(
                "unexpected_selection_id",
                "conversion summary selection_id does not match M031 corpus",
                json_path="$.selection_id",
            )
        )
    if not isinstance(rows, list):
        return findings + [
            diagnostic(
                "malformed_conversion_results",
                "conversion summary results must be a list",
                json_path="$.results",
            )
        ]
    if conversion_summary.get("row_count") != len(rows):
        findings.append(
            diagnostic(
                "row_count_mismatch",
                "conversion summary row_count does not match results length",
                json_path="$.row_count",
            )
        )
    actual_counts = dict(
        sorted(Counter(str(row.get("status")) for row in rows if isinstance(row, Mapping)).items())
    )
    if conversion_summary.get("counts") != actual_counts:
        findings.append(
            diagnostic(
                "status_counts_mismatch",
                "conversion summary counts do not match result statuses",
                json_path="$.counts",
            )
        )
    parser_ready_count = sum(
        1 for row in rows if isinstance(row, Mapping) and row.get("parser_ready") is True
    )
    if conversion_summary.get("parser_ready_count") != parser_ready_count:
        findings.append(
            diagnostic(
                "parser_ready_count_mismatch",
                "parser_ready_count does not match result rows",
                json_path="$.parser_ready_count",
            )
        )
    if len(diagnostics_rows) != len(rows):
        findings.append(
            diagnostic(
                "diagnostic_row_count_mismatch",
                "conversion diagnostics JSONL row count must match conversion results",
                json_path="$",
            )
        )
    for section in REQUIRED_REPORT_SECTIONS:
        if section not in report:
            findings.append(
                diagnostic(
                    "conversion_report_section_missing",
                    f"conversion report missing required section {section}",
                    json_path="$",
                )
            )
    findings.extend(flag_findings(conversion_summary, where="conversion-summary"))
    flags = conversion_summary.get("fail_closed_safety_flags")
    if isinstance(flags, Mapping):
        findings.extend(flag_findings(flags, where="conversion-summary.fail_closed_safety_flags"))
    return findings


def validate_loader_linkage(
    *,
    conversion_rows: list[Mapping[str, Any]],
    loader_summary: Mapping[str, Any],
    loader_summary_path: Path,
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    loader_rows = loader_summary.get("results")
    if not isinstance(loader_rows, list):
        return [
            diagnostic(
                "malformed_loader_results",
                "loader summary results must be a list",
                json_path="$.results",
            )
        ]
    if len(loader_rows) != len(conversion_rows):
        findings.append(
            diagnostic(
                "loader_conversion_row_count_mismatch",
                "loader and conversion row counts differ",
                json_path="$.results",
            )
        )
    indexed = index_loader_rows(loader_summary)
    source_root = loader_summary_path.parent / "source"
    for index, row in enumerate(conversion_rows):
        loader_row = indexed.get(row_key(row))
        if loader_row is None:
            findings.append(
                diagnostic(
                    "missing_loader_linkage",
                    "conversion row has no matching loader evidence row",
                    row=row,
                    json_path=f"$.results[{index}]",
                )
            )
            continue
        if row.get("source_loader_status") != loader_row.get("status"):
            findings.append(
                diagnostic(
                    "loader_status_linkage_mismatch",
                    "conversion row source_loader_status does not match loader row",
                    row=row,
                    json_path=f"$.results[{index}].source_loader_status",
                )
            )
        local_path = loader_row.get("local_path")
        if isinstance(local_path, str) and local_path.strip():
            row_code = str(row.get("diagnostic_code") or row.get("refusal_code") or "")
            expected_path_refusal = row.get("status") == "blocked" and row_code in {
                "unsafe_relative_path",
                "missing_source_artifact",
                "missing_local_source_path",
            }
            try:
                source_path = safe_under_root(source_root, local_path, code_label="source_path")
            except ValueError as exc:
                if not expected_path_refusal:
                    findings.append(
                        diagnostic(
                            str(exc),
                            "loader local_path is unsafe",
                            row=row,
                            json_path=f"$.results[{index}].local_path",
                        )
                    )
            else:
                if source_path.exists() and source_path.is_file():
                    expected_hash = loader_row.get("sha256")
                    expected_size = loader_row.get("byte_size")
                    actual_hash = sha256_file(source_path)
                    actual_size = source_path.stat().st_size
                    if isinstance(expected_hash, str) and expected_hash != actual_hash:
                        findings.append(
                            diagnostic(
                                "source_sha256_mismatch",
                                "source artifact hash no longer matches loader evidence",
                                row=row,
                                json_path=f"$.results[{index}].sha256",
                            )
                        )
                    if isinstance(expected_size, int) and expected_size != actual_size:
                        findings.append(
                            diagnostic(
                                "source_byte_size_mismatch",
                                "source artifact size no longer matches loader evidence",
                                row=row,
                                json_path=f"$.results[{index}].byte_size",
                            )
                        )
                elif (
                    loader_row.get("status") not in {"blocked", "loader_blocked"}
                    and not expected_path_refusal
                ):
                    findings.append(
                        diagnostic(
                            "missing_source_artifact",
                            "loader evidence local_path does not exist",
                            row=row,
                            json_path=f"$.results[{index}].local_path",
                        )
                    )
    return findings


def validate_conversion_row(
    row: Mapping[str, Any], *, index: int, converted_text_dir: Path
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    status = row.get("status")
    role = str(row.get("source_role") or "")
    parser_ready = row.get("parser_ready") is True
    if status not in TERMINAL_STATUSES:
        findings.append(
            diagnostic(
                "unexpected_conversion_status",
                f"unexpected conversion status {status!r}",
                row=row,
                json_path=f"$.results[{index}].status",
            )
        )
    try:
        safe_path = row.get("safe_path")
        if safe_path is not None:
            safe_relative_path(safe_path, code_label="safe_path")
    except ValueError:
        findings.append(
            diagnostic(
                "unsafe_safe_path",
                "conversion safe_path is not confined to the local corpus source namespace",
                row=row,
                json_path=f"$.results[{index}].safe_path",
            )
        )
    row_flags = row.get("fail_closed_safety_flags")
    if isinstance(row_flags, Mapping):
        findings.extend(flag_findings(row_flags, where="row.fail_closed_safety_flags", row=row))
    findings.extend(flag_findings(row, where="row", row=row))
    if parser_ready:
        if status != "converted":
            findings.append(
                diagnostic(
                    "parser_ready_status_mismatch",
                    "parser_ready rows must have converted status",
                    row=row,
                    json_path=f"$.results[{index}].status",
                )
            )
        if role in METADATA_ONLY_ROLES:
            findings.append(
                diagnostic(
                    "metadata_only_parser_ready_claim",
                    "metadata-only source role was promoted to parser-ready",
                    row=row,
                    json_path=f"$.results[{index}].parser_ready",
                )
            )
        if role in HTML_ROLES:
            quality = row.get("quality") if isinstance(row.get("quality"), Mapping) else {}
            bounded = (
                row.get("bounded_extraction")
                if isinstance(row.get("bounded_extraction"), Mapping)
                else {}
            )
            if quality.get("status") in {"low_quality", "empty"} or bounded.get(  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
                "fallback_stub_detected"
            ):
                findings.append(
                    diagnostic(
                        "low_quality_html_parser_ready_claim",
                        "fallback/low-quality HTML was promoted to parser-ready",
                        row=row,
                        json_path=f"$.results[{index}].parser_ready",
                    )
                )
        converted_value = row.get("converted_text_path")
        try:
            converted_path = safe_under_root(
                converted_text_dir, converted_value, code_label="converted_text_path"
            )
        except ValueError as exc:
            findings.append(
                diagnostic(
                    str(exc),
                    "converted_text_path is unsafe",
                    row=row,
                    json_path=f"$.results[{index}].converted_text_path",
                )
            )
        else:
            if not converted_path.exists():
                findings.append(
                    diagnostic(
                        "missing_converted_text_artifact",
                        "parser-ready row points to missing converted text",
                        row=row,
                        json_path=f"$.results[{index}].converted_text_path",
                    )
                )
            elif not converted_path.is_file():
                findings.append(
                    diagnostic(
                        "converted_text_not_file",
                        "converted_text_path is not a file",
                        row=row,
                        json_path=f"$.results[{index}].converted_text_path",
                    )
                )
            else:
                actual_hash = sha256_file(converted_path)
                actual_size = converted_path.stat().st_size
                if row.get("converted_text_sha256") != actual_hash:
                    findings.append(
                        diagnostic(
                            "converted_text_sha256_mismatch",
                            "converted text hash no longer matches conversion summary",
                            row=row,
                            json_path=f"$.results[{index}].converted_text_sha256",
                        )
                    )
                if row.get("converted_text_byte_size") != actual_size:
                    findings.append(
                        diagnostic(
                            "converted_text_byte_size_mismatch",
                            "converted text size no longer matches conversion summary",
                            row=row,
                            json_path=f"$.results[{index}].converted_text_byte_size",
                        )
                    )
    else:
        if (
            row.get("converted_text_path") is not None
            or row.get("converted_text_sha256") is not None
            or row.get("converted_text_byte_size") not in (None, 0)
        ):
            findings.append(
                diagnostic(
                    "non_parser_ready_converted_text_claim",
                    "non-parser-ready row carries converted text provenance",
                    row=row,
                    json_path=f"$.results[{index}].converted_text_path",
                )
            )
        if status != "converted" and not row.get("refusal_code"):
            findings.append(
                diagnostic(
                    "missing_refusal_code",
                    "non-converted row must carry a stable refusal_code",
                    row=row,
                    json_path=f"$.results[{index}].refusal_code",
                )
            )
    return findings


def build_closeout_summary(
    conversion_summary: Mapping[str, Any], findings: list[dict[str, Any]]
) -> dict[str, Any]:
    rows = (
        conversion_summary.get("results")
        if isinstance(conversion_summary.get("results"), list)
        else []
    )
    return {
        "schema_version": VERIFIER_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "passed" if not findings else "failed",
        "failure_count": len(findings),
        # pyrefly: ignore [bad-argument-type]
        "row_count": len(rows),  # ty:ignore[invalid-argument-type]
        "parser_ready_count": sum(
            # pyrefly: ignore [not-iterable]
            1
            for row in rows  # ty:ignore[not-iterable]
            if isinstance(row, Mapping) and row.get("parser_ready") is True  # ty:ignore[not-iterable]
        ),
        "conversion_status_counts": dict(
            sorted(
                # pyrefly: ignore [not-iterable]
                Counter(str(row.get("status")) for row in rows if isinstance(row, Mapping)).items()  # ty:ignore[not-iterable]
            )
        ),
        "diagnostic_code_counts": dict(
            sorted(Counter(str(finding.get("diagnostic_code")) for finding in findings).items())
        ),
        "network_fetch_attempted": False,
        "graph_import_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_write_attempted": False,
        "production_persistence_attempted": False,
        "fail_closed_safety_flags": dict.fromkeys(sorted(EXPECTED_FALSE_FLAGS), False),
    }


def render_report(summary: Mapping[str, Any], findings: list[Mapping[str, Any]]) -> str:
    lines = [
        "# M031 Parser Conversion Closeout Report",
        "",
        "Validate-only cold-reader audit for the M031 parser conversion replay boundary. This report is metadata-only and does not embed source payloads, converted text snippets, PDF bytes, encoded payload material, graph-ready facts, or LadybugDB readiness claims.",
        "",
        f"- Status: `{summary.get('status')}`",
        f"- Failure count: {summary.get('failure_count')}",
        f"- Row count: {summary.get('row_count')}",
        f"- Parser-ready rows: {summary.get('parser_ready_count')}",
        "- Network fetch attempted: `False`",
        "- Graph/import/LadybugDB writes: `False`",
        "",
        "## Failure Modes",
        "",
        "Malformed or missing JSON artifacts, stale loader/conversion linkage, source hash drift, missing converted text, unsafe paths, report drift, redaction leaks, and permissive graph/import flags become non-zero verifier diagnostics.",
        "",
        "## Load Profile",
        "",
        "Expected load is seven conversion rows and one converted text file. At 10x, local file hashing saturates first; hashing is streamed in 1 MiB chunks and there is no network, subprocess, graph import, or LadybugDB write path.",
        "",
        "## Negative Tests",
        "",
        "Covered by `tests/test_m031_parser_conversion_replay.py`: mutated converted hash, deleted converted text, unsafe safe_path, raw payload marker leakage, parser-ready promotion for fallback HTML or metadata-only abs pages, and unsafe graph/import/write flags.",
        "",
        "## Findings",
        "",
    ]
    if not findings:
        lines.append("- None.")
    else:
        for finding in findings:
            lines.append(
                f"- `{finding.get('diagnostic_code')}` severity=`{finding.get('severity')}` identity=`{finding.get('identity') or '<none>'}` "
                f"article_ref=`{finding.get('article_ref') or '<none>'}` source_role=`{finding.get('source_role') or '<none>'}` json_path=`{finding.get('json_path')}`"
            )
    return "\n".join(lines) + "\n"


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--loader-summary", required=True, type=Path)
    parser.add_argument("--conversion-summary", required=True, type=Path)
    parser.add_argument("--conversion-diagnostics", required=True, type=Path)
    parser.add_argument("--conversion-report", required=True, type=Path)
    parser.add_argument("--converted-text-dir", required=True, type=Path)
    parser.add_argument("--write-summary", type=Path)
    parser.add_argument("--write-diagnostics", type=Path)
    parser.add_argument("--write-report", type=Path)
    return parser.parse_args(argv)


def verify(
    argv: list[str] | argparse.Namespace | None = None,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    args = parse_args(argv) if not isinstance(argv, argparse.Namespace) else argv
    findings: list[dict[str, Any]] = []
    selection = load_json(args.selection)
    loader_summary = load_json(args.loader_summary)
    conversion_summary = load_json(args.conversion_summary)
    diagnostics_rows = load_jsonl(args.conversion_diagnostics)
    report = args.conversion_report.read_text(encoding="utf-8")
    if selection.get("selection_id") != loader_summary.get("selection_id") or selection.get(
        "selection_id"
    ) != conversion_summary.get("selection_id"):
        findings.append(
            diagnostic(
                "selection_artifact_mismatch",
                "selection_id does not match across input artifacts",
                json_path="$.selection_id",
            )
        )
    # pyrefly: ignore [bad-argument-type]
    findings.extend(validate_schema_and_counts(conversion_summary, diagnostics_rows, report))  # ty:ignore[invalid-argument-type]
    rows = (
        conversion_summary.get("results")
        if isinstance(conversion_summary.get("results"), list)
        else []
    )
    conversion_rows = [row for row in rows if isinstance(row, Mapping)]  # ty:ignore[not-iterable]
    findings.extend(
        validate_loader_linkage(
            conversion_rows=conversion_rows,
            loader_summary=loader_summary,
            loader_summary_path=args.loader_summary,
        )
    )
    converted_text_dir = args.converted_text_dir.resolve()
    for index, row in enumerate(conversion_rows):
        findings.extend(
            validate_conversion_row(row, index=index, converted_text_dir=converted_text_dir)
        )
    findings.extend(
        validate_no_payload_leakage(
            conversion_summary,
            serialized=json.dumps(conversion_summary, sort_keys=True),
            where=args.conversion_summary.as_posix(),
        )
    )
    findings.extend(
        validate_no_payload_leakage(
            diagnostics_rows,
            serialized=json.dumps(diagnostics_rows, sort_keys=True),
            where=args.conversion_diagnostics.as_posix(),
        )
    )
    findings.extend(
        validate_no_payload_leakage({}, serialized=report, where=args.conversion_report.as_posix())
    )
    closeout_summary = build_closeout_summary(conversion_summary, findings)
    # pyrefly: ignore [bad-argument-type]
    closeout_report = render_report(closeout_summary, findings)  # ty:ignore[invalid-argument-type]
    if args.write_summary:
        write_json(args.write_summary, closeout_summary)
    if args.write_diagnostics:
        # pyrefly: ignore [bad-argument-type]
        write_jsonl(args.write_diagnostics, findings)  # ty:ignore[invalid-argument-type]
    if args.write_report:
        atomic_write_text(args.write_report, closeout_report)
    return closeout_summary, findings


def main(argv: list[str] | None = None) -> int:
    try:
        summary, findings = verify(argv)
    except Exception as exc:  # validate-only setup failure is surfaced as a stable CLI failure.
        sys.stderr.write(
            json.dumps(
                {"status": "failed", "code": "verifier_setup_failed", "message": str(exc)},
                sort_keys=True,
            )
            + "\n"
        )
        return 2
    sys.stdout.write(
        json.dumps({"status": summary["status"], "failure_count": len(findings)}, sort_keys=True)
        + "\n"
    )
    return 0 if not findings else 1


if __name__ == "__main__":
    raise SystemExit(main())
