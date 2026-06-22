#!/usr/bin/env python3
"""Validate-only closeout verifier for M031 S04 chunk evidence replay artifacts.

This verifier consumes already-materialized S03/S04 artifacts and independently
checks that S04 is safe for downstream S05 review consumption. It does not rerun
conversion, chunking, review generation, network fetches, graph imports, or
LadybugDB writes. Every invalid artifact is reported as a stable diagnostic with
severity and JSON path; any finding makes the CLI exit non-zero.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections import Counter
from collections.abc import Iterable, Mapping
from pathlib import Path, PurePosixPath
from typing import Any

from research_graph.graph.readiness.review import validate_review_artifacts
from research_graph.repair.chunk_import_contract import (
    validate_import_ready_package,
    validation_to_dict,
)

MILESTONE_ID = "M031-vwpd8e"
SLICE_ID = "S04"
SOURCE_SLICE_ID = "S03"
SELECTION_ID = "m031-catalog-backed-replay-v1"
S03_CLOSEOUT_SCHEMA_VERSION = "m031-parser-conversion-closeout-verifier.v1"
CONVERSION_SCHEMA_VERSION = "m031-parser-conversion-replay.v1"
CHUNK_SUMMARY_SCHEMA_VERSION = "m031-chunk-evidence-replay.v1"
CHUNK_DIAGNOSTIC_SCHEMA_VERSION = "m031-chunk-evidence-diagnostic.v1"
VERIFIER_SCHEMA_VERSION = "m031-chunk-evidence-closeout-verifier.v1"
GRAPH_PACKAGE_SCHEMA_VERSION = "m031-graph-readiness-review-package.v1"
STRUCTURE_PACKAGE_SCHEMA_VERSION = "m005-import-ready-chunk-package.v1"
DEFAULT_CORPUS_DIR = Path("data/article_corpora/m031-catalog-backed-replay-v1")

EXPECTED_FALSE_FLAGS = {
    "network_fetch_attempted",
    "graph_import_allowed",
    "trusted_kg_import_allowed",
    "production_import_attempted",
    "ladybugdb_written",
    "graph_write_attempted",
    "production_persistence_attempted",
    "production_ladybugdb_write_allowed",
    "kg_readiness_claimed",
    "chunk_ready_claimed_for_non_parser_ready_rows",
    "raw_text_included",
    "chunk_text_included",
    "embeddings_included",
    "vectors_included",
    "raw_payload_embedded_in_metadata",
}
FORBIDDEN_PAYLOAD_KEYS = {
    "text",
    "raw_text",
    "chunk_text",
    "paper_text",
    "claim_text",
    "html",
    "raw_html",
    "pdf_bytes",
    "binary_payload",
    "base64_payload",
    "embedding",
    "embeddings",
    "vector",
    "vectors",
    "converted_text",
}
FORBIDDEN_SNIPPETS = {
    "deterministic fallback capture",
    "This fixture PDF contains enough local scientific prose",
    "No network fetches or graph writes should be needed",
    "Local Parser Ready Paper",
    "%PDF-",
    "<html",
    "</html",
    "base64,",
}
REQUIRED_REPORT_SECTIONS = (
    "## Failure Modes",
    "## Load Profile",
    "## Negative Tests",
    "## Graph-Readiness Review Handoff",
)


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


def write_jsonl(path: Path, rows: Iterable[Mapping[str, Any]]) -> None:
    atomic_write_text(path, "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows))


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


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
        "package_key": row.get("package_key") if row else None,
        "json_path": row.get("json_path")
        if row and isinstance(row.get("json_path"), str) and json_path == "$"
        else json_path,
        "path": path.as_posix() if isinstance(path, Path) else path,
        "network_fetch_attempted": False,
        "graph_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
    }


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


def resolve_artifact_path(project_root: Path, value: Any, *, code_label: str) -> Path:
    return safe_under_root(project_root, value, code_label=code_label)


def row_key(row: Mapping[str, Any]) -> tuple[str, str, str]:
    return (
        str(row.get("identity") or ""),
        str(row.get("source_role") or ""),
        str(row.get("variant_id") or ""),
    )


def package_key(row: Mapping[str, Any]) -> str:
    return f"{str(row.get('article_ref') or row.get('identity') or 'missing').replace('/', '_').replace(':', '_')}_{row.get('source_role') or 'missing'}"


def flag_findings(
    record: Mapping[str, Any], *, where: str, row: Mapping[str, Any] | None = None
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    for flag in sorted(EXPECTED_FALSE_FLAGS):
        if record.get(flag) is True:
            findings.append(
                diagnostic(
                    "unsafe_safety_flag_true",
                    f"fail-closed safety flag is true: {flag}",
                    row=row,
                    json_path=f"$.{flag}",
                    path=where,
                )
            )
    flags = record.get("fail_closed_safety_flags")
    if isinstance(flags, Mapping):
        for flag in sorted(EXPECTED_FALSE_FLAGS):
            if flags.get(flag) is True:
                findings.append(
                    diagnostic(
                        "unsafe_safety_flag_true",
                        f"fail-closed safety flag is true: {flag}",
                        row=row,
                        json_path=f"$.fail_closed_safety_flags.{flag}",
                        path=where,
                    )
                )
    return findings


def validate_no_payload_leakage(value: Any, *, serialized: str, where: str) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []

    def walk(node: Any, path: str) -> None:
        if isinstance(node, dict):
            for key, item in node.items():
                if str(key) in FORBIDDEN_PAYLOAD_KEYS:
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


def validate_s03_closeout(
    closeout: Mapping[str, Any], conversion_summary: Mapping[str, Any]
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    rows = (
        conversion_summary.get("results")
        if isinstance(conversion_summary.get("results"), list)
        else []
    )
    parser_ready_count = sum(
        1 for row in rows if isinstance(row, Mapping) and row.get("parser_ready") is True
    )
    if closeout.get("schema_version") != S03_CLOSEOUT_SCHEMA_VERSION:
        findings.append(
            diagnostic(
                "unexpected_s03_closeout_schema",
                "S03 closeout schema is not recognized",
                json_path="$.schema_version",
            )
        )
    if closeout.get("status") != "passed" or closeout.get("failure_count") != 0:
        findings.append(
            diagnostic(
                "s03_closeout_not_passed",
                "S03 parser conversion closeout must be passed with zero failures",
                json_path="$.status",
            )
        )
    if closeout.get("row_count") != len(rows):
        findings.append(
            diagnostic(
                "stale_s03_closeout_row_count",
                "S03 closeout row_count does not match conversion results",
                json_path="$.row_count",
            )
        )
    if closeout.get("parser_ready_count") != parser_ready_count:
        findings.append(
            diagnostic(
                "stale_s03_closeout_parser_ready_count",
                "S03 closeout parser_ready_count does not match conversion results",
                json_path="$.parser_ready_count",
            )
        )
    findings.extend(flag_findings(closeout, where="parser-conversion-closeout-summary"))
    return findings


def validate_summary_counts(
    *,
    selection: Mapping[str, Any],
    conversion_summary: Mapping[str, Any],
    closeout: Mapping[str, Any],
    chunk_summary: Mapping[str, Any],
    diagnostics_rows: list[Mapping[str, Any]],
    report: str,
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    if (
        selection.get("selection_id") != SELECTION_ID
        or conversion_summary.get("selection_id") != SELECTION_ID
        or chunk_summary.get("selection_id") != SELECTION_ID
    ):
        findings.append(
            diagnostic(
                "selection_artifact_mismatch",
                "selection_id does not match across M031 artifacts",
                json_path="$.selection_id",
            )
        )
    if conversion_summary.get("schema_version") != CONVERSION_SCHEMA_VERSION:
        findings.append(
            diagnostic(
                "unexpected_conversion_schema",
                "conversion summary schema is not recognized",
                json_path="$.schema_version",
            )
        )
    if chunk_summary.get("schema_version") != CHUNK_SUMMARY_SCHEMA_VERSION:
        findings.append(
            diagnostic(
                "unexpected_chunk_summary_schema",
                "chunk summary schema is not recognized",
                json_path="$.schema_version",
            )
        )
    if chunk_summary.get("source_slice_id") != SOURCE_SLICE_ID:
        findings.append(
            diagnostic(
                "unexpected_chunk_source_slice",
                "chunk summary must consume S03 artifacts",
                json_path="$.source_slice_id",
            )
        )
    rows = conversion_summary.get("results")
    if not isinstance(rows, list):
        return findings + [
            diagnostic(
                "malformed_conversion_results",
                "conversion summary results must be a list",
                json_path="$.results",
            )
        ]
    parser_ready_rows = [
        row for row in rows if isinstance(row, Mapping) and row.get("parser_ready") is True
    ]
    non_parser_ready_rows = [
        row for row in rows if isinstance(row, Mapping) and row.get("parser_ready") is not True
    ]
    if closeout.get("row_count") != len(rows):
        findings.append(
            diagnostic(
                "stale_s03_closeout_row_count",
                "S03 closeout is stale relative to conversion summary",
                json_path="$.row_count",
            )
        )
    if chunk_summary.get("row_count") != len(rows) or len(diagnostics_rows) != len(rows):
        findings.append(
            diagnostic(
                "chunk_row_count_mismatch",
                "chunk summary/diagnostics row counts must match S03 rows",
                json_path="$.row_count",
            )
        )
    if chunk_summary.get("chunked_parser_ready_row_count") != len(parser_ready_rows):
        findings.append(
            diagnostic(
                "parser_ready_chunk_count_mismatch",
                "chunked parser-ready row count must match S03 parser-ready rows",
                json_path="$.chunked_parser_ready_row_count",
            )
        )
    if chunk_summary.get("zero_chunk_refusal_count") != len(non_parser_ready_rows):
        findings.append(
            diagnostic(
                "zero_chunk_refusal_count_mismatch",
                "non-parser-ready rows must remain zero-chunk refusals",
                json_path="$.zero_chunk_refusal_count",
            )
        )
    actual_counts = dict(
        sorted(Counter(str(row.get("status")) for row in diagnostics_rows).items())
    )
    if chunk_summary.get("counts") != actual_counts:
        findings.append(
            diagnostic(
                "chunk_status_counts_mismatch",
                "chunk summary counts do not match diagnostics JSONL",
                json_path="$.counts",
            )
        )
    if chunk_summary.get("package_count") != len(parser_ready_rows) or chunk_summary.get(
        "graph_readiness_package_count"
    ) != len(parser_ready_rows):
        findings.append(
            diagnostic(
                "package_count_mismatch",
                "parser-ready rows must have one structure and graph package each",
                json_path="$.package_count",
            )
        )
    if chunk_summary.get("pending_graph_readiness_review_count") != len(parser_ready_rows):
        findings.append(
            diagnostic(
                "pending_review_count_mismatch",
                "parser-ready packages must remain pending independent review",
                json_path="$.pending_graph_readiness_review_count",
            )
        )
    if chunk_summary.get("independent_review_completed_count") != 0:
        findings.append(
            diagnostic(
                "completed_review_claimed",
                "generated S04 artifacts must not claim completed independent review",
                json_path="$.independent_review_completed_count",
            )
        )
    for section in REQUIRED_REPORT_SECTIONS:
        if section not in report:
            findings.append(
                diagnostic(
                    "chunk_report_section_missing",
                    f"chunk report missing required section {section}",
                    json_path="$",
                )
            )
    findings.extend(flag_findings(chunk_summary, where="chunk-evidence-summary"))
    return findings


def validate_parser_ready_identity_and_hash(
    conversion_rows: list[Mapping[str, Any]], *, project_root: Path
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    for index, row in enumerate(conversion_rows):
        if row.get("parser_ready") is not True:
            continue
        if row.get("status") != "converted":
            findings.append(
                diagnostic(
                    "parser_ready_status_mismatch",
                    "parser-ready row must be converted",
                    row=row,
                    json_path=f"$.results[{index}].status",
                )
            )
        try:
            converted_path = resolve_artifact_path(
                project_root, row.get("converted_text_path"), code_label="converted_text_path"
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
            continue
        if not converted_path.exists() or not converted_path.is_file():
            findings.append(
                diagnostic(
                    "missing_converted_text_artifact",
                    "parser-ready converted text artifact is missing",
                    row=row,
                    json_path=f"$.results[{index}].converted_text_path",
                    path=converted_path,
                )
            )
            continue
        actual_hash = sha256_file(converted_path)
        actual_size = converted_path.stat().st_size
        if row.get("converted_text_sha256") != actual_hash:
            findings.append(
                diagnostic(
                    "converted_text_sha256_mismatch",
                    "parser-ready converted text hash no longer matches S03 summary",
                    row=row,
                    json_path=f"$.results[{index}].converted_text_sha256",
                    path=converted_path,
                )
            )
        if row.get("converted_text_byte_size") != actual_size:
            findings.append(
                diagnostic(
                    "converted_text_byte_size_mismatch",
                    "parser-ready converted text size no longer matches S03 summary",
                    row=row,
                    json_path=f"$.results[{index}].converted_text_byte_size",
                    path=converted_path,
                )
            )
    return findings


def validate_diagnostic_rows(
    *,
    conversion_rows: list[Mapping[str, Any]],
    diagnostics_rows: list[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    by_key = {row_key(row): row for row in conversion_rows}
    seen: set[tuple[str, str, str]] = set()
    for index, row in enumerate(diagnostics_rows):
        key = row_key(row)
        seen.add(key)
        source = by_key.get(key)
        if row.get("schema_version") != CHUNK_DIAGNOSTIC_SCHEMA_VERSION:
            findings.append(
                diagnostic(
                    "unexpected_chunk_diagnostic_schema",
                    "chunk diagnostic schema is not recognized",
                    row=row,
                    json_path=f"$[{index}].schema_version",
                )
            )
        if not isinstance(row.get("json_path"), str) or not row.get("json_path", "").startswith(
            "$.results["
        ):
            findings.append(
                diagnostic(
                    "missing_diagnostic_json_path",
                    "diagnostic row must carry a stable JSON path",
                    row=row,
                    json_path=f"$[{index}].json_path",
                )
            )
        if source is None:
            findings.append(
                diagnostic(
                    "chunk_diagnostic_without_s03_row",
                    "chunk diagnostic has no matching S03 row",
                    row=row,
                    json_path=f"$[{index}]",
                )
            )
            continue
        if source.get("parser_ready") is True:
            if row.get("status") != "chunked" or row.get("chunk_count", 0) <= 0:
                findings.append(
                    diagnostic(
                        "parser_ready_chunk_missing",
                        "parser-ready row must be chunked with package evidence",
                        row=row,
                        json_path=f"$[{index}].status",
                    )
                )
        else:
            if row.get("status") != "zero_chunk_refused" or row.get("chunk_count") != 0:
                findings.append(
                    diagnostic(
                        "non_parser_ready_not_zero_chunk_refused",
                        "non-parser-ready row must remain a zero-chunk refusal",
                        row=row,
                        json_path=f"$[{index}].status",
                    )
                )
            if (
                row.get("package_path") is not None
                or row.get("graph_readiness_package_path") is not None
            ):
                findings.append(
                    diagnostic(
                        "non_parser_ready_package_claim",
                        "non-parser-ready row must not carry package paths",
                        row=row,
                        json_path=f"$[{index}].package_path",
                    )
                )
        findings.extend(flag_findings(row, where="chunk-diagnostics-row", row=row))
    missing = set(by_key) - seen
    for key in sorted(missing):
        findings.append(
            diagnostic(
                "missing_chunk_diagnostic_for_s03_row",
                f"missing chunk diagnostic for S03 row {key}",
                json_path="$",
            )
        )
    return findings


def validate_package_pair(
    *,
    diagnostic_row: Mapping[str, Any],
    chunk_summary: Mapping[str, Any],
    project_root: Path,
) -> tuple[list[dict[str, Any]], int, int, int]:
    findings: list[dict[str, Any]] = []
    chunk_count = 0
    evidence_path_count = 0
    graph_package_count = 0
    try:
        package_path = resolve_artifact_path(
            project_root, diagnostic_row.get("package_path"), code_label="package_path"
        )
        graph_path = resolve_artifact_path(
            project_root,
            diagnostic_row.get("graph_readiness_package_path"),
            code_label="graph_readiness_package_path",
        )
    except ValueError as exc:
        return (
            [diagnostic(str(exc), "package path is unsafe or missing", row=diagnostic_row)],
            0,
            0,
            0,
        )
    if not package_path.exists():
        findings.append(
            diagnostic(
                "missing_structure_package",
                "structure-aware chunk package is missing",
                row=diagnostic_row,
                json_path="$.package_path",
                path=package_path,
            )
        )
        return findings, 0, 0, 0
    if not graph_path.exists():
        findings.append(
            diagnostic(
                "missing_graph_readiness_package",
                "graph-readiness package is missing",
                row=diagnostic_row,
                json_path="$.graph_readiness_package_path",
                path=graph_path,
            )
        )
        return findings, 0, 0, 0
    try:
        package = load_json(package_path)
    except Exception as exc:
        findings.append(
            diagnostic(
                "malformed_structure_package",
                f"structure-aware package is malformed: {exc}",
                row=diagnostic_row,
                path=package_path,
            )
        )
        return findings, 0, 0, 0
    try:
        graph_package = load_json(graph_path)
    except Exception as exc:
        findings.append(
            diagnostic(
                "malformed_graph_readiness_package",
                f"graph-readiness package is malformed: {exc}",
                row=diagnostic_row,
                path=graph_path,
            )
        )
        return findings, 0, 0, 0
    if package.get("schema_version") != STRUCTURE_PACKAGE_SCHEMA_VERSION:
        findings.append(
            diagnostic(
                "unexpected_structure_package_schema",
                "structure-aware package schema is not recognized",
                row=diagnostic_row,
                json_path="$.schema_version",
                path=package_path,
            )
        )
    if graph_package.get("schema_version") != GRAPH_PACKAGE_SCHEMA_VERSION:
        findings.append(
            diagnostic(
                "unexpected_graph_package_schema",
                "graph-readiness package schema is not recognized",
                row=diagnostic_row,
                json_path="$.schema_version",
                path=graph_path,
            )
        )
    validation = validation_to_dict(validate_import_ready_package(package))
    if validation.get("valid_package") is not True:
        findings.append(
            diagnostic(
                "invalid_import_contract_package",
                "structure package failed import-contract validation",
                row=diagnostic_row,
                json_path="$.package_validation",
                path=package_path,
            )
        )
    if (
        validation.get("import_ready") is not False
        or validation.get("import_eligible_chunk_count") != 0
    ):
        findings.append(
            diagnostic(
                "permissive_import_contract_package",
                "S04 package must remain valid but not import-ready",
                row=diagnostic_row,
                json_path="$.package_validation.import_ready",
                path=package_path,
            )
        )
    graph_validation = (
        graph_package.get("validation")
        if isinstance(graph_package.get("validation"), Mapping)
        else {}
    )
    if (
        graph_validation.get("import_ready") is not False
        or graph_validation.get("import_eligible_chunk_count") != 0
    ):
        findings.append(
            diagnostic(
                "permissive_graph_package_validation",
                "graph package validation must be fail-closed",
                row=diagnostic_row,
                json_path="$.validation.import_ready",
                path=graph_path,
            )
        )
    if (
        graph_package.get("review_state") != "pending_independent_graph_readiness_review"
        or graph_package.get("output_contract_completed") is not False
    ):
        findings.append(
            diagnostic(
                "unsafe_graph_review_state",
                "graph package must be pending independent review and incomplete",
                row=diagnostic_row,
                json_path="$.review_state",
                path=graph_path,
            )
        )
    chunks = package.get("chunks") if isinstance(package.get("chunks"), list) else []
    elements = package.get("elements") if isinstance(package.get("elements"), list) else []
    annotations = package.get("annotations") if isinstance(package.get("annotations"), list) else []
    chunk_count = len(chunks)
    if (
        diagnostic_row.get("chunk_count") != chunk_count
        or graph_package.get("chunk_count") != chunk_count
    ):
        findings.append(
            diagnostic(
                "package_chunk_count_mismatch",
                "chunk counts disagree across diagnostic/package/graph package",
                row=diagnostic_row,
                json_path="$.chunk_count",
                path=package_path,
            )
        )
    if diagnostic_row.get("element_count") != len(elements) or graph_package.get(
        "element_count"
    ) != len(elements):
        findings.append(
            diagnostic(
                "package_element_count_mismatch",
                "element counts disagree across diagnostic/package/graph package",
                row=diagnostic_row,
                json_path="$.element_count",
                path=package_path,
            )
        )
    if diagnostic_row.get("annotation_count") != len(annotations) or graph_package.get(
        "annotation_count"
    ) != len(annotations):
        findings.append(
            diagnostic(
                "package_annotation_count_mismatch",
                "annotation counts disagree across diagnostic/package/graph package",
                row=diagnostic_row,
                json_path="$.annotation_count",
                path=package_path,
            )
        )
    for index, chunk in enumerate(chunks):
        if isinstance(chunk, Mapping) and chunk.get("source_span") and chunk.get("source_artifact"):
            evidence_path_count += 1
        else:
            findings.append(
                diagnostic(
                    "missing_chunk_evidence_path",
                    "chunk is missing source evidence path/span",
                    row=diagnostic_row,
                    json_path=f"$.chunks[{index}]",
                    path=package_path,
                )
            )
        if isinstance(chunk, Mapping) and "trusted_kg_import" not in set(
            chunk.get("excluded_uses") or []
        ):
            findings.append(
                diagnostic(
                    "unsafe_chunk_excluded_uses",
                    "S04 chunks must exclude trusted KG import until independent review",
                    row=diagnostic_row,
                    json_path=f"$.chunks[{index}].excluded_uses",
                    path=package_path,
                )
            )
    if evidence_path_count != chunk_count:
        findings.append(
            diagnostic(
                "evidence_path_count_mismatch",
                "every chunk must have source evidence path/span",
                row=diagnostic_row,
                json_path="$.chunks",
                path=package_path,
            )
        )
    if graph_path.as_posix() not in [
        str(value) for value in chunk_summary.get("graph_readiness_package_paths", [])
    ] and diagnostic_row.get("graph_readiness_package_path") not in chunk_summary.get(
        "graph_readiness_package_paths", []
    ):
        findings.append(
            diagnostic(
                "graph_package_path_missing_from_summary",
                "graph package path is absent from summary path list",
                row=diagnostic_row,
                json_path="$.graph_readiness_package_paths",
                path=graph_path,
            )
        )
    graph_package_count = 1
    findings.extend(flag_findings(package, where=package_path.as_posix(), row=diagnostic_row))
    findings.extend(flag_findings(graph_package, where=graph_path.as_posix(), row=diagnostic_row))
    findings.extend(
        validate_no_payload_leakage(
            package, serialized=json.dumps(package, sort_keys=True), where=package_path.as_posix()
        )
    )
    findings.extend(
        validate_no_payload_leakage(
            graph_package,
            serialized=json.dumps(graph_package, sort_keys=True),
            where=graph_path.as_posix(),
        )
    )
    return findings, chunk_count, evidence_path_count, graph_package_count


def validate_packages(
    *,
    diagnostics_rows: list[Mapping[str, Any]],
    chunk_summary: Mapping[str, Any],
    project_root: Path,
) -> tuple[list[dict[str, Any]], dict[str, int]]:
    findings: list[dict[str, Any]] = []
    counters = {
        "package_count": 0,
        "graph_readiness_package_count": 0,
        "chunk_count": 0,
        "evidence_path_count": 0,
    }
    for row in diagnostics_rows:
        if row.get("status") != "chunked":
            continue
        package_findings, chunk_count, evidence_path_count, graph_package_count = (
            validate_package_pair(
                diagnostic_row=row, chunk_summary=chunk_summary, project_root=project_root
            )
        )
        findings.extend(package_findings)
        counters["package_count"] += (
            1
            if not any(
                f.get("diagnostic_code")
                in {"missing_structure_package", "malformed_structure_package"}
                and f.get("package_key") == row.get("package_key")
                for f in package_findings
            )
            else 0
        )
        counters["graph_readiness_package_count"] += graph_package_count
        counters["chunk_count"] += chunk_count
        counters["evidence_path_count"] += evidence_path_count
    return findings, counters


def validate_review_artifact_set(
    *, review_dir: Path, events_path: Path, review_summary_path: Path, expected_review_count: int
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    try:
        events = load_jsonl(events_path)
    except Exception as exc:
        return [
            diagnostic(
                "malformed_review_event",
                f"review events JSONL is malformed or missing: {exc}",
                json_path="$",
                path=events_path,
            )
        ]
    requested_count = 0
    summary_count = 0
    for line_index, event in enumerate(events, start=1):
        if event.get("event") == "independent_review.requested":
            requested_count += 1
        if event.get("event") == "independent_review.summary":
            summary_count += 1
        if event.get("event") == "independent_review.verdict":
            findings.append(
                diagnostic(
                    "fabricated_completed_review_event",
                    "S04 generated artifacts must not include independent_review.verdict events",
                    json_path=f"$[{line_index - 1}].event",
                    path=events_path,
                )
            )
        if event.get("output_contract_completed") is not False:
            findings.append(
                diagnostic(
                    "review_event_contract_completed",
                    "generated review events must keep output_contract_completed=false",
                    json_path=f"$[{line_index - 1}].output_contract_completed",
                    path=events_path,
                )
            )
        if event.get("independent_review_completed") is not False:
            findings.append(
                diagnostic(
                    "review_event_completed_claim",
                    "generated review events must not claim independent review completion",
                    json_path=f"$[{line_index - 1}].independent_review_completed",
                    path=events_path,
                )
            )
        findings.extend(flag_findings(event, where=events_path.as_posix()))
    if requested_count != expected_review_count:
        findings.append(
            diagnostic(
                "review_requested_count_mismatch",
                "review requested event count must match parser-ready package count",
                json_path="$",
                path=events_path,
            )
        )
    if expected_review_count and summary_count != 1:
        findings.append(
            diagnostic(
                "review_summary_event_count_mismatch",
                "review events must contain exactly one summary event",
                json_path="$",
                path=events_path,
            )
        )
    try:
        review_summary = review_summary_path.read_text(encoding="utf-8")
    except OSError as exc:
        findings.append(
            diagnostic(
                "missing_review_summary",
                f"independent review summary is missing: {exc}",
                path=review_summary_path,
            )
        )
        review_summary = ""
    if "Independent reviewer verdicts are still required" not in review_summary:
        findings.append(
            diagnostic(
                "review_summary_not_pending",
                "independent review summary must clearly remain pending",
                path=review_summary_path,
            )
        )
    validation = validate_review_artifacts(
        review_dir=review_dir, events_path=events_path, require_completed_review=False
    )
    if not validation.ok:
        findings.append(
            diagnostic(
                "review_artifact_validation_failed",
                "; ".join(validation.diagnostics),
                path=review_dir,
            )
        )
    completed_validation = validate_review_artifacts(
        review_dir=review_dir, events_path=events_path, require_completed_review=True
    )
    if completed_validation.ok:
        findings.append(
            diagnostic(
                "review_artifact_completed_too_early",
                "generated review artifacts must fail completed-review validation until an independent reviewer acts",
                path=review_dir,
            )
        )
    findings.extend(
        validate_no_payload_leakage(
            events, serialized=json.dumps(events, sort_keys=True), where=events_path.as_posix()
        )
    )
    findings.extend(
        validate_no_payload_leakage(
            {}, serialized=review_summary, where=review_summary_path.as_posix()
        )
    )
    return findings


def build_closeout_summary(
    chunk_summary: Mapping[str, Any], findings: list[dict[str, Any]], counters: Mapping[str, int]
) -> dict[str, Any]:
    return {
        "schema_version": VERIFIER_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "passed" if not findings else "failed",
        "failure_count": len(findings),
        "row_count": chunk_summary.get("row_count", 0),
        "parser_ready_row_count": chunk_summary.get("parser_ready_row_count", 0),
        "zero_chunk_refusal_count": chunk_summary.get("zero_chunk_refusal_count", 0),
        "package_count": counters.get("package_count", 0),
        "graph_readiness_package_count": counters.get("graph_readiness_package_count", 0),
        "chunk_count": counters.get("chunk_count", 0),
        "evidence_path_count": counters.get("evidence_path_count", 0),
        "pending_graph_readiness_review_count": chunk_summary.get(
            "pending_graph_readiness_review_count", 0
        ),
        "independent_review_completed_count": 0,
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
        "automated_state_is_structural_only": True,
        "fail_closed_safety_flags": dict.fromkeys(sorted(EXPECTED_FALSE_FLAGS), False),
    }


def render_report(summary: Mapping[str, Any], findings: list[Mapping[str, Any]]) -> str:
    lines = [
        "# M031 Chunk Evidence Replay Closeout Report",
        "",
        "Validate-only closeout audit for the S04 chunk/evidence replay and graph-readiness handoff. This report is metadata-only and does not embed source text, chunk text, PDF bytes, embeddings, vectors, graph facts, or LadybugDB write claims.",
        "",
        f"- Status: `{summary.get('status')}`",
        f"- Failure count: {summary.get('failure_count')}",
        f"- Row count: {summary.get('row_count')}",
        f"- Parser-ready rows: {summary.get('parser_ready_row_count')}",
        f"- Zero-chunk refusals: {summary.get('zero_chunk_refusal_count')}",
        f"- Package count: {summary.get('package_count')}",
        f"- Chunk evidence path count: {summary.get('evidence_path_count')}",
        "- Import-eligible chunks: `0`",
        "- Network fetch attempted: `False`",
        "- Graph/import/LadybugDB writes: `False`",
        "",
        "## Failure Modes",
        "",
        "Missing/malformed JSON, stale S03 closeout counts, unsafe or missing parser-ready converted paths, hash drift, missing/corrupt package JSON, missing chunk evidence spans, malformed review events, stale/generated review completion claims, raw payload leakage, and permissive graph/import/LadybugDB flags all produce stable non-zero diagnostics.",
        "",
        "## Load Profile",
        "",
        "Expected load is seven S03 rows, one parser-ready package pair, and one pending review bundle. At 10x, local JSON parsing and converted-text hashing saturate first; hashing is streamed in 1 MiB chunks, and there is no network, model, conversion, chunk generation, graph import, or LadybugDB write path.",
        "",
        "## Negative Tests",
        "",
        "Covered by `tests/test_m031_chunk_evidence_replay.py`: stale S03 closeout, missing/corrupt package JSON, removed chunk evidence paths, malformed/fabricated review events, raw payload leakage, and permissive import/write flags.",
        "",
        "## Graph-Readiness Review Handoff",
        "",
        "Generated review artifacts are accepted only as pending-review evidence. Completed-review validation must remain failing until an independent reviewer records `output_contract_completed=true` verdict events.",
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
                f"source_role=`{finding.get('source_role') or '<none>'}` package_key=`{finding.get('package_key') or '<none>'}` json_path=`{finding.get('json_path')}` path=`{finding.get('path') or '<none>'}`"
            )
    return "\n".join(lines) + "\n"


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", default=DEFAULT_CORPUS_DIR / "selection.json", type=Path)
    parser.add_argument(
        "--conversion-summary",
        default=DEFAULT_CORPUS_DIR / "conversion-quality" / "conversion-quality-summary.json",
        type=Path,
    )
    parser.add_argument(
        "--s03-closeout-summary",
        default=DEFAULT_CORPUS_DIR / "parser-conversion-closeout-summary.json",
        type=Path,
    )
    parser.add_argument(
        "--chunk-summary",
        default=DEFAULT_CORPUS_DIR / "chunk-evidence" / "chunk-evidence-summary.json",
        type=Path,
    )
    parser.add_argument(
        "--chunk-diagnostics",
        default=DEFAULT_CORPUS_DIR / "chunk-evidence" / "chunk-evidence-diagnostics.jsonl",
        type=Path,
    )
    parser.add_argument(
        "--chunk-report",
        default=DEFAULT_CORPUS_DIR / "chunk-evidence" / "chunk-evidence-report.md",
        type=Path,
    )
    parser.add_argument(
        "--review-events",
        default=DEFAULT_CORPUS_DIR / "chunk-evidence" / "independent-review-events.jsonl",
        type=Path,
    )
    parser.add_argument(
        "--review-dir", default=DEFAULT_CORPUS_DIR / "graph-readiness-review", type=Path
    )
    parser.add_argument(
        "--review-summary",
        default=DEFAULT_CORPUS_DIR / "graph-readiness-review" / "independent-review-summary.md",
        type=Path,
    )
    parser.add_argument("--project-root", default=Path.cwd(), type=Path)
    parser.add_argument(
        "--write-summary",
        default=DEFAULT_CORPUS_DIR / "chunk-evidence-closeout-summary.json",
        type=Path,
    )
    parser.add_argument(
        "--write-diagnostics",
        default=DEFAULT_CORPUS_DIR / "chunk-evidence-closeout-diagnostics.jsonl",
        type=Path,
    )
    parser.add_argument(
        "--write-report",
        default=DEFAULT_CORPUS_DIR / "chunk-evidence-closeout-report.md",
        type=Path,
    )
    return parser.parse_args(argv)


def verify(
    argv: list[str] | argparse.Namespace | None = None,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    args = parse_args(argv) if not isinstance(argv, argparse.Namespace) else argv
    findings: list[dict[str, Any]] = []
    counters = {
        "package_count": 0,
        "graph_readiness_package_count": 0,
        "chunk_count": 0,
        "evidence_path_count": 0,
    }
    project_root = args.project_root.resolve()
    try:
        selection = load_json(args.selection)
        conversion_summary = load_json(args.conversion_summary)
        closeout = load_json(args.s03_closeout_summary)
        chunk_summary = load_json(args.chunk_summary)
        diagnostics_rows = load_jsonl(args.chunk_diagnostics)
        chunk_report = args.chunk_report.read_text(encoding="utf-8")
    except Exception as exc:
        findings.append(
            diagnostic(
                "verifier_setup_failed", f"failed to load required artifact: {exc}", path="$"
            )
        )
        chunk_summary = {}
        diagnostics_rows = []
    else:
        rows = (
            conversion_summary.get("results")
            if isinstance(conversion_summary.get("results"), list)
            else []
        )
        conversion_rows = [row for row in rows if isinstance(row, Mapping)]
        findings.extend(validate_s03_closeout(closeout, conversion_summary))
        findings.extend(
            validate_summary_counts(
                selection=selection,
                conversion_summary=conversion_summary,
                closeout=closeout,
                chunk_summary=chunk_summary,
                diagnostics_rows=diagnostics_rows,
                report=chunk_report,
            )
        )
        findings.extend(
            validate_parser_ready_identity_and_hash(conversion_rows, project_root=project_root)
        )
        findings.extend(
            validate_diagnostic_rows(
                conversion_rows=conversion_rows, diagnostics_rows=diagnostics_rows
            )
        )
        package_findings, counters = validate_packages(
            diagnostics_rows=diagnostics_rows,
            chunk_summary=chunk_summary,
            project_root=project_root,
        )
        findings.extend(package_findings)
        expected_review_count = sum(1 for row in diagnostics_rows if row.get("status") == "chunked")
        findings.extend(
            validate_review_artifact_set(
                review_dir=args.review_dir,
                events_path=args.review_events,
                review_summary_path=args.review_summary,
                expected_review_count=expected_review_count,
            )
        )
        findings.extend(
            validate_no_payload_leakage(
                chunk_summary,
                serialized=json.dumps(chunk_summary, sort_keys=True),
                where=args.chunk_summary.as_posix(),
            )
        )
        findings.extend(
            validate_no_payload_leakage(
                diagnostics_rows,
                serialized=json.dumps(diagnostics_rows, sort_keys=True),
                where=args.chunk_diagnostics.as_posix(),
            )
        )
        findings.extend(
            validate_no_payload_leakage(
                {}, serialized=chunk_report, where=args.chunk_report.as_posix()
            )
        )
    closeout_summary = build_closeout_summary(chunk_summary, findings, counters)
    closeout_report = render_report(closeout_summary, findings)
    if args.write_summary:
        write_json(args.write_summary, closeout_summary)
    if args.write_diagnostics:
        write_jsonl(args.write_diagnostics, findings)
    if args.write_report:
        atomic_write_text(args.write_report, closeout_report)
    return closeout_summary, findings


def main(argv: list[str] | None = None) -> int:
    try:
        summary, findings = verify(argv)
    except Exception as exc:
        sys.stderr.write(
            json.dumps(
                {"status": "failed", "code": "verifier_unhandled_failure", "message": str(exc)},
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
