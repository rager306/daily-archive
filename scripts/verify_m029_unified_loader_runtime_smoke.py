#!/usr/bin/env python3
"""Verifier for the M029 unified loader runtime smoke artifacts."""

from __future__ import annotations

import argparse
import json
import re
import sys
import tempfile
from collections import Counter, defaultdict
from collections.abc import Iterable, Mapping, Sequence
from pathlib import Path, PurePosixPath
from typing import Any

MILESTONE_ID = "M029-eb0ljz"
SLICE_ID = "S04"
SELECTION_ID = "m029-unified-corpus-v1"
SCHEMA_VERSION = "m029-runtime-smoke-verifier.v1"
ROOT = Path(__file__).resolve().parents[1]
UNSAFE_TRUE_FLAGS = {
    "graph_import_allowed",
    "production_ladybugdb_write_allowed",
    "trusted_kg_import_allowed",
    "production_import_attempted",
    "ladybugdb_written",
    "network_fetch_attempted",
    "raw_text_embedded_in_metadata",
    "raw_binary_embedded_in_metadata",
    "raw_text_embedded",
    "raw_binary_embedded",
    "raw_payload_embedded_in_metadata",
}
FORBIDDEN_REPORT_SNIPPETS = {"<html", "</html", "%PDF-", "base64,", "RAW_ARXIV", "RAW_PDF"}
FORBIDDEN_EVIDENCE_SNIPPETS = FORBIDDEN_REPORT_SNIPPETS | {
    "raw_text",
    "raw_binary",
    "text_body",
    "html_body",
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
    atomic_write_text(path, json.dumps(payload, indent=2, sort_keys=True) + "\n")


def write_jsonl(path: Path, rows: Iterable[Mapping[str, Any]]) -> None:
    atomic_write_text(path, "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows))


def rel(path: Path, root: Path = ROOT) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return str(path)


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


def safe_under_root(root: Path, value: Any, *, code_label: str) -> Path:
    normalized = safe_relative_path(value, code_label=code_label)
    root_resolved = root.resolve()
    resolved = (root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root_resolved):
        raise ValueError(f"{code_label}_escapes_root")
    return resolved


def diagnostic(
    code: str,
    message: str,
    *,
    path: Path | str | None = None,
    json_path: str = "$",
    article_ref: str | None = None,
) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "severity": "error",
        "diagnostic_code": code,
        "code": code,
        "message": message,
        "failure_reason": message,
        "path": rel(path) if isinstance(path, Path) else path,
        "json_path": json_path,
        "article_ref": article_ref,
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_import_allowed": False,
    }


def article_key_for(row: Mapping[str, Any]) -> str:
    value = row.get("article_ref") or row.get("identity_key")
    if not isinstance(value, str) or not value:
        raise ValueError("missing_article_ref_or_identity_key")
    return value


def selection_by_article(selection: Mapping[str, Any]) -> dict[str, dict[str, Any]]:
    articles = selection.get("articles")
    if not isinstance(articles, list):
        raise ValueError("selection articles must be a list")
    by_article: dict[str, dict[str, Any]] = {}
    for index, article in enumerate(articles):
        if not isinstance(article, dict):
            raise ValueError(f"selection article at index {index} is not an object")
        article_ref = article_key_for(article)
        by_article[article_ref] = dict(article)
    return by_article


def conversion_by_article(conversion_summary: Mapping[str, Any]) -> dict[str, list[dict[str, Any]]]:
    rows = conversion_summary.get("results")
    if not isinstance(rows, list):
        raise ValueError("conversion summary results must be a list")
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for index, row in enumerate(rows):
        if not isinstance(row, dict):
            raise ValueError(f"conversion row at index {index} is not an object")
        article_ref = article_key_for(row)
        grouped[article_ref].append(dict(row))
    return dict(grouped)


def row_unsafe_flags(row: Mapping[str, Any]) -> list[str]:
    found: list[str] = []
    for key in UNSAFE_TRUE_FLAGS:
        if row.get(key) is True:
            found.append(key)
    nested = row.get("fail_closed_safety_flags")
    if isinstance(nested, Mapping):
        for key in UNSAFE_TRUE_FLAGS:
            if nested.get(key) is True:
                found.append(f"fail_closed_safety_flags.{key}")
    return sorted(set(found))


def parser_ready_article_refs(conversion_summary: Mapping[str, Any]) -> set[str]:
    refs: set[str] = set()
    for article_ref, rows in conversion_by_article(conversion_summary).items():
        if any(row.get("parser_ready") is True and row.get("converted_text_path") for row in rows):
            refs.add(article_ref)
    return refs


def check_summary_shape(
    summary: Mapping[str, Any], diagnostics: Sequence[Mapping[str, Any]], path: Path
) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    if summary.get("schema_version") != "m029-runtime-smoke.v1":
        problems.append(
            diagnostic(
                "invalid_schema_version",
                "runtime summary schema version is invalid",
                path=path,
                json_path="$.schema_version",
            )
        )
    if summary.get("milestone_id") != MILESTONE_ID or summary.get("slice_id") != SLICE_ID:
        problems.append(
            diagnostic(
                "invalid_milestone_or_slice",
                "runtime summary milestone/slice identifiers are invalid",
                path=path,
            )
        )
    results = summary.get("results")
    if not isinstance(results, list):
        problems.append(
            diagnostic(
                "missing_summary_results",
                "runtime summary results must be a list",
                path=path,
                json_path="$.results",
            )
        )
        return problems
    if len(results) != len(diagnostics):
        problems.append(
            diagnostic(
                "summary_diagnostics_count_mismatch",
                "summary results and diagnostics row counts differ",
                path=path,
            )
        )
    if summary.get("article_count") != len(results):
        problems.append(
            diagnostic(
                "article_count_mismatch",
                "summary article_count does not match results length",
                path=path,
                json_path="$.article_count",
            )
        )
    status_counts = Counter(
        str(row.get("status", "unknown")) for row in results if isinstance(row, Mapping)
    )
    if summary.get("runtime_loaded_count") != status_counts.get("loaded", 0):
        problems.append(
            diagnostic(
                "runtime_loaded_count_mismatch",
                "runtime_loaded_count does not match result statuses",
                path=path,
            )
        )
    if summary.get("zero_chunk_count") != status_counts.get("zero_chunk", 0):
        problems.append(
            diagnostic(
                "zero_chunk_count_mismatch",
                "zero_chunk_count does not match result statuses",
                path=path,
            )
        )
    evidence_count = sum(
        int(row.get("runtime_evidence_count", 0)) for row in results if isinstance(row, Mapping)
    )
    if summary.get("runtime_evidence_count") != evidence_count:
        problems.append(
            diagnostic(
                "runtime_evidence_count_mismatch",
                "runtime_evidence_count does not match rows",
                path=path,
            )
        )
    return problems


def check_fail_closed(
    summary: Mapping[str, Any], rows: Sequence[Mapping[str, Any]], path: Path
) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    for flag in row_unsafe_flags(summary):
        problems.append(
            diagnostic(
                "unsafe_summary_flag_true", f"unsafe summary flag is true: {flag}", path=path
            )
        )
    for index, row in enumerate(rows):
        for flag in row_unsafe_flags(row):
            problems.append(
                diagnostic(
                    "unsafe_runtime_flag_true",
                    f"unsafe runtime row flag is true: {flag}",
                    path=path,
                    json_path=f"$.results[{index}]",
                    article_ref=str(row.get("article_ref")),
                )
            )
    return problems


def check_selection_alignment(
    *,
    selection: Mapping[str, Any],
    rows: Sequence[Mapping[str, Any]],
    check_selection_count: int | None,
    check_article_identity: bool,
    check_source_strategy_mapping: bool,
    path: Path,
) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    selected = selection_by_article(selection)
    row_by_article = {article_key_for(row): row for row in rows}
    if check_selection_count is not None and len(selected) != check_selection_count:
        problems.append(
            diagnostic(
                "selection_count_mismatch",
                f"selection count {len(selected)} != expected {check_selection_count}",
                path=path,
            )
        )
    if set(row_by_article) != set(selected):
        missing = sorted(set(selected) - set(row_by_article))
        extra = sorted(set(row_by_article) - set(selected))
        problems.append(
            diagnostic(
                "runtime_selection_article_mismatch",
                f"runtime rows do not match selection articles; missing={missing} extra={extra}",
                path=path,
            )
        )
    for article_ref, article in selected.items():
        row = row_by_article.get(article_ref)
        if row is None:
            continue
        if check_article_identity:
            for key in ["article_key", "identity_key"]:
                if article.get(key) != row.get(key):
                    problems.append(
                        diagnostic(
                            "article_identity_mismatch",
                            f"{key} mismatch for {article_ref}",
                            path=path,
                            article_ref=article_ref,
                        )
                    )
            expected_url = article.get("canonical_url") or article.get("seed_url")
            if expected_url != row.get("canonical_url"):
                problems.append(
                    diagnostic(
                        "canonical_url_mismatch",
                        f"canonical_url mismatch for {article_ref}",
                        path=path,
                        article_ref=article_ref,
                    )
                )
        if check_source_strategy_mapping and article.get("source_strategy") != row.get(
            "source_strategy"
        ):
            problems.append(
                diagnostic(
                    "source_strategy_mismatch",
                    f"source_strategy mismatch for {article_ref}",
                    path=path,
                    article_ref=article_ref,
                )
            )
    return problems


def check_parser_alignment(
    *,
    conversion_summary: Mapping[str, Any],
    rows: Sequence[Mapping[str, Any]],
    path: Path,
) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    parser_ready_refs = parser_ready_article_refs(conversion_summary)
    for row in rows:
        article_ref = article_key_for(row)
        if article_ref in parser_ready_refs:
            if row.get("status") != "loaded" or int(row.get("runtime_evidence_count", 0)) < 1:
                problems.append(
                    diagnostic(
                        "parser_ready_article_not_loaded",
                        f"parser-ready article was not runtime-loaded: {article_ref}",
                        path=path,
                        article_ref=article_ref,
                    )
                )
            if row.get("parser_ready_from_conversion") is not True:
                problems.append(
                    diagnostic(
                        "parser_ready_flag_not_preserved",
                        f"parser_ready flag not preserved for {article_ref}",
                        path=path,
                        article_ref=article_ref,
                    )
                )
        elif row.get("status") != "zero_chunk":
            problems.append(
                diagnostic(
                    "non_parser_ready_article_loaded",
                    f"non-parser-ready article unexpectedly loaded: {article_ref}",
                    path=path,
                    article_ref=article_ref,
                )
            )
    return problems


def check_artifact_paths(
    summary: Mapping[str, Any],
    rows: Sequence[Mapping[str, Any]],
    corpus_dir: Path,
    artifact_root: Path,
    path: Path,
) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    event_dir_raw = summary.get("runtime_event_dir")
    try:
        event_dir = safe_under_root(artifact_root, event_dir_raw, code_label="runtime_event_dir")
        if not event_dir.is_relative_to(corpus_dir.resolve()):
            raise ValueError("runtime_event_dir_outside_corpus")
    except ValueError as exc:
        problems.append(
            diagnostic(
                "unsafe_runtime_event_dir", str(exc), path=path, json_path="$.runtime_event_dir"
            )
        )
    for index, row in enumerate(rows):
        if row.get("status") == "loaded":
            for field in ["converted_text_path", "runtime_event_log_path"]:
                try:
                    candidate = safe_under_root(artifact_root, row.get(field), code_label=field)
                    if not candidate.is_relative_to(corpus_dir.resolve()):
                        raise ValueError(f"{field}_outside_corpus")
                except ValueError as exc:
                    problems.append(
                        diagnostic(
                            f"unsafe_{field}",
                            str(exc),
                            path=path,
                            json_path=f"$.results[{index}].{field}",
                            article_ref=str(row.get("article_ref")),
                        )
                    )
                    continue
                if not candidate.exists() or not candidate.is_file():
                    problems.append(
                        diagnostic(
                            f"missing_{field}",
                            f"{field} does not exist",
                            path=candidate,
                            json_path=f"$.results[{index}].{field}",
                            article_ref=str(row.get("article_ref")),
                        )
                    )
    return problems


def check_report(report_path: Path) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    text = report_path.read_text(encoding="utf-8")
    required = [
        "# M029 Unified Loader Runtime Smoke",
        "## Article Outcomes",
        "## Fail-Closed Boundaries",
    ]
    for marker in required:
        if marker not in text:
            problems.append(
                diagnostic(
                    "runtime_report_missing_section",
                    f"report missing section marker: {marker}",
                    path=report_path,
                )
            )
    lowered = text.lower()
    for snippet in FORBIDDEN_REPORT_SNIPPETS:
        if snippet.lower() in lowered:
            problems.append(
                diagnostic(
                    "runtime_report_embeds_raw_payload",
                    f"report contains forbidden raw payload marker: {snippet}",
                    path=report_path,
                )
            )
    return problems


def slug(value: str) -> str:
    safe = re.sub(r"[^A-Za-z0-9._-]+", "_", value).strip("_")
    return safe or "article"


def evidence_filename(row: Mapping[str, Any]) -> str:
    return f"{slug(article_key_for(row))}.evidence.json"


def evidence_record(row: Mapping[str, Any], *, evidence_dir: Path) -> dict[str, Any]:
    return {
        "schema_version": "m029-loader-evidence.v1",
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "article_ref": row.get("article_ref"),
        "article_key": row.get("article_key"),
        "identity_key": row.get("identity_key"),
        "canonical_url": row.get("canonical_url"),
        "seed_url": row.get("seed_url"),
        "source_code": row.get("source_code"),
        "source_strategy": row.get("source_strategy"),
        "selected_variant_id": row.get("selected_variant_id"),
        "selected_source_role": row.get("selected_source_role"),
        "conversion_status": row.get("conversion_status"),
        "conversion_diagnostic_code": row.get("conversion_diagnostic_code"),
        "parser_ready_from_conversion": row.get("parser_ready_from_conversion") is True,
        "status": row.get("status"),
        "diagnostic_code": row.get("diagnostic_code"),
        "failure_reason": row.get("failure_reason"),
        "runtime_loader_outcome": row.get("runtime_loader_outcome"),
        "runtime_loader_failure_reason": row.get("runtime_loader_failure_reason"),
        "runtime_loader_warning_count": row.get("runtime_loader_warning_count", 0),
        "runtime_loader_warnings": list(row.get("runtime_loader_warnings", []))
        if isinstance(row.get("runtime_loader_warnings"), list)
        else [],
        "runtime_loader_name": row.get("runtime_loader_name"),
        "runtime_parser_name": row.get("runtime_parser_name"),
        "runtime_event_log_path": row.get("runtime_event_log_path"),
        "converted_text_path": row.get("converted_text_path"),
        "converted_text_sha256": row.get("converted_text_sha256"),
        "converted_text_byte_size": row.get("converted_text_byte_size", 0),
        "runtime_evidence_count": int(row.get("runtime_evidence_count", 0)),
        "runtime_chunk_count": int(row.get("runtime_chunk_count", 0)),
        "zero_chunk": row.get("zero_chunk") is True,
        "evidence_path": rel(evidence_dir / evidence_filename(row)),
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "graph_import_allowed": False,
        "fail_closed_safety_flags": {
            "network_fetch_attempted": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
            "trusted_kg_import_allowed": False,
            "graph_import_allowed": False,
            "raw_text_embedded_in_metadata": False,
            "raw_binary_embedded_in_metadata": False,
        },
    }


def evidence_diagnostic_row(record: Mapping[str, Any]) -> dict[str, Any]:
    code = str(record.get("diagnostic_code") or "runtime_loader_unknown")
    return {
        "schema_version": "m029-loader-evidence-diagnostic.v1",
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "severity": "info" if record.get("status") == "loaded" else "warning",
        "article_ref": record.get("article_ref"),
        "identity_key": record.get("identity_key"),
        "canonical_url": record.get("canonical_url"),
        "source_strategy": record.get("source_strategy"),
        "diagnostic_code": code,
        "code": code,
        "failure_reason": record.get("failure_reason"),
        "runtime_evidence_count": record.get("runtime_evidence_count", 0),
        "runtime_chunk_count": record.get("runtime_chunk_count", 0),
        "zero_chunk": record.get("zero_chunk") is True,
        "evidence_path": record.get("evidence_path"),
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_import_allowed": False,
    }


def build_evidence_bundle(
    *,
    summary: Mapping[str, Any],
    rows: Sequence[Mapping[str, Any]],
    evidence_dir: Path,
    evidence_summary_path: Path,
    evidence_diagnostics_path: Path,
) -> tuple[list[dict[str, Any]], dict[str, Any], list[dict[str, Any]]]:
    records = [evidence_record(row, evidence_dir=evidence_dir) for row in rows]
    diagnostics = [evidence_diagnostic_row(record) for record in records]
    status_counts = Counter(str(record.get("status", "unknown")) for record in records)
    diagnostic_counts = Counter(str(record.get("diagnostic_code", "unknown")) for record in records)
    failure_counts = Counter(str(record.get("failure_reason") or "none") for record in records)
    evidence_summary = {
        "schema_version": "m029-loader-evidence-summary.v1",
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "runtime_smoke_summary_path": rel(Path(str(summary.get("runtime_summary_path", ""))))
        if summary.get("runtime_summary_path")
        else None,
        "article_count": len(records),
        "evidence_record_count": len(records),
        "runtime_loaded_count": status_counts.get("loaded", 0),
        "zero_chunk_count": status_counts.get("zero_chunk", 0),
        "runtime_evidence_count": sum(
            int(record.get("runtime_evidence_count", 0)) for record in records
        ),
        "runtime_chunk_count": sum(int(record.get("runtime_chunk_count", 0)) for record in records),
        "status_counts": dict(sorted(status_counts.items())),
        "diagnostic_code_counts": dict(sorted(diagnostic_counts.items())),
        "failure_reason_counts": dict(sorted(failure_counts.items())),
        "evidence_dir": rel(evidence_dir),
        "evidence_summary_path": rel(evidence_summary_path),
        "evidence_diagnostics_path": rel(evidence_diagnostics_path),
        "evidence_paths": [record["evidence_path"] for record in records],
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "graph_import_allowed": False,
        "raw_text_embedded_in_metadata": False,
        "raw_binary_embedded_in_metadata": False,
    }
    return records, evidence_summary, diagnostics


def check_evidence_bundle(
    *,
    runtime_summary: Mapping[str, Any],
    records: Sequence[Mapping[str, Any]],
    evidence_summary: Mapping[str, Any],
    diagnostics: Sequence[Mapping[str, Any]],
    path: Path,
    check_evidence_counts: bool,
    check_zero_chunk_outcomes: bool,
) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    runtime_rows = (
        runtime_summary.get("results") if isinstance(runtime_summary.get("results"), list) else []
    )
    if check_evidence_counts:
        if len(records) != len(runtime_rows):
            problems.append(
                diagnostic(
                    "evidence_record_count_mismatch",
                    "evidence record count does not match runtime rows",
                    path=path,
                )
            )
        runtime_evidence_count = sum(
            int(row.get("runtime_evidence_count", 0))
            for row in runtime_rows
            if isinstance(row, Mapping)
        )
        if evidence_summary.get("runtime_evidence_count") != runtime_evidence_count:
            problems.append(
                diagnostic(
                    "evidence_runtime_count_mismatch",
                    "evidence summary runtime_evidence_count does not match runtime rows",
                    path=path,
                )
            )
        if len(diagnostics) != len(records):
            problems.append(
                diagnostic(
                    "evidence_diagnostic_count_mismatch",
                    "evidence diagnostics must contain one row per evidence record",
                    path=path,
                )
            )
    if check_zero_chunk_outcomes:
        for record in records:
            if record.get("zero_chunk") is True:
                if (
                    int(record.get("runtime_evidence_count", -1)) != 0
                    or int(record.get("runtime_chunk_count", -1)) != 0
                ):
                    problems.append(
                        diagnostic(
                            "zero_chunk_has_runtime_evidence",
                            "zero-chunk evidence has non-zero counters",
                            path=path,
                            article_ref=str(record.get("article_ref")),
                        )
                    )
                if not record.get("failure_reason") or not record.get("diagnostic_code"):
                    problems.append(
                        diagnostic(
                            "zero_chunk_missing_failure_diagnostic",
                            "zero-chunk evidence must preserve failure reason and diagnostic code",
                            path=path,
                            article_ref=str(record.get("article_ref")),
                        )
                    )
    serialized = json.dumps(
        {"records": records, "summary": evidence_summary, "diagnostics": diagnostics},
        sort_keys=True,
    ).lower()
    for snippet in FORBIDDEN_EVIDENCE_SNIPPETS:
        if snippet.lower() in serialized and snippet.lower() not in {"raw_text", "raw_binary"}:
            problems.append(
                diagnostic(
                    "evidence_embeds_forbidden_payload_marker",
                    f"evidence contains forbidden marker: {snippet}",
                    path=path,
                )
            )
    for record in records:
        for flag in row_unsafe_flags(record):
            problems.append(
                diagnostic(
                    "unsafe_evidence_flag_true",
                    f"unsafe evidence flag is true: {flag}",
                    path=path,
                    article_ref=str(record.get("article_ref")),
                )
            )
    return problems


def write_evidence_bundle(
    records: Sequence[Mapping[str, Any]],
    summary: Mapping[str, Any],
    diagnostics: Sequence[Mapping[str, Any]],
    *,
    evidence_dir: Path,
    evidence_summary_path: Path,
    evidence_diagnostics_path: Path,
) -> None:
    evidence_dir.mkdir(parents=True, exist_ok=True)
    for stale in evidence_dir.glob("*.evidence.json"):
        stale.unlink()
    for record in records:
        write_json(evidence_dir / evidence_filename(record), record)
    write_json(evidence_summary_path, summary)
    write_jsonl(evidence_diagnostics_path, diagnostics)


def verify(args: argparse.Namespace) -> list[dict[str, Any]]:
    selection_path = Path(args.selection)
    conversion_summary_path = Path(args.conversion_summary)
    summary_path = Path(args.runtime_smoke_summary)
    diagnostics_path = Path(args.runtime_smoke_diagnostics)
    report_path = Path(args.runtime_smoke_report)
    corpus_dir = summary_path.parent
    artifact_root = corpus_dir.parents[2] if len(corpus_dir.parents) >= 3 else ROOT
    selection = load_json(selection_path)
    conversion_summary = load_json(conversion_summary_path)
    summary = load_json(summary_path)
    diagnostics_rows = load_jsonl(diagnostics_path)
    rows = summary.get("results") if isinstance(summary.get("results"), list) else []
    row_mappings = [row for row in rows if isinstance(row, Mapping)]
    problems: list[dict[str, Any]] = []
    problems.extend(check_summary_shape(summary, diagnostics_rows, summary_path))
    problems.extend(check_fail_closed(summary, row_mappings, summary_path))
    problems.extend(
        check_selection_alignment(
            selection=selection,
            rows=row_mappings,
            check_selection_count=args.check_selection_count,
            check_article_identity=args.check_article_identity,
            check_source_strategy_mapping=args.check_source_strategy_mapping,
            path=summary_path,
        )
    )
    if args.check_parser_ready_alignment:
        problems.extend(
            check_parser_alignment(
                conversion_summary=conversion_summary, rows=row_mappings, path=summary_path
            )
        )
    problems.extend(
        check_artifact_paths(summary, row_mappings, corpus_dir, artifact_root, summary_path)
    )
    problems.extend(check_report(report_path))
    if args.require_no_network and summary.get("network_fetch_attempted") is not False:
        problems.append(
            diagnostic(
                "network_fetch_attempted",
                "runtime smoke summary indicates network fetch",
                path=summary_path,
            )
        )
    if args.require_no_import_flags:
        for key in [
            "production_import_attempted",
            "ladybugdb_written",
            "graph_import_allowed",
            "trusted_kg_import_allowed",
        ]:
            if summary.get(key) is not False:
                problems.append(
                    diagnostic(
                        "import_flag_not_fail_closed",
                        f"{key} is not false",
                        path=summary_path,
                        json_path=f"$.{key}",
                    )
                )
    if args.evidence_dir or args.write_evidence_summary or args.write_evidence_diagnostics:
        if (
            not args.evidence_dir
            or not args.write_evidence_summary
            or not args.write_evidence_diagnostics
        ):
            problems.append(
                diagnostic(
                    "incomplete_evidence_output_args",
                    "evidence-dir, write-evidence-summary, and write-evidence-diagnostics must be provided together",
                    path=summary_path,
                )
            )
        else:
            evidence_dir = Path(args.evidence_dir)
            evidence_summary_path = Path(args.write_evidence_summary)
            evidence_diagnostics_path = Path(args.write_evidence_diagnostics)
            records, evidence_summary, evidence_diagnostics = build_evidence_bundle(
                summary=summary,
                rows=row_mappings,
                evidence_dir=evidence_dir,
                evidence_summary_path=evidence_summary_path,
                evidence_diagnostics_path=evidence_diagnostics_path,
            )
            problems.extend(
                check_evidence_bundle(
                    runtime_summary=summary,
                    records=records,
                    evidence_summary=evidence_summary,
                    diagnostics=evidence_diagnostics,
                    path=evidence_summary_path,
                    check_evidence_counts=args.check_evidence_counts,
                    check_zero_chunk_outcomes=args.check_zero_chunk_outcomes,
                )
            )
            if not problems:
                write_evidence_bundle(
                    records,
                    evidence_summary,
                    evidence_diagnostics,
                    evidence_dir=evidence_dir,
                    evidence_summary_path=evidence_summary_path,
                    evidence_diagnostics_path=evidence_diagnostics_path,
                )
    return problems


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True)
    parser.add_argument("--conversion-summary", required=True)
    parser.add_argument("--runtime-smoke-summary", required=True)
    parser.add_argument("--runtime-smoke-diagnostics", required=False)
    parser.add_argument("--runtime-smoke-report", required=False)
    parser.add_argument("--evidence-dir", required=False)
    parser.add_argument("--write-evidence-summary", required=False)
    parser.add_argument("--write-evidence-diagnostics", required=False)
    parser.add_argument("--check-evidence-counts", action="store_true")
    parser.add_argument("--check-zero-chunk-outcomes", action="store_true")
    parser.add_argument("--require-no-network", action="store_true")
    parser.add_argument("--require-no-import-flags", action="store_true")
    parser.add_argument("--check-selection-count", type=int, default=None)
    parser.add_argument("--check-article-identity", action="store_true")
    parser.add_argument("--check-source-strategy-mapping", action="store_true")
    parser.add_argument("--check-parser-ready-alignment", action="store_true")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    parsed = parser.parse_args(argv[1:] if argv else None)
    if parsed.runtime_smoke_diagnostics is None:
        parsed.runtime_smoke_diagnostics = str(
            Path(parsed.runtime_smoke_summary).with_name("runtime-smoke-diagnostics.jsonl")
        )
    if parsed.runtime_smoke_report is None:
        parsed.runtime_smoke_report = str(
            Path(parsed.runtime_smoke_summary).with_name("runtime-smoke-report.md")
        )
    try:
        problems = verify(parsed)
    except Exception as exc:
        sys.stderr.write(f"runtime smoke verification crashed: {type(exc).__name__}: {exc}\n")
        return 1
    if problems:
        for problem in problems:
            sys.stderr.write(json.dumps(problem, sort_keys=True) + "\n")
        return 1
    sys.stdout.write(
        json.dumps(
            {
                "status": "passed",
                "selection": parsed.selection,
                "runtime_smoke_summary": parsed.runtime_smoke_summary,
                "runtime_smoke_diagnostics": parsed.runtime_smoke_diagnostics,
                "runtime_smoke_report": parsed.runtime_smoke_report,
                "evidence_dir": parsed.evidence_dir,
                "evidence_summary": parsed.write_evidence_summary,
                "evidence_diagnostics": parsed.write_evidence_diagnostics,
            },
            sort_keys=True,
        )
        + "\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
