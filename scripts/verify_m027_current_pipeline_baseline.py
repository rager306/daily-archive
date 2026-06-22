#!/usr/bin/env python3
"""Validate-only verifier for the M027 S04 current pipeline baseline replay.

The verifier consumes already-generated S04 baseline artifacts and validates
provenance, artifact completeness, redaction, and fail-closed safety flags. It
never reruns conversion, fetches network sources, imports graph state, or writes
LadybugDB/production state.
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
SLICE_ID = "S04"
SOURCE_SLICE_ID = "S03"
SELECTION_ID = "m027-mixed-source-corpus-v1"
SCHEMA_VERSION = "m027-current-pipeline-baseline.v1"
DIAGNOSTIC_SCHEMA_VERSION = "m027-current-pipeline-baseline-diagnostic.v1"
ARTIFACT_SCHEMA_VERSION = "m027-current-pipeline-baseline-artifact.v1"
VERIFIER_SCHEMA_VERSION = "m027-current-pipeline-baseline-verifier.v1"
ROOT = Path(__file__).resolve().parents[1]
CORPUS_DIR = ROOT / "data" / "article_corpora" / SELECTION_ID
SUMMARY_PATH = CORPUS_DIR / "current-pipeline-baseline-summary.json"
DIAGNOSTICS_PATH = CORPUS_DIR / "current-pipeline-baseline-diagnostics.jsonl"
REPORT_PATH = CORPUS_DIR / "current-pipeline-baseline-report.md"
OUTPUT_DIR = CORPUS_DIR / "current-pipeline-baseline"
CONVERSION_SUMMARY_PATH = CORPUS_DIR / "conversion-quality-summary.json"

EXPECTED_ARTICLE_COUNT = 6
EXPECTED_VARIANT_COUNT = 11
TERMINAL_CONVERSION_STATUSES = {"converted", "metadata_only", "blocked", "failed", "low_quality"}
TERMINAL_BASELINE_DIAGNOSTIC_CODES = {
    "current_pipeline_retrieval_only_chunks",
    "current_pipeline_zero_chunks",
    "metadata_only_not_replayed",
}
REQUIRED_DIAGNOSTIC_CODES = {
    "s03_linkage_verified",
    "converted_payload_hash_verified",
    "no_converted_payload_expected",
    "metadata_only_not_replayed",
}
FALSE_SAFETY_FLAGS = {
    "network_fetch_attempted",
    "graph_import_allowed",
    "trusted_kg_import_allowed",
    "production_import_attempted",
    "production_ladybugdb_write_allowed",
    "ladybugdb_written",
    "raw_text_embedded_in_metadata",
    "raw_binary_embedded_in_metadata",
    "raw_payload_embedded_in_metadata",
}
UNSAFE_TRUE_FLAGS = {
    *FALSE_SAFETY_FLAGS,
    "raw_text_included",
    "raw_binary_included",
    "raw_payload_included",
    "embeddings_included",
    "unsupported_readiness_claim",
    "graph_readiness_claim",
    "trusted_fact_claim",
    "hardening_applied",
}
FORBIDDEN_PAYLOAD_KEYS = {
    "text",
    "raw_text",
    "chunk_text",
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
    root_resolved = root.resolve()
    if isinstance(relative_path, str) and Path(relative_path).is_absolute():
        resolved = Path(relative_path).resolve()
        if not resolved.is_relative_to(root_resolved):
            raise ValueError(f"{code_label}_escapes_root")
        return resolved
    normalized = safe_relative_path(relative_path, code_label=code_label)
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
        "path": rel(path) if isinstance(path, Path) else path,
        "json_path": json_path,
        "article_ref": article_ref,
        "variant_id": variant_id,
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_import_allowed": False,
    }


def validate_no_payload_leakage(value: Any, *, serialized: str, where: str) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []

    def walk(node: Any, path: str) -> None:
        if isinstance(node, dict):
            for key, item in node.items():
                if key in FORBIDDEN_PAYLOAD_KEYS:
                    findings.append(
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
            findings.append(
                diagnostic(
                    "metadata_payload_snippet_leakage",
                    f"metadata artifact includes forbidden raw payload snippet {snippet!r}",
                    path=where,
                )
            )
    return findings


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
                )
            )
    return findings


def conversion_index(
    conversion_summary: Mapping[str, Any],
) -> dict[tuple[str, str], dict[str, Any]]:
    rows = conversion_summary.get("results")
    if not isinstance(rows, list):
        return {}
    indexed: dict[tuple[str, str], dict[str, Any]] = {}
    for row in rows:
        if (
            isinstance(row, dict)
            and isinstance(row.get("article_ref"), str)
            and isinstance(row.get("variant_id"), str)
        ):
            indexed[(row["article_ref"], row["variant_id"])] = row
    return indexed


def validate_converted_payload(
    record: Mapping[str, Any], conversion_row: Mapping[str, Any] | None, *, root: Path
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    article_ref = str(record.get("article_ref")) if record.get("article_ref") else None
    variant_id = str(record.get("variant_id")) if record.get("variant_id") else None
    payload = record.get("converted_payload")
    if not isinstance(payload, dict):
        return [
            diagnostic(
                "missing_converted_payload_provenance",
                "baseline row lacks converted payload provenance",
                article_ref=article_ref,
                variant_id=variant_id,
            )
        ]
    parser_ready = record.get("parser_ready") is True
    if not parser_ready:
        if payload.get("verified") is not False:
            findings.append(
                diagnostic(
                    "metadata_only_payload_marked_verified",
                    "metadata-only row has verified converted payload provenance",
                    article_ref=article_ref,
                    variant_id=variant_id,
                )
            )
        return findings
    if payload.get("verified") is not True:
        findings.append(
            diagnostic(
                "parser_ready_payload_not_verified",
                "parser-ready row lacks verified converted payload provenance",
                article_ref=article_ref,
                variant_id=variant_id,
            )
        )
        return findings
    try:
        converted_path = safe_under_root(
            root, payload.get("path"), code_label="converted_payload_path"
        )
    except ValueError as exc:
        return [
            *findings,
            diagnostic(
                f"unsafe_converted_payload_path:{exc}",
                f"unsafe converted payload path: {exc}",
                article_ref=article_ref,
                variant_id=variant_id,
            ),
        ]
    if not converted_path.exists() or not converted_path.is_file():
        findings.append(
            diagnostic(
                "missing_converted_payload_artifact",
                "converted payload artifact is missing",
                path=converted_path,
                article_ref=article_ref,
                variant_id=variant_id,
            )
        )
        return findings
    actual_hash = sha256_file(converted_path)
    actual_size = converted_path.stat().st_size
    if payload.get("sha256") != actual_hash:
        findings.append(
            diagnostic(
                "converted_payload_sha256_mismatch",
                "converted payload hash is stale",
                path=converted_path,
                article_ref=article_ref,
                variant_id=variant_id,
            )
        )
    if payload.get("byte_size") != actual_size:
        findings.append(
            diagnostic(
                "converted_payload_byte_size_mismatch",
                "converted payload byte size is stale",
                path=converted_path,
                article_ref=article_ref,
                variant_id=variant_id,
            )
        )
    if conversion_row is None:
        findings.append(
            diagnostic(
                "conversion_row_missing_for_baseline",
                "baseline row has no matching S03 conversion row",
                article_ref=article_ref,
                variant_id=variant_id,
            )
        )
    else:
        if conversion_row.get("converted_text_sha256") != payload.get("sha256"):
            findings.append(
                diagnostic(
                    "baseline_conversion_hash_mismatch",
                    "baseline converted payload hash differs from S03 conversion row",
                    article_ref=article_ref,
                    variant_id=variant_id,
                )
            )
        if conversion_row.get("converted_text_byte_size") != payload.get("byte_size"):
            findings.append(
                diagnostic(
                    "baseline_conversion_size_mismatch",
                    "baseline converted payload byte size differs from S03 conversion row",
                    article_ref=article_ref,
                    variant_id=variant_id,
                )
            )
    return findings


def load_artifact(
    path_value: Any, *, root: Path, article_ref: str
) -> tuple[dict[str, Any] | None, list[dict[str, Any]]]:
    try:
        artifact_path = safe_under_root(root, path_value, code_label="baseline_artifact_path")
    except ValueError as exc:
        return None, [
            diagnostic(
                f"unsafe_baseline_artifact_path:{exc}",
                f"unsafe baseline artifact path: {exc}",
                article_ref=article_ref,
            )
        ]
    if not artifact_path.exists() or not artifact_path.is_file():
        return None, [
            diagnostic(
                "missing_baseline_artifact",
                "per-article baseline artifact is missing",
                path=artifact_path,
                article_ref=article_ref,
            )
        ]
    try:
        artifact = load_json(artifact_path)
    except Exception as exc:
        return None, [
            diagnostic(
                "malformed_baseline_artifact",
                f"per-article baseline artifact is malformed: {exc}",
                path=artifact_path,
                article_ref=article_ref,
            )
        ]
    findings = validate_no_payload_leakage(
        artifact, serialized=json.dumps(artifact, sort_keys=True), where=rel(artifact_path)
    )
    findings.extend(false_flag_diagnostics(artifact, where=rel(artifact_path)))
    readiness = artifact.get("readiness")
    if isinstance(readiness, dict):
        findings.extend(
            false_flag_diagnostics(readiness, where=f"{rel(artifact_path)} $.readiness")
        )
    if artifact.get("schema_version") != ARTIFACT_SCHEMA_VERSION:
        findings.append(
            diagnostic(
                "artifact_contract_mismatch",
                "per-article artifact has unexpected schema_version",
                path=artifact_path,
                article_ref=article_ref,
                json_path="$.schema_version",
            )
        )
    if artifact.get("article_ref") != article_ref:
        findings.append(
            diagnostic(
                "artifact_article_ref_mismatch",
                "per-article artifact article_ref does not match summary",
                path=artifact_path,
                article_ref=article_ref,
                json_path="$.article_ref",
            )
        )
    return artifact, findings


def validate_report(report_text: str, *, path: Path) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    for heading in ["Failure Modes", "Load Profile", "Negative Tests"]:
        if heading not in report_text:
            findings.append(
                diagnostic(
                    "report_section_missing",
                    f"baseline report missing {heading} section",
                    path=path,
                )
            )
    forbidden_claims = [
        "graph ready: **true**",
        "trusted fact claim: **true**",
        "import ready: **true**",
        "hardening applied: **true**",
    ]
    lowered = report_text.lower()
    for claim in forbidden_claims:
        if claim in lowered:
            findings.append(
                diagnostic(
                    "unsupported_readiness_claim",
                    f"baseline report contains unsupported readiness/import claim: {claim}",
                    path=path,
                )
            )
    return findings


def verify(args: argparse.Namespace) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    diagnostics: list[dict[str, Any]] = []
    summary = load_json(args.summary)
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
    diagnostics.extend(validate_report(report_text, path=args.report))

    expected_top = {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "completed",
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
    readiness = summary.get("readiness")
    if isinstance(readiness, dict):
        diagnostics.extend(false_flag_diagnostics(readiness, where="$.readiness"))
    else:
        diagnostics.append(
            diagnostic(
                "missing_readiness_summary",
                "summary lacks readiness object",
                path=args.summary,
                json_path="$.readiness",
            )
        )

    try:
        conversion_summary_path = safe_under_root(
            args.root, summary.get("conversion_summary_path"), code_label="conversion_summary_path"
        )
    except ValueError as exc:
        conversion_summary_path = args.conversion_summary
        diagnostics.append(
            diagnostic(
                f"unsafe_conversion_summary_path:{exc}",
                f"unsafe conversion summary path: {exc}",
                path=args.summary,
                json_path="$.conversion_summary_path",
            )
        )
    if not conversion_summary_path.exists():
        conversion_summary: dict[str, Any] = {}
        diagnostics.append(
            diagnostic(
                "missing_conversion_summary",
                "S03 conversion summary referenced by baseline is missing",
                path=conversion_summary_path,
            )
        )
    else:
        conversion_summary = load_json(conversion_summary_path)
        actual_conversion_sha = sha256_file(conversion_summary_path)
        if summary.get("conversion_summary_sha256") != actual_conversion_sha:
            diagnostics.append(
                diagnostic(
                    "conversion_summary_sha256_mismatch",
                    "baseline conversion_summary_sha256 is stale",
                    path=conversion_summary_path,
                    json_path="$.conversion_summary_sha256",
                )
            )
        try:
            s03_source_path = safe_under_root(
                args.root,
                conversion_summary.get("source_summary_path"),
                code_label="source_summary_path",
            )
            if conversion_summary.get("source_summary_sha256") != sha256_file(s03_source_path):
                diagnostics.append(
                    diagnostic(
                        "s03_source_summary_sha256_mismatch",
                        "S03 conversion source_summary_sha256 is stale",
                        path=s03_source_path,
                        json_path="$.source_summary_sha256",
                    )
                )
        except Exception as exc:
            diagnostics.append(
                diagnostic(
                    "s03_source_linkage_unverifiable",
                    f"S03 source-summary linkage cannot be verified: {exc}",
                    path=conversion_summary_path,
                )
            )
    conversion_rows = conversion_index(conversion_summary)

    rows = summary.get("article_results")
    if not isinstance(rows, list):
        rows = []
        diagnostics.append(
            diagnostic(
                "missing_baseline_results",
                "summary is missing article_results list",
                path=args.summary,
                json_path="$.article_results",
            )
        )
    diagnostic_counts = Counter(str(row.get("diagnostic_code")) for row in jsonl_rows)
    if sum(diagnostic_counts.values()) != len(jsonl_rows):
        diagnostics.append(
            diagnostic(
                "diagnostic_row_count_mismatch",
                "JSONL diagnostic count accounting is inconsistent",
                path=args.diagnostics,
            )
        )
    if dict(sorted(diagnostic_counts.items())) != summary.get("diagnostic_counts"):
        diagnostics.append(
            diagnostic(
                "diagnostic_counts_mismatch",
                "summary diagnostic_counts do not match JSONL diagnostics",
                path=args.summary,
                json_path="$.diagnostic_counts",
            )
        )
    missing_codes = REQUIRED_DIAGNOSTIC_CODES - set(diagnostic_counts)
    if missing_codes:
        diagnostics.append(
            diagnostic(
                "required_diagnostic_code_missing",
                f"baseline diagnostics missing required codes: {sorted(missing_codes)}",
                path=args.diagnostics,
            )
        )
    for index, row in enumerate(jsonl_rows):
        diagnostics.extend(false_flag_diagnostics(row, where=rel(args.diagnostics), row=row))
        if row.get("schema_version") != DIAGNOSTIC_SCHEMA_VERSION:
            diagnostics.append(
                diagnostic(
                    "diagnostic_contract_mismatch",
                    "diagnostic JSONL row has unexpected schema_version",
                    path=args.diagnostics,
                    json_path=f"$[{index}].schema_version",
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
                f"expected {args.expected_variant_count} baseline variants",
                path=args.summary,
                json_path="$.variant_count",
            )
        )
    parser_ready_count = sum(
        1 for row in rows if isinstance(row, dict) and row.get("parser_ready") is True
    )
    if summary.get("parser_ready_variant_count") != parser_ready_count:
        diagnostics.append(
            diagnostic(
                "parser_ready_variant_count_mismatch",
                "summary parser-ready count does not match baseline rows",
                path=args.summary,
                json_path="$.parser_ready_variant_count",
            )
        )
    if summary.get("metadata_only_variant_count") != len(rows) - parser_ready_count:
        diagnostics.append(
            diagnostic(
                "metadata_only_variant_count_mismatch",
                "summary metadata-only count does not match baseline rows",
                path=args.summary,
                json_path="$.metadata_only_variant_count",
            )
        )

    variants_by_article: dict[str, set[str]] = defaultdict(set)
    artifacts_by_article: dict[str, dict[str, Any]] = {}
    artifact_paths = (
        summary.get("artifact_paths") if isinstance(summary.get("artifact_paths"), dict) else {}
    )
    for row in rows:
        if not isinstance(row, dict):
            diagnostics.append(
                diagnostic(
                    "malformed_baseline_row",
                    "baseline article_result is not an object",
                    path=args.summary,
                )
            )
            continue
        article_ref = str(row.get("article_ref")) if row.get("article_ref") else ""
        variant_id = str(row.get("variant_id")) if row.get("variant_id") else ""
        variants_by_article[article_ref].add(variant_id)
        diagnostics.extend(false_flag_diagnostics(row, where=rel(args.summary), row=row))
        metrics = row.get("current_pipeline_metrics")
        if isinstance(metrics, dict):
            diagnostics.extend(
                false_flag_diagnostics(metrics, where="current_pipeline_metrics", row=row)
            )
            if (
                metrics.get("import_ready") is True
                or int(metrics.get("import_eligible_chunk_count") or 0) > 0
            ):
                diagnostics.append(
                    diagnostic(
                        "unsafe_import_ready_claim",
                        "baseline row claims import readiness/import-eligible chunks",
                        article_ref=article_ref,
                        variant_id=variant_id,
                    )
                )
        else:
            diagnostics.append(
                diagnostic(
                    "missing_current_pipeline_metrics",
                    "baseline row lacks current_pipeline_metrics",
                    article_ref=article_ref,
                    variant_id=variant_id,
                )
            )
        if row.get("conversion_status") not in TERMINAL_CONVERSION_STATUSES:
            diagnostics.append(
                diagnostic(
                    "non_terminal_conversion_status",
                    "baseline row has non-terminal S03 conversion status",
                    article_ref=article_ref,
                    variant_id=variant_id,
                )
            )
        baseline_codes = {
            str(d.get("diagnostic_code"))
            for d in jsonl_rows
            if d.get("article_ref") == article_ref and d.get("variant_id") == variant_id
        }
        if not (baseline_codes & TERMINAL_BASELINE_DIAGNOSTIC_CODES):
            diagnostics.append(
                diagnostic(
                    "terminal_baseline_status_missing",
                    "baseline row lacks terminal current-baseline diagnostic",
                    article_ref=article_ref,
                    variant_id=variant_id,
                )
            )
        diagnostics.extend(
            validate_converted_payload(
                row, conversion_rows.get((article_ref, variant_id)), root=args.root
            )
        )
        artifact_path_value = row.get("baseline_artifact_path") or artifact_paths.get(article_ref)  # ty:ignore[unresolved-attribute]
        artifact, artifact_findings = load_artifact(
            artifact_path_value, root=args.root, article_ref=article_ref
        )
        diagnostics.extend(artifact_findings)
        if artifact is not None:
            artifacts_by_article[article_ref] = artifact

    for article_ref, variants in sorted(variants_by_article.items()):
        artifact = artifacts_by_article.get(article_ref)
        if artifact is None:
            continue
        artifact_variants = artifact.get("variants")
        if not isinstance(artifact_variants, list):
            diagnostics.append(
                diagnostic(
                    "missing_artifact_variants",
                    "per-article baseline artifact lacks variants list",
                    article_ref=article_ref,
                )
            )
            continue
        artifact_variant_ids = {
            str(row.get("variant_id"))
            for row in artifact_variants
            if isinstance(row, dict) and row.get("variant_id")
        }
        if artifact_variant_ids != variants:
            diagnostics.append(
                diagnostic(
                    "baseline_artifact_variant_mismatch",
                    "per-article baseline artifact variants do not match summary rows",
                    article_ref=article_ref,
                )
            )
        if artifact.get("variant_count") != len(artifact_variants):
            diagnostics.append(
                diagnostic(
                    "artifact_variant_count_mismatch",
                    "per-article artifact variant_count does not match variants list",
                    article_ref=article_ref,
                )
            )

    verifier_summary = {
        "schema_version": VERIFIER_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
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
    parser.add_argument("--summary", type=Path, default=SUMMARY_PATH)
    parser.add_argument("--diagnostics", type=Path, default=DIAGNOSTICS_PATH)
    parser.add_argument("--report", type=Path, default=REPORT_PATH)
    parser.add_argument("--output-dir", type=Path, default=OUTPUT_DIR)
    parser.add_argument("--conversion-summary", type=Path, default=CONVERSION_SUMMARY_PATH)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--expected-article-count", type=int, default=EXPECTED_ARTICLE_COUNT)
    parser.add_argument("--expected-variant-count", type=int, default=EXPECTED_VARIANT_COUNT)
    return parser.parse_args(argv[1:])


def main(argv: list[str] | None = None) -> int:
    argv = argv or sys.argv
    args = parse_args(argv)
    try:
        verifier_summary, diagnostics = verify(args)
    except Exception as exc:
        sys.stderr.write(f"M027 current pipeline baseline verifier failed: {exc}\n")
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
