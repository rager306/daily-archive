#!/usr/bin/env python3
"""Validate-only verifier for the M027 S05 end-to-end mixed-source replay.

The verifier reads already-generated S05 replay artifacts and validates their
provenance, redaction, fail-closed safety flags, dependency hashes, per-article
artifacts, baseline-comparison coverage, readiness blockers, and output artifact
hashes. It never reruns conversion, loader, parser, PageIndex, chunking,
evidence generation, graph import, network fetches, or production writes.
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
SLICE_ID = "S05"
SOURCE_SLICE_ID = "S03"
BASELINE_SLICE_ID = "S04"
SELECTION_ID = "m027-mixed-source-corpus-v1"
SCHEMA_VERSION = "m027-end-to-end-mixed-replay.v1"
DIAGNOSTIC_SCHEMA_VERSION = "m027-end-to-end-mixed-replay-diagnostic.v1"
ARTIFACT_SCHEMA_VERSION = "m027-end-to-end-mixed-replay-artifact.v1"
DECISION_SCHEMA_VERSION = "m027-end-to-end-mixed-replay-readiness-decision.v1"
VERIFIER_SCHEMA_VERSION = "m027-end-to-end-mixed-replay-verifier.v1"
ROOT = Path(__file__).resolve().parents[1]
CORPUS_DIR = ROOT / "data" / "article_corpora" / SELECTION_ID
SUMMARY_PATH = CORPUS_DIR / "end-to-end-mixed-replay-summary.json"
DIAGNOSTICS_PATH = CORPUS_DIR / "end-to-end-mixed-replay-diagnostics.jsonl"
EVENTS_PATH = CORPUS_DIR / "end-to-end-mixed-replay-events.jsonl"
REPORT_PATH = CORPUS_DIR / "end-to-end-mixed-replay-report.md"
READINESS_DECISION_PATH = CORPUS_DIR / "end-to-end-mixed-replay-readiness-decision.json"
OUTPUT_DIR = CORPUS_DIR / "end-to-end-mixed-replay"
VERIFICATION_PATH = CORPUS_DIR / "end-to-end-mixed-replay-verification.json"
VERIFICATION_REPORT_PATH = CORPUS_DIR / "end-to-end-mixed-replay-verification-report.md"
CONVERSION_SUMMARY_PATH = CORPUS_DIR / "conversion-quality-summary.json"
BASELINE_SUMMARY_PATH = CORPUS_DIR / "current-pipeline-baseline-summary.json"
BASELINE_DIAGNOSTICS_PATH = CORPUS_DIR / "current-pipeline-baseline-diagnostics.jsonl"

EXPECTED_ARTICLE_COUNT = 6
EXPECTED_VARIANT_COUNT = 11
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
    "import_ready_claim",
    "ready_for_import",
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
    "article_text",
    "paper_text",
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
REQUIRED_DIAGNOSTIC_CODES = {
    "s03_linkage_verified",
    "s04_baseline_summary_loaded",
    "converted_payload_hash_verified",
    "end_to_end_boundaries_completed",
    "s04_baseline_exact_match",
}
TERMINAL_REPLAY_CODES = {
    "end_to_end_boundaries_completed",
    "parser_ready_zero_chunks_preserved",
    "metadata_only_not_parser_ready_skipped",
}


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
        "baseline_slice_id": BASELINE_SLICE_ID,
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


def load_json(path: Path) -> tuple[dict[str, Any] | None, list[dict[str, Any]]]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return None, [
            diagnostic("missing_json_artifact", "required JSON artifact is missing", path=path)
        ]
    except json.JSONDecodeError as exc:
        return None, [
            diagnostic(
                "malformed_json_artifact", f"required JSON artifact is malformed: {exc}", path=path
            )
        ]
    except OSError as exc:
        return None, [
            diagnostic(
                "json_artifact_unreadable",
                f"required JSON artifact is unreadable: {exc}",
                path=path,
            )
        ]
    if not isinstance(value, dict):
        return None, [
            diagnostic(
                "json_artifact_not_object", "required JSON artifact must be an object", path=path
            )
        ]
    return value, []


def load_jsonl(path: Path) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    rows: list[dict[str, Any]] = []
    findings: list[dict[str, Any]] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError:
        return rows, [
            diagnostic("missing_jsonl_artifact", "required JSONL artifact is missing", path=path)
        ]
    except OSError as exc:
        return rows, [
            diagnostic(
                "jsonl_artifact_unreadable",
                f"required JSONL artifact is unreadable: {exc}",
                path=path,
            )
        ]
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as exc:
            findings.append(
                diagnostic(
                    "malformed_diagnostics_jsonl",
                    f"JSONL row is malformed: {exc}",
                    path=path,
                    json_path=f"$[{line_number}]",
                )
            )
            continue
        if not isinstance(value, dict):
            findings.append(
                diagnostic(
                    "malformed_diagnostics_jsonl",
                    "JSONL row is not an object",
                    path=path,
                    json_path=f"$[{line_number}]",
                )
            )
            continue
        rows.append(value)
    return rows, findings


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


def baseline_index(baseline_summary: Mapping[str, Any]) -> dict[tuple[str, str], dict[str, Any]]:
    rows = baseline_summary.get("article_results")
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


def validate_input_artifact_hashes(
    summary: Mapping[str, Any], *, root: Path
) -> tuple[dict[str, Any], dict[str, Any], list[dict[str, Any]]]:
    findings: list[dict[str, Any]] = []
    conversion_summary: dict[str, Any] = {}
    baseline_summary: dict[str, Any] = {}
    input_specs = [
        (
            "conversion_summary_path",
            "conversion_summary_sha256",
            "s03_conversion_summary",
            "conversion_summary",
        ),
        (
            "baseline_summary_path",
            "baseline_summary_sha256",
            "s04_baseline_summary",
            "baseline_summary",
        ),
        (
            "baseline_diagnostics_path",
            "baseline_diagnostics_sha256",
            "s04_baseline_diagnostics",
            "baseline_diagnostics",
        ),
    ]
    for path_key, hash_key, role, label in input_specs:
        try:
            path = safe_under_root(root, summary.get(path_key), code_label=path_key)
        except ValueError as exc:
            findings.append(
                diagnostic(
                    f"unsafe_{path_key}:{exc}",
                    f"unsafe input artifact path: {exc}",
                    json_path=f"$.{path_key}",
                )
            )
            continue
        if not path.exists() or not path.is_file():
            findings.append(
                diagnostic(
                    "missing_input_artifact",
                    f"input artifact is missing: {role}",
                    path=path,
                    json_path=f"$.{path_key}",
                )
            )
            continue
        actual_hash = sha256_file(path)
        if summary.get(hash_key) != actual_hash:
            findings.append(
                diagnostic(
                    f"{label}_sha256_mismatch",
                    f"{role} hash is stale",
                    path=path,
                    json_path=f"$.{hash_key}",
                )
            )
        if role == "s03_conversion_summary":
            loaded, load_findings = load_json(path)
            findings.extend(load_findings)
            conversion_summary = loaded or {}
        elif role == "s04_baseline_summary":
            loaded, load_findings = load_json(path)
            findings.extend(load_findings)
            baseline_summary = loaded or {}
    if conversion_summary:
        try:
            source_path = safe_under_root(
                root,
                conversion_summary.get("source_summary_path"),
                code_label="source_summary_path",
            )
            if conversion_summary.get("source_summary_sha256") != sha256_file(source_path):
                findings.append(
                    diagnostic(
                        "s03_source_summary_sha256_mismatch",
                        "S03 source-summary linkage hash is stale",
                        path=source_path,
                        json_path="$.source_summary_sha256",
                    )
                )
        except Exception as exc:
            findings.append(
                diagnostic(
                    "s03_source_linkage_unverifiable",
                    f"S03 source-summary linkage cannot be verified: {exc}",
                )
            )
    return conversion_summary, baseline_summary, findings


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
                "replay row lacks converted payload provenance",
                article_ref=article_ref,
                variant_id=variant_id,
            )
        ]
    if record.get("parser_ready") is not True:
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
                "conversion_row_missing_for_replay",
                "replay row has no matching S03 conversion row",
                article_ref=article_ref,
                variant_id=variant_id,
            )
        )
    else:
        if conversion_row.get("converted_text_sha256") != payload.get("sha256"):
            findings.append(
                diagnostic(
                    "replay_conversion_hash_mismatch",
                    "replay converted payload hash differs from S03 conversion row",
                    article_ref=article_ref,
                    variant_id=variant_id,
                )
            )
        if conversion_row.get("converted_text_byte_size") != payload.get("byte_size"):
            findings.append(
                diagnostic(
                    "replay_conversion_size_mismatch",
                    "replay converted payload byte size differs from S03 conversion row",
                    article_ref=article_ref,
                    variant_id=variant_id,
                )
            )
    return findings


def load_replay_artifact(
    path_value: Any, *, root: Path, article_ref: str
) -> tuple[dict[str, Any] | None, list[dict[str, Any]]]:
    try:
        artifact_path = safe_under_root(root, path_value, code_label="replay_artifact_path")
    except ValueError as exc:
        return None, [
            diagnostic(
                f"unsafe_replay_artifact_path:{exc}",
                f"unsafe replay artifact path: {exc}",
                article_ref=article_ref,
            )
        ]
    if not artifact_path.exists() or not artifact_path.is_file():
        return None, [
            diagnostic(
                "missing_per_article_replay_artifact",
                "per-article replay artifact is missing",
                path=artifact_path,
                article_ref=article_ref,
            )
        ]
    artifact, load_findings = load_json(artifact_path)
    if artifact is None:
        return None, [
            diagnostic(
                "malformed_per_article_replay_artifact",
                "per-article replay artifact is malformed",
                path=artifact_path,
                article_ref=article_ref,
            ),
            *load_findings,
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
    for heading in [
        "Decision",
        "Aggregate Summary",
        "Article Results",
        "Diagnostics",
        "Provenance",
        "Failure Modes",
        "Load Profile",
        "Negative Tests",
    ]:
        if heading not in report_text:
            findings.append(
                diagnostic(
                    "report_section_missing", f"replay report missing {heading} section", path=path
                )
            )
    lowered = report_text.lower()
    for claim in [
        "graph readiness claim: **true**",
        "trusted fact claim: **true**",
        "ready for import: **true**",
        "import ready: **true**",
    ]:
        if claim in lowered:
            findings.append(
                diagnostic(
                    "unsupported_readiness_claim",
                    f"replay report contains unsupported readiness/import claim: {claim}",
                    path=path,
                )
            )
    return findings


def validate_output_artifacts(summary: Mapping[str, Any], *, root: Path) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    rows = summary.get("output_artifacts")
    if not isinstance(rows, list) or not rows:
        return [
            diagnostic(
                "missing_output_artifact_provenance",
                "summary lacks output_artifacts provenance",
                json_path="$.output_artifacts",
            )
        ]
    required_roles = {
        "summary",
        "diagnostics",
        "events",
        "report",
        "readiness_decision",
        "per_article_replay",
    }
    roles = {str(row.get("role")) for row in rows if isinstance(row, dict)}
    missing_roles = required_roles - roles
    if missing_roles:
        findings.append(
            diagnostic(
                "output_artifact_role_missing",
                f"summary output_artifacts missing roles: {sorted(missing_roles)}",
                json_path="$.output_artifacts",
            )
        )
    for index, row in enumerate(rows):
        if not isinstance(row, dict):
            findings.append(
                diagnostic(
                    "malformed_output_artifact_row",
                    "output_artifacts row is not an object",
                    json_path=f"$.output_artifacts[{index}]",
                )
            )
            continue
        try:
            path = safe_under_root(root, row.get("path"), code_label="output_artifact_path")
        except ValueError as exc:
            findings.append(
                diagnostic(
                    f"unsafe_output_artifact_path:{exc}",
                    f"unsafe output artifact path: {exc}",
                    json_path=f"$.output_artifacts[{index}].path",
                )
            )
            continue
        if not path.exists() or not path.is_file():
            findings.append(
                diagnostic(
                    "missing_output_artifact",
                    "output artifact is missing",
                    path=path,
                    json_path=f"$.output_artifacts[{index}].path",
                )
            )
            continue
        if row.get("exists") is not True:
            findings.append(
                diagnostic(
                    "output_artifact_exists_flag_mismatch",
                    "output artifact exists flag is not true",
                    path=path,
                    json_path=f"$.output_artifacts[{index}].exists",
                )
            )
        # The summary row is self-referential: finalizing output provenance necessarily
        # rewrites the summary after computing the pre-final summary artifact row. All
        # other output sizes and hashes are directly recomputable validate-only provenance.
        if row.get("role") == "summary":
            continue
        if row.get("byte_size") != path.stat().st_size:
            findings.append(
                diagnostic(
                    "output_artifact_byte_size_mismatch",
                    "output artifact byte size is stale",
                    path=path,
                    json_path=f"$.output_artifacts[{index}].byte_size",
                )
            )
        if row.get("sha256") != sha256_file(path):
            findings.append(
                diagnostic(
                    "output_artifact_sha256_mismatch",
                    "output artifact hash is stale",
                    path=path,
                    json_path=f"$.output_artifacts[{index}].sha256",
                )
            )
    return findings


def verify(args: argparse.Namespace) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    root = Path(args.root)
    diagnostics: list[dict[str, Any]] = []
    summary, findings = load_json(args.summary)
    diagnostics.extend(findings)
    jsonl_rows, findings = load_jsonl(args.diagnostics)
    diagnostics.extend(findings)
    events, findings = load_jsonl(args.events)
    diagnostics.extend(findings)
    decision, findings = load_json(args.readiness_decision)
    diagnostics.extend(findings)
    try:
        report_text = args.report.read_text(encoding="utf-8")
    except FileNotFoundError:
        report_text = ""
        diagnostics.append(
            diagnostic(
                "missing_report_artifact", "replay markdown report is missing", path=args.report
            )
        )
    except OSError as exc:
        report_text = ""
        diagnostics.append(
            diagnostic(
                "report_artifact_unreadable",
                f"replay markdown report is unreadable: {exc}",
                path=args.report,
            )
        )

    if summary is None:
        verifier_summary = {
            "schema_version": VERIFIER_SCHEMA_VERSION,
            "milestone_id": MILESTONE_ID,
            "slice_id": SLICE_ID,
            "status": "failed",
            "diagnostic_count": len(diagnostics),
            "network_fetch_attempted": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
            "graph_import_allowed": False,
        }
        return verifier_summary, diagnostics

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
            events, serialized=json.dumps(events, sort_keys=True), where=rel(args.events)
        )
    )
    if decision is not None:
        diagnostics.extend(
            validate_no_payload_leakage(
                decision,
                serialized=json.dumps(decision, sort_keys=True),
                where=rel(args.readiness_decision),
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
        "baseline_slice_id": BASELINE_SLICE_ID,
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
        if (
            readiness.get("end_to_end_replay_completed") is not True
            or readiness.get("validate_only") is not True
        ):
            diagnostics.append(
                diagnostic(
                    "readiness_contract_mismatch",
                    "summary readiness must be completed validate-only",
                    path=args.summary,
                    json_path="$.readiness",
                )
            )
    else:
        diagnostics.append(
            diagnostic(
                "missing_readiness_summary",
                "summary lacks readiness object",
                path=args.summary,
                json_path="$.readiness",
            )
        )

    if decision is None:
        diagnostics.append(
            diagnostic(
                "missing_readiness_decision",
                "readiness decision JSON is missing or malformed",
                path=args.readiness_decision,
            )
        )
    else:
        diagnostics.extend(false_flag_diagnostics(decision, where=rel(args.readiness_decision)))
        if (
            decision.get("schema_version") != DECISION_SCHEMA_VERSION
            or decision.get("ready_for_import") is not False
        ):
            diagnostics.append(
                diagnostic(
                    "readiness_decision_contract_mismatch",
                    "readiness decision must be S05 validate-only and not import-ready",
                    path=args.readiness_decision,
                )
            )
        expected_blockers: set[str] = set()
        if int(summary.get("baseline_missing_count") or 0) > 0:
            expected_blockers.add("s04_baseline_rows_missing")
        if int(summary.get("zero_chunk_parser_ready_variant_count") or 0) > 0:
            expected_blockers.add("parser_ready_zero_chunk_variants_preserved")
        if int(summary.get("import_ready_count") or 0) > 0:
            expected_blockers.add("unexpected_import_ready_records")
        actual_blockers = (
            set(decision.get("blockers") or [])
            if isinstance(decision.get("blockers"), list)
            else set()
        )
        if not expected_blockers.issubset(actual_blockers):
            diagnostics.append(
                diagnostic(
                    "readiness_blockers_mismatch",
                    "readiness decision blockers do not cover summary blockers",
                    path=args.readiness_decision,
                    json_path="$.blockers",
                )
            )

    conversion_summary, baseline_summary, findings = validate_input_artifact_hashes(
        summary, root=root
    )
    diagnostics.extend(findings)
    conversion_rows = conversion_index(conversion_summary)
    baseline_rows = baseline_index(baseline_summary)

    diagnostic_counts = Counter(
        str(row.get("diagnostic_code")) for row in jsonl_rows if isinstance(row, dict)
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
                f"replay diagnostics missing required codes: {sorted(missing_codes)}",
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

    if not events:
        diagnostics.append(
            diagnostic(
                "missing_replay_events", "events JSONL has no replay events", path=args.events
            )
        )
    else:
        if (
            events[0].get("event_type") != "replay_started"
            or events[-1].get("event_type") != "replay_completed"
        ):
            diagnostics.append(
                diagnostic(
                    "event_sequence_mismatch",
                    "events must start with replay_started and end with replay_completed",
                    path=args.events,
                )
            )
        for index, row in enumerate(events):
            diagnostics.extend(false_flag_diagnostics(row, where=rel(args.events), row=row))
            if row.get("schema_version") != DIAGNOSTIC_SCHEMA_VERSION:
                diagnostics.append(
                    diagnostic(
                        "event_contract_mismatch",
                        "event JSONL row has unexpected schema_version",
                        path=args.events,
                        json_path=f"$[{index}].schema_version",
                    )
                )

    rows = summary.get("article_results")
    if not isinstance(rows, list):
        rows = []
        diagnostics.append(
            diagnostic(
                "missing_replay_results",
                "summary is missing article_results list",
                path=args.summary,
                json_path="$.article_results",
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
                f"expected {args.expected_variant_count} replay variants",
                path=args.summary,
                json_path="$.variant_count",
            )
        )
    parser_ready_count = sum(
        1 for row in rows if isinstance(row, dict) and row.get("parser_ready") is True
    )
    metadata_only_count = len(rows) - parser_ready_count
    if summary.get("parser_ready_variant_count") != parser_ready_count:
        diagnostics.append(
            diagnostic(
                "parser_ready_variant_count_mismatch",
                "summary parser-ready count does not match rows",
                path=args.summary,
                json_path="$.parser_ready_variant_count",
            )
        )
    if summary.get("metadata_only_variant_count") != metadata_only_count:
        diagnostics.append(
            diagnostic(
                "metadata_only_variant_count_mismatch",
                "summary metadata-only count does not match rows",
                path=args.summary,
                json_path="$.metadata_only_variant_count",
            )
        )
    comparison_counts = Counter(
        str(row.get("baseline_comparison", {}).get("category"))
        for row in rows
        if isinstance(row, dict) and isinstance(row.get("baseline_comparison"), dict)
    )
    if dict(sorted(comparison_counts.items())) != summary.get("baseline_comparison_counts"):
        diagnostics.append(
            diagnostic(
                "baseline_comparison_counts_mismatch",
                "summary baseline comparison counts do not match rows",
                path=args.summary,
                json_path="$.baseline_comparison_counts",
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
                    "malformed_replay_row",
                    "replay article_result is not an object",
                    path=args.summary,
                )
            )
            continue
        article_ref = str(row.get("article_ref")) if row.get("article_ref") else ""
        variant_id = str(row.get("variant_id")) if row.get("variant_id") else ""
        variants_by_article[article_ref].add(variant_id)
        diagnostics.extend(false_flag_diagnostics(row, where=rel(args.summary), row=row))
        diagnostics.extend(
            validate_converted_payload(
                row, conversion_rows.get((article_ref, variant_id)), root=root
            )
        )
        if (article_ref, variant_id) not in baseline_rows:
            diagnostics.append(
                diagnostic(
                    "s04_baseline_row_missing",
                    "replay row has no matching S04 baseline row",
                    article_ref=article_ref,
                    variant_id=variant_id,
                )
            )
        comparison = row.get("baseline_comparison")
        if not isinstance(comparison, dict):
            diagnostics.append(
                diagnostic(
                    "missing_baseline_comparison",
                    "replay row lacks baseline_comparison",
                    article_ref=article_ref,
                    variant_id=variant_id,
                )
            )
        elif comparison.get("category") == "baseline_missing":
            diagnostics.append(
                diagnostic(
                    "baseline_comparison_missing_row",
                    "replay preserved missing baseline row blocker",
                    article_ref=article_ref,
                    variant_id=variant_id,
                )
            )
        metrics = row.get("boundary_metrics")
        if not isinstance(metrics, dict):
            diagnostics.append(
                diagnostic(
                    "missing_boundary_metrics",
                    "replay row lacks boundary metrics",
                    article_ref=article_ref,
                    variant_id=variant_id,
                )
            )
        else:
            diagnostics.extend(false_flag_diagnostics(metrics, where="boundary_metrics", row=row))
            chunking = metrics.get("chunking") if isinstance(metrics.get("chunking"), dict) else {}
            evidence = metrics.get("evidence") if isinstance(metrics.get("evidence"), dict) else {}
            contract = (
                metrics.get("import_contract")
                if isinstance(metrics.get("import_contract"), dict)
                else {}
            )
            if row.get("parser_ready") is True:
                for key in [
                    "loader",
                    "parser",
                    "page_index",
                    "chunking",
                    "evidence",
                    "import_contract",
                ]:
                    if not isinstance(metrics.get(key), dict):
                        diagnostics.append(
                            diagnostic(
                                "missing_boundary_metric_key",
                                f"parser-ready row lacks boundary metric key: {key}",
                                article_ref=article_ref,
                                variant_id=variant_id,
                            )
                        )
                if int(chunking.get("chunk_count") or 0) == 0 and not any(
                    d.get("diagnostic_code") == "parser_ready_zero_chunks_preserved"
                    and d.get("variant_id") == variant_id
                    for d in jsonl_rows
                ):
                    diagnostics.append(
                        diagnostic(
                            "zero_chunk_parser_ready_diagnostic_missing",
                            "zero-chunk parser-ready row lacks preservation diagnostic",
                            article_ref=article_ref,
                            variant_id=variant_id,
                        )
                    )
                if int(evidence.get("evidence_path_count") or 0) < 0:
                    diagnostics.append(
                        diagnostic(
                            "evidence_count_invalid",
                            "evidence path count cannot be negative",
                            article_ref=article_ref,
                            variant_id=variant_id,
                        )
                    )
            if (
                contract.get("import_ready") is True
                or int(contract.get("import_eligible_chunk_count") or 0) > 0
            ):
                diagnostics.append(
                    diagnostic(
                        "unsafe_import_ready_claim",
                        "replay row claims import readiness/import-eligible chunks",
                        article_ref=article_ref,
                        variant_id=variant_id,
                    )
                )
        replay_codes = {
            str(d.get("diagnostic_code"))
            for d in jsonl_rows
            if d.get("article_ref") == article_ref and d.get("variant_id") == variant_id
        }
        if not (replay_codes & TERMINAL_REPLAY_CODES):
            diagnostics.append(
                diagnostic(
                    "terminal_replay_status_missing",
                    "replay row lacks terminal boundary diagnostic",
                    article_ref=article_ref,
                    variant_id=variant_id,
                )
            )
        artifact_path_value = row.get("replay_artifact_path") or artifact_paths.get(article_ref)
        artifact, artifact_findings = load_replay_artifact(
            artifact_path_value, root=root, article_ref=article_ref
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
                    "per-article replay artifact lacks variants list",
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
                    "replay_artifact_variant_mismatch",
                    "per-article replay artifact variants do not match summary rows",
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

    diagnostics.extend(validate_output_artifacts(summary, root=root))

    verifier_summary = {
        "schema_version": VERIFIER_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "baseline_slice_id": BASELINE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "passed" if not diagnostics else "failed",
        "diagnostic_count": len(diagnostics),
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_import_allowed": False,
        "cwd": str(Path.cwd()),
        "command": " ".join(sys.argv),
        "exit_status": 0 if not diagnostics else 1,
        "verification_result": "passed" if not diagnostics else "failed",
    }
    return verifier_summary, diagnostics


def write_verification_artifacts(
    args: argparse.Namespace, verifier_summary: dict[str, Any], diagnostics: list[dict[str, Any]]
) -> None:
    """Persist redacted validate-only closeout metadata and a markdown report."""
    verification_path = safe_under_root(
        args.root, str(args.verification_output), code_label="verification_output"
    )
    report_path = safe_under_root(
        args.root, str(args.verification_report), code_label="verification_report"
    )
    payload = {
        **verifier_summary,
        "diagnostics": diagnostics,
        "diagnostic_codes": sorted(
            {str(row.get("diagnostic_code")) for row in diagnostics if row.get("diagnostic_code")}
        ),
        "artifact_paths": {
            "summary": str(args.summary),
            "diagnostics": str(args.diagnostics),
            "events": str(args.events),
            "report": str(args.report),
            "readiness_decision": str(args.readiness_decision),
            "output_dir": str(args.output_dir),
            "conversion_summary": str(args.conversion_summary),
            "baseline_summary": str(args.baseline_summary),
            "baseline_diagnostics": str(args.baseline_diagnostics),
        },
    }
    leakage_findings = validate_no_payload_leakage(
        payload, serialized=json.dumps(payload, sort_keys=True), where=rel(verification_path)
    )
    if leakage_findings:
        raise RuntimeError(
            f"verification artifact leakage detected: {[row.get('diagnostic_code') for row in leakage_findings]}"
        )
    verification_path.parent.mkdir(parents=True, exist_ok=True)
    verification_path.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    report_lines = [
        "# M027 S05 End to End Mixed Replay Verification",
        "",
        f"- status: `{verifier_summary['status']}`",
        f"- diagnostic_count: `{verifier_summary['diagnostic_count']}`",
        f"- milestone_id: `{MILESTONE_ID}`",
        f"- slice_id: `{SLICE_ID}`",
        f"- source_slice_id: `{SOURCE_SLICE_ID}`",
        f"- baseline_slice_id: `{BASELINE_SLICE_ID}`",
        f"- network_fetch_attempted: `{verifier_summary['network_fetch_attempted']}`",
        f"- production_import_attempted: `{verifier_summary['production_import_attempted']}`",
        f"- graph_import_allowed: `{verifier_summary['graph_import_allowed']}`",
        f"- ladybugdb_written: `{verifier_summary['ladybugdb_written']}`",
        f"- exit_status: `{verifier_summary['exit_status']}`",
        "",
        "## Diagnostics",
    ]
    if diagnostics:
        report_lines.extend(
            f"- `{row.get('diagnostic_code')}` at `{row.get('json_path', '$')}`: {row.get('message', '')}"
            for row in diagnostics
        )
    else:
        report_lines.append("- None.")
    report = "\n".join(report_lines) + "\n"
    leakage_findings = validate_no_payload_leakage(
        {"report": report}, serialized=report, where=rel(report_path)
    )
    if leakage_findings:
        raise RuntimeError(
            f"verification report leakage detected: {[row.get('diagnostic_code') for row in leakage_findings]}"
        )
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(report, encoding="utf-8")


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--summary", type=Path, default=SUMMARY_PATH)
    parser.add_argument("--diagnostics", type=Path, default=DIAGNOSTICS_PATH)
    parser.add_argument("--events", type=Path, default=EVENTS_PATH)
    parser.add_argument("--report", type=Path, default=REPORT_PATH)
    parser.add_argument("--readiness-decision", type=Path, default=READINESS_DECISION_PATH)
    parser.add_argument("--output-dir", type=Path, default=OUTPUT_DIR)
    parser.add_argument("--conversion-summary", type=Path, default=CONVERSION_SUMMARY_PATH)
    parser.add_argument("--baseline-summary", type=Path, default=BASELINE_SUMMARY_PATH)
    parser.add_argument("--baseline-diagnostics", type=Path, default=BASELINE_DIAGNOSTICS_PATH)
    parser.add_argument("--verification-output", type=Path, default=VERIFICATION_PATH)
    parser.add_argument("--verification-report", type=Path, default=VERIFICATION_REPORT_PATH)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--expected-article-count", type=int, default=EXPECTED_ARTICLE_COUNT)
    parser.add_argument("--expected-variant-count", type=int, default=EXPECTED_VARIANT_COUNT)
    return parser.parse_args(argv[1:])


def main(argv: list[str] | None = None) -> int:
    argv = argv or sys.argv
    args = parse_args(argv)
    verifier_summary, diagnostics = verify(args)
    write_verification_artifacts(args, verifier_summary, diagnostics)
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
