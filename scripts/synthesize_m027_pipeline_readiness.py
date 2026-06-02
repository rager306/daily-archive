#!/usr/bin/env python3
"""Generate or validate M027 S07 pipeline readiness synthesis artifacts.

The synthesis is metadata-only and fail-closed. It reads completed S01-S06 local
artifacts, hashes them, validates safety/readiness boundaries, and writes a
fresh-reader-friendly report. It never fetches network content, imports graph
facts, writes LadybugDB, or regenerates upstream slice artifacts.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections import Counter
from collections.abc import Mapping, Sequence
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

MILESTONE_ID = "M027-aakeky"
SLICE_ID = "S07"
SOURCE_SLICE_IDS = ("S01", "S02", "S03", "S04", "S05", "S06")
SELECTION_ID = "m027-mixed-source-corpus-v1"
SCHEMA_VERSION = "m027-pipeline-readiness-synthesis.v1"
DIAGNOSTIC_SCHEMA_VERSION = "m027-pipeline-readiness-synthesis-diagnostic.v1"
ROOT = Path(__file__).resolve().parents[1]
CORPUS_DIR = ROOT / "data" / "article_corpora" / SELECTION_ID
OUTPUT_DIR = CORPUS_DIR / "pipeline-readiness-synthesis"
SUMMARY_PATH = OUTPUT_DIR / "pipeline-readiness-synthesis-summary.json"
DIAGNOSTICS_PATH = OUTPUT_DIR / "pipeline-readiness-synthesis-diagnostics.jsonl"
REPORT_PATH = OUTPUT_DIR / "pipeline-readiness-synthesis-report.md"

INPUT_ARTIFACTS: tuple[tuple[str, Path, str], ...] = (
    ("s01_catalog_summary", CORPUS_DIR / "catalog-summary.json", "S01 catalog selection and loader boundary summary"),
    ("s02_source_acquisition_summary", CORPUS_DIR / "source-acquisition-summary.json", "S02 local source acquisition summary"),
    ("s03_conversion_quality_summary", CORPUS_DIR / "conversion-quality-summary.json", "S03 conversion quality summary"),
    ("s04_current_pipeline_baseline_summary", CORPUS_DIR / "current-pipeline-baseline-summary.json", "S04 current pipeline baseline summary"),
    ("s05_end_to_end_replay_summary", CORPUS_DIR / "end-to-end-mixed-replay-summary.json", "S05 end-to-end mixed replay summary"),
    ("s05_end_to_end_replay_verification", CORPUS_DIR / "end-to-end-mixed-replay-verification.json", "S05 validate-only verification result"),
    ("s05_readiness_decision", CORPUS_DIR / "end-to-end-mixed-replay-readiness-decision.json", "S05 fail-closed readiness decision"),
    ("s06_riskratchet_summary", CORPUS_DIR / "provenance-riskratchet-gate" / "provenance-riskratchet-gate-summary.json", "S06 provenance/riskratchet gate summary"),
    ("s06_riskratchet_diagnostics", CORPUS_DIR / "provenance-riskratchet-gate" / "provenance-riskratchet-gate-diagnostics.jsonl", "S06 provenance/riskratchet diagnostics"),
    ("s06_maintainability_diagnostic", CORPUS_DIR / "provenance-riskratchet-gate" / "maintainability-diagnostic.json", "S06 diagnostic-only maintainability telemetry"),
    ("s05_verifier_script", ROOT / "scripts" / "verify_m027_end_to_end_mixed_replay.py", "S05 verifier implementation used as contract reference"),
    ("s06_verifier_script", ROOT / "scripts" / "verify_m027_provenance_and_riskratchet_gate.py", "S06 verifier implementation used as contract reference"),
)

FALSE_SAFETY_FLAGS: dict[str, bool] = {
    "network_fetch_attempted": False,
    "production_import_attempted": False,
    "graph_import_allowed": False,
    "trusted_kg_import_allowed": False,
    "production_ladybugdb_write_allowed": False,
    "ladybugdb_written": False,
    "raw_text_embedded_in_metadata": False,
    "raw_binary_embedded_in_metadata": False,
    "raw_payload_embedded_in_metadata": False,
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
    "ready_for_graph_import",
    "production_ready",
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
FORBIDDEN_SNIPPETS = ("<html", "</html", "%PDF-", "base64,", "RAW_ARXIV_ABS_SECRET", "RAW_NATURE_BODY_SECRET", "RAW_PDF_SECRET")
REQUIRED_SUMMARY_CONTRACTS = {
    "s01_catalog_summary": ("S01", None),
    "s02_source_acquisition_summary": ("S02", "m027-source-acquisition.v1"),
    "s03_conversion_quality_summary": ("S03", "m027-conversion-quality.v1"),
    "s04_current_pipeline_baseline_summary": ("S04", "m027-current-pipeline-baseline.v1"),
    "s05_end_to_end_replay_summary": ("S05", "m027-end-to-end-mixed-replay.v1"),
    "s05_end_to_end_replay_verification": ("S05", "m027-end-to-end-mixed-replay-verifier.v1"),
    "s05_readiness_decision": ("S05", "m027-end-to-end-mixed-replay-readiness-decision.v1"),
    "s06_riskratchet_summary": ("S06", "m027-provenance-riskratchet-gate.v1"),
}


def utc_now() -> str:
    return datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


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


def git_commit_from_head(root: Path) -> str:
    head_path = root / ".git" / "HEAD"
    try:
        head = head_path.read_text(encoding="utf-8").strip()
    except OSError:
        return "unknown-unavailable"
    if head.startswith("ref: "):
        ref_path = root / ".git" / head.removeprefix("ref: ").strip()
        try:
            return ref_path.read_text(encoding="utf-8").strip() or "unknown-unavailable"
        except OSError:
            return "unknown-unavailable"
    return head or "unknown-unavailable"


def safe_relative_path(value: Any, *, label: str) -> PurePosixPath:
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"missing_{label}")
    if "://" in value:
        raise ValueError(f"url_not_allowed_as_{label}")
    normalized = PurePosixPath(value.replace("\\", "/"))
    if normalized.is_absolute() or ".." in normalized.parts or any(part == "" for part in normalized.parts):
        raise ValueError(f"unsafe_{label}")
    return normalized


def safe_under_root(root: Path, value: Any, *, label: str) -> Path:
    root_resolved = root.resolve()
    if isinstance(value, str) and Path(value).is_absolute():
        resolved = Path(value).resolve()
        if not resolved.is_relative_to(root_resolved):
            raise ValueError(f"{label}_escapes_root")
        return resolved
    normalized = safe_relative_path(value, label=label)
    resolved = (root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root_resolved):
        raise ValueError(f"{label}_escapes_root")
    return resolved


def diagnostic(
    code: str,
    message: str,
    *,
    severity: str = "error",
    phase: str = "validation",
    artifact_path: Path | str | None = None,
    json_path: str = "$",
    failure_source_class: str = "artifact_contract",
) -> dict[str, Any]:
    path_value = rel(artifact_path) if isinstance(artifact_path, Path) else artifact_path
    return {
        "schema_version": DIAGNOSTIC_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "phase": phase,
        "severity": severity,
        "diagnostic_code": code,
        "code": code,
        "failure_source_class": failure_source_class,
        "failure_domain": failure_source_class,
        "artifact_path": path_value,
        "path": path_value,
        "json_path": json_path,
        "message": message,
        "recovery_guidance": recovery_guidance_for(code, failure_source_class),
        **FALSE_SAFETY_FLAGS,
    }


def recovery_guidance_for(code: str, failure_source_class: str) -> str:
    if "missing" in code:
        return "Regenerate or restore the referenced upstream slice artifact, then rerun S07 synthesis."
    if "malformed" in code:
        return "Inspect the referenced JSON/JSONL row and rerun the owning verifier before rerunning S07."
    if "hash" in code or "byte_size" in code:
        return "Rerun the owning validate-only verifier so artifact provenance matches current file bytes."
    if "unsafe" in code or failure_source_class == "safety_flags":
        return "Restore fail-closed local-only flags and remove graph/import/raw-payload claims before rerunning."
    if failure_source_class == "claim_boundary":
        return "Downgrade the readiness statement to preprocessing-only evidence or move graph/import readiness to a future validated slice."
    return "Inspect the diagnostic path and rerun the upstream owner before trusting the S07 report."


def load_json(path: Path) -> tuple[dict[str, Any] | None, list[dict[str, Any]]]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return None, [diagnostic("missing_json_artifact", "required JSON artifact is missing", artifact_path=path, failure_source_class="filesystem")]
    except json.JSONDecodeError as exc:
        return None, [diagnostic("malformed_json_artifact", f"required JSON artifact is malformed: {exc}", artifact_path=path, failure_source_class="json_contract")]
    except OSError as exc:
        return None, [diagnostic("json_artifact_unreadable", f"required JSON artifact is unreadable: {exc}", artifact_path=path, failure_source_class="filesystem")]
    if not isinstance(value, dict):
        return None, [diagnostic("json_artifact_not_object", "required JSON artifact must be a JSON object", artifact_path=path, failure_source_class="json_contract")]
    return value, []


def load_jsonl(path: Path) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    rows: list[dict[str, Any]] = []
    findings: list[dict[str, Any]] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError:
        return rows, [diagnostic("missing_jsonl_artifact", "required JSONL artifact is missing", artifact_path=path, failure_source_class="filesystem")]
    except OSError as exc:
        return rows, [diagnostic("jsonl_artifact_unreadable", f"required JSONL artifact is unreadable: {exc}", artifact_path=path, failure_source_class="filesystem")]
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as exc:
            findings.append(diagnostic("malformed_jsonl_artifact", f"JSONL row is malformed: {exc}", artifact_path=path, json_path=f"$[{line_number}]", failure_source_class="json_contract"))
            continue
        if not isinstance(value, dict):
            findings.append(diagnostic("malformed_jsonl_artifact", "JSONL row must be an object", artifact_path=path, json_path=f"$[{line_number}]", failure_source_class="json_contract"))
            continue
        rows.append(value)
    return rows, findings


def artifact_row(path: Path, *, role: str, description: str, root: Path) -> dict[str, Any]:
    exists = path.exists() and path.is_file()
    return {
        "role": role,
        "description": description,
        "path": rel(path, root),
        "exists": exists,
        "sha256": sha256_file(path) if exists else None,
        "byte_size": path.stat().st_size if exists else None,
    }


def validate_no_payload_leakage(value: Any, *, serialized: str, where: Path | str) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []

    def walk(node: Any, path: str) -> None:
        if isinstance(node, dict):
            for key, item in node.items():
                if key in FORBIDDEN_PAYLOAD_KEYS:
                    findings.append(diagnostic("metadata_payload_key_leakage", f"metadata artifact includes forbidden payload key {key!r}", artifact_path=where, json_path=f"{path}.{key}", failure_source_class="redaction"))
                walk(item, f"{path}.{key}")
        elif isinstance(node, list):
            for index, item in enumerate(node):
                walk(item, f"{path}[{index}]")

    walk(value, "$")
    lowered = serialized.lower()
    for snippet in FORBIDDEN_SNIPPETS:
        if snippet.lower() in lowered:
            findings.append(diagnostic("metadata_payload_snippet_leakage", "metadata artifact includes a forbidden raw payload sentinel", artifact_path=where, failure_source_class="redaction"))
    return findings


def false_flag_diagnostics(value: Mapping[str, Any], *, where: Path | str, json_prefix: str = "$") -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    for key in sorted(UNSAFE_TRUE_FLAGS):
        if value.get(key) is True:
            source = "claim_boundary" if "claim" in key or "ready" in key else "safety_flags"
            findings.append(diagnostic("unsafe_safety_or_readiness_flag_true", f"unsafe fail-closed/readiness flag is true: {key}", artifact_path=where, json_path=f"{json_prefix}.{key}", failure_source_class=source))
    return findings


def validate_path_fields(value: Any, *, where: Path | str, root: Path, path: str = "$") -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    if isinstance(value, dict):
        for key, item in value.items():
            child = f"{path}.{key}"
            if isinstance(item, str) and (key.endswith("_path") or key.endswith("_dir") or key in {"path", "json_report", "human_report"}):
                try:
                    safe_under_root(root, item, label=key)
                except ValueError as exc:
                    findings.append(diagnostic(f"unsafe_artifact_reference:{exc}", f"unsafe local artifact reference in {key}: {exc}", artifact_path=where, json_path=child, failure_source_class="path_safety"))
            findings.extend(validate_path_fields(item, where=where, root=root, path=child))
    elif isinstance(value, list):
        for index, item in enumerate(value):
            findings.extend(validate_path_fields(item, where=where, root=root, path=f"{path}[{index}]"))
    return findings


def validate_declared_artifact_rows(rows: Any, *, where: Path | str, root: Path, json_path: str) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    if not isinstance(rows, list):
        return findings
    for index, row in enumerate(rows):
        if not isinstance(row, dict) or "path" not in row:
            continue
        row_path = f"{json_path}[{index}]"
        try:
            path = safe_under_root(root, row.get("path"), label="declared_artifact_path")
        except ValueError as exc:
            findings.append(diagnostic(f"unsafe_declared_artifact_path:{exc}", f"unsafe declared artifact path: {exc}", artifact_path=where, json_path=f"{row_path}.path", failure_source_class="path_safety"))
            continue
        if not path.exists() or not path.is_file():
            findings.append(diagnostic("missing_declared_artifact", "declared artifact is missing", artifact_path=path, json_path=f"{row_path}.path", failure_source_class="filesystem"))
            continue
        role = str(row.get("role"))
        if role == "summary" and isinstance(where, Path) and path.resolve() == where.resolve():
            continue
        if row.get("sha256") is not None and row.get("sha256") != sha256_file(path):
            findings.append(diagnostic("declared_artifact_sha256_mismatch", "declared artifact hash is stale", artifact_path=path, json_path=f"{row_path}.sha256", failure_source_class="provenance_hash"))
        if row.get("byte_size") is not None and row.get("byte_size") != path.stat().st_size:
            findings.append(diagnostic("declared_artifact_byte_size_mismatch", "declared artifact byte size is stale", artifact_path=path, json_path=f"{row_path}.byte_size", failure_source_class="provenance_hash"))
    return findings


def validate_contract(role: str, payload: Mapping[str, Any], *, where: Path) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    expected = REQUIRED_SUMMARY_CONTRACTS.get(role)
    if expected:
        expected_slice, expected_schema = expected
        if payload.get("slice_id") != expected_slice:
            findings.append(diagnostic("slice_contract_mismatch", f"{role} has unexpected slice_id", artifact_path=where, json_path="$.slice_id", failure_source_class="artifact_contract"))
        if expected_schema is not None and payload.get("schema_version") != expected_schema:
            findings.append(diagnostic("schema_contract_mismatch", f"{role} has unexpected schema_version", artifact_path=where, json_path="$.schema_version", failure_source_class="artifact_contract"))
    if payload.get("selection_id") not in (None, SELECTION_ID):
        findings.append(diagnostic("selection_contract_mismatch", f"{role} has unexpected selection_id", artifact_path=where, json_path="$.selection_id", failure_source_class="artifact_contract"))
    if payload.get("milestone_id") not in (None, MILESTONE_ID):
        findings.append(diagnostic("milestone_contract_mismatch", f"{role} has unexpected milestone_id", artifact_path=where, json_path="$.milestone_id", failure_source_class="artifact_contract"))
    if role.endswith("verification") and payload.get("status") != "passed":
        findings.append(diagnostic("upstream_verification_not_passed", f"{role} must be passed before S07 synthesis", artifact_path=where, json_path="$.status", failure_source_class="artifact_contract"))
    if role == "s06_maintainability_diagnostic":
        if payload.get("diagnostic_only") is not True or payload.get("blocking") is True or payload.get("pass_fail_affected") is True:
            findings.append(diagnostic("riskratchet_blocking_or_not_diagnostic_only", "S06 maintainability telemetry must remain diagnostic-only and non-blocking", artifact_path=where, json_path="$", failure_source_class="riskratchet"))
    if role == "s06_riskratchet_summary":
        risk = payload.get("riskratchet") if isinstance(payload.get("riskratchet"), dict) else {}
        if risk.get("blocking") is True or risk.get("pass_fail_affected") is True or risk.get("diagnostic_only") is not True:
            findings.append(diagnostic("riskratchet_summary_blocking_or_not_diagnostic_only", "S06 riskratchet summary must remain diagnostic-only and non-blocking", artifact_path=where, json_path="$.riskratchet", failure_source_class="riskratchet"))
    if role == "s05_readiness_decision":
        if payload.get("decision") != "not_import_ready_validate_only" or payload.get("ready_for_import") is True:
            findings.append(diagnostic("readiness_decision_claim_creep", "S05 decision must remain not_import_ready_validate_only", artifact_path=where, json_path="$.decision", failure_source_class="claim_boundary"))
    return findings


def load_inputs(root: Path) -> tuple[dict[str, Any], dict[str, list[dict[str, Any]]], list[dict[str, Any]], list[dict[str, Any]]]:
    payloads: dict[str, Any] = {}
    jsonl_payloads: dict[str, list[dict[str, Any]]] = {}
    diagnostics: list[dict[str, Any]] = []
    input_rows: list[dict[str, Any]] = []
    for role, path, description in INPUT_ARTIFACTS:
        input_rows.append(artifact_row(path, role=role, description=description, root=root))
        if path.suffix == ".jsonl":
            rows, findings = load_jsonl(path)
            jsonl_payloads[role] = rows
            diagnostics.extend(findings)
            diagnostics.extend(validate_no_payload_leakage(rows, serialized=json.dumps(rows, sort_keys=True), where=path))
            for index, row in enumerate(rows):
                if isinstance(row, dict):
                    diagnostics.extend(false_flag_diagnostics(row, where=path, json_prefix=f"$[{index}]"))
            continue
        if path.suffix == ".py":
            if not path.exists():
                diagnostics.append(diagnostic("missing_script_artifact", "required verifier script is missing", artifact_path=path, failure_source_class="filesystem"))
            continue
        payload, findings = load_json(path)
        diagnostics.extend(findings)
        if payload is None:
            continue
        payloads[role] = payload
        diagnostics.extend(validate_contract(role, payload, where=path))
        diagnostics.extend(false_flag_diagnostics(payload, where=path))
        diagnostics.extend(validate_path_fields(payload, where=path, root=root))
        diagnostics.extend(validate_no_payload_leakage(payload, serialized=json.dumps(payload, sort_keys=True), where=path))
        for artifact_json_path in ("$.input_artifacts", "$.output_artifacts", "$.provenance.input_artifacts", "$.provenance.output_artifacts"):
            node = payload
            for part in artifact_json_path.removeprefix("$.").split("."):
                node = node.get(part) if isinstance(node, dict) else None
            diagnostics.extend(validate_declared_artifact_rows(node, where=path, root=root, json_path=artifact_json_path))
    return payloads, jsonl_payloads, diagnostics, input_rows


def counter_from(obj: Any) -> dict[str, int]:
    if isinstance(obj, dict):
        return {str(k): int(v) for k, v in obj.items() if isinstance(v, int)}
    return {}


def build_requirement_coverage() -> list[dict[str, str]]:
    return [
        {"requirement_id": "R036", "status": "owned", "coverage": "S07 synthesizes provenance closeout across S01-S06 with input hashes, failure phase, diagnostic codes, safety flags, and recovery guidance."},
        {"requirement_id": "R024", "status": "supported_preprocessing_only", "coverage": "S01-S05 provide local corpus selection, capture, conversion, current-pipeline baseline, and replay evidence without production import."},
        {"requirement_id": "R027", "status": "supported_preprocessing_only", "coverage": "S03-S05 preserve conversion/replay boundary evidence and metadata-only decisions for six selected articles."},
        {"requirement_id": "R029", "status": "supported_preprocessing_only", "coverage": "S04-S06 expose current-pipeline behavior, exact replay comparison, and diagnostic-only maintainability telemetry."},
        {"requirement_id": "R019", "status": "out_of_scope_future", "coverage": "Graph/trusted fact readiness remains explicitly out of scope; no import-ready evidence is claimed."},
        {"requirement_id": "R022", "status": "out_of_scope_future", "coverage": "Production graph import, KG writes, and import eligibility remain future work."},
        {"requirement_id": "R023", "status": "out_of_scope_future", "coverage": "Article text/binary payload handling beyond metadata-only artifacts is not introduced here."},
        {"requirement_id": "R031", "status": "out_of_scope_future", "coverage": "Unattended scale, CI scheduling, dashboards, and runtime services are not claimed."},
        {"requirement_id": "R032", "status": "out_of_scope_future", "coverage": "Operational paging or production monitoring is not introduced by S07."},
        {"requirement_id": "R033", "status": "out_of_scope_future", "coverage": "Graph/import promotion policy remains future validation work."},
    ]


def build_summary(payloads: Mapping[str, Mapping[str, Any]], jsonl_payloads: Mapping[str, Sequence[Mapping[str, Any]]], diagnostics: Sequence[Mapping[str, Any]], input_rows: Sequence[Mapping[str, Any]], *, root: Path) -> dict[str, Any]:
    severity_counts = Counter(str(row.get("severity", "unknown")) for row in diagnostics)
    phase_counts = Counter(str(row.get("phase", "unknown")) for row in diagnostics)
    code_counts = Counter(str(row.get("diagnostic_code", "unknown")) for row in diagnostics)
    error_count = severity_counts.get("error", 0)
    status = "passed" if error_count == 0 else "failed"
    catalog = payloads.get("s01_catalog_summary", {})
    s02 = payloads.get("s02_source_acquisition_summary", {})
    s03 = payloads.get("s03_conversion_quality_summary", {})
    s04 = payloads.get("s04_current_pipeline_baseline_summary", {})
    s05 = payloads.get("s05_end_to_end_replay_summary", {})
    decision = payloads.get("s05_readiness_decision", {})
    s06 = payloads.get("s06_riskratchet_summary", {})
    maintainability = payloads.get("s06_maintainability_diagnostic", {})
    readiness_state = "ready_with_blockers_conditions" if status == "passed" else "not_ready"
    blockers = [
        "No graph import, trusted fact, import-ready, production, or LadybugDB readiness is claimed.",
        "S05 decision remains not_import_ready_validate_only.",
        "One parser-ready variant still produces zero chunks and metadata-only variants remain non-parser-ready evidence only.",
        "Riskratchet telemetry is diagnostic-only and non-blocking; it is not a production quality gate.",
    ]
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_ids": list(SOURCE_SLICE_IDS),
        "selection_id": SELECTION_ID,
        "status": status,
        "verification_result": status,
        "created_at": utc_now(),
        "health": {
            "status": status,
            "readiness_state": readiness_state,
            "failure_phase_counts": dict(sorted(phase_counts.items())),
            "severity_counts": dict(sorted(severity_counts.items())),
            "diagnostic_count": len(diagnostics),
            "error_count": error_count,
            "diagnostic_codes": dict(sorted(code_counts.items())),
        },
        "provenance": {
            "command": " ".join(sys.argv),
            "cwd": str(Path.cwd()),
            "git_commit": git_commit_from_head(root),
            "input_artifacts": list(input_rows),
            "output_artifacts": [],
            "self_hash_excluded": True,
            "self_hash_excluded_reason": "summary output provenance is self-referential; validate-only skips the summary row hash and enforces diagnostics/report hashes",
        },
        "functional_readiness": {
            "ready_now": [
                "Local six-article S01-S06 preprocessing evidence is present and hash-enumerated." if status == "passed" else "No ready-now claim while validation has errors.",
                "Catalog, capture, conversion, baseline, replay, and provenance/riskratchet boundaries can be inspected from local metadata artifacts." if status == "passed" else "Fix diagnostics before trusting the local metadata chain.",
            ],
            "ready_with_blockers_conditions": blockers,
            "not_ready": [
                "Not ready for graph import, trusted KG facts, production ingestion, unattended scaling, or parser-quality claims.",
                "Not ready if any S07 diagnostic has severity=error; validation exits non-zero in that case.",
            ],
        },
        "slice_readiness": {
            "S01": {"artifact_role": "s01_catalog_summary", "article_count": catalog.get("article_count"), "variant_count": catalog.get("variant_count"), "status": "metadata_boundary_present" if catalog else "missing"},
            "S02": {"artifact_role": "s02_source_acquisition_summary", "status": s02.get("status"), "captured_variant_count": s02.get("variant_count"), "network_capture_boundary": "capture may have been allowed upstream; S07 does not fetch"},
            "S03": {"artifact_role": "s03_conversion_quality_summary", "status": s03.get("status"), "parser_ready_count": s03.get("parser_ready_count"), "counts": counter_from(s03.get("counts"))},
            "S04": {"artifact_role": "s04_current_pipeline_baseline_summary", "status": s04.get("status"), "chunk_count": s04.get("current_pipeline_chunk_count"), "import_ready_count": s04.get("import_ready_count")},
            "S05": {"artifact_role": "s05_end_to_end_replay_summary", "status": s05.get("status"), "chunk_count": s05.get("chunk_count"), "baseline_missing_count": s05.get("baseline_missing_count"), "decision": decision.get("decision")},
            "S06": {"artifact_role": "s06_riskratchet_summary", "status": s06.get("status"), "riskratchet": s06.get("riskratchet", {}), "maintainability_status": maintainability.get("status")},
        },
        "module_boundaries": [
            {"boundary": "catalog_and_selection", "owned_by": "S01", "status": "local metadata only"},
            {"boundary": "source_acquisition", "owned_by": "S02", "status": "captured source metadata; replay phase no-network"},
            {"boundary": "conversion_quality", "owned_by": "S03", "status": "converted/metadata-only quality evidence; no raw payload embedding"},
            {"boundary": "current_pipeline_baseline", "owned_by": "S04", "status": "retrieval-only baseline, import eligibility forced to zero"},
            {"boundary": "end_to_end_replay", "owned_by": "S05", "status": "exact replay comparison and not-import-ready decision"},
            {"boundary": "provenance_riskratchet", "owned_by": "S06", "status": "diagnostic-only maintainability telemetry"},
            {"boundary": "pipeline_readiness_synthesis", "owned_by": "S07", "status": "validation/report surface only"},
        ],
        "integration_gaps": [
            "Graph import and trusted fact promotion remain blocked and unattempted.",
            "Parser-ready zero-chunk behavior is preserved as current-pipeline evidence, not corrected here.",
            "Metadata-only variants remain non-parser-ready and cannot support graph/import claims.",
            "No dashboard, pager, scheduled CI, or runtime readiness service is introduced.",
            "No unattended 10x scale claim is made beyond linear local hashing/rendering characteristics.",
        ],
        "requirement_coverage": build_requirement_coverage(),
        "drill_down_paths": {
            row[0]: rel(row[1], root) for row in INPUT_ARTIFACTS
        } | {
            "s07_summary": rel(SUMMARY_PATH, root),
            "s07_diagnostics": rel(DIAGNOSTICS_PATH, root),
            "s07_report": rel(REPORT_PATH, root),
        },
        "safety_flags": dict(FALSE_SAFETY_FLAGS),
        "failure_modes": {
            "filesystem": "Missing/unreadable/stale local artifacts emit diagnostics with artifact_path/json_path and non-zero exit.",
            "json_jsonl": "Malformed JSON or JSONL rows emit stable malformed_* diagnostics and block readiness synthesis.",
            "network": "No network API is called; URL-like artifact references are rejected as path-tampering risks.",
            "subprocess": "No subprocess is invoked by S07; upstream verifier scripts are hashed as local provenance inputs only.",
            "graph_database": "No graph database, LadybugDB, production import, or trusted KG writer is called; all related flags remain false.",
        },
        "load_profile": {
            "expected_articles": 6,
            "expected_input_artifacts": len(INPUT_ARTIFACTS),
            "first_10x_saturation": "local filesystem hashing and JSON/Markdown rendering grow linearly with artifact count and report size",
            "protection": "streaming SHA-256 reads, local-only path checks, no network/database pools, no graph writers, and no unattended scaling claims",
        },
        "negative_tests": {
            "covered_by": "tests/test_m027_pipeline_readiness_synthesis.py",
            "cases": [
                "missing upstream JSON produces missing_json_artifact and failed status",
                "URL-like artifact path references produce unsafe_artifact_reference diagnostics",
                "readiness/import claim creep produces unsafe_safety_or_readiness_flag_true diagnostics",
                "stale declared output hashes produce declared_artifact_sha256_mismatch diagnostics",
            ],
        },
        "next_cycle_recommendations": [
            "Keep the next staged corpus cycle validate-only until parser-ready zero-chunk behavior and metadata-only boundaries are resolved.",
            "Promote graph/import readiness only in a separate slice with explicit import eligibility tests and LadybugDB write controls.",
            "Carry forward S07 input hashes and diagnostic codes as the first freshness check for future agents.",
            "If 10x corpus work is attempted, add pagination/batching evidence before claiming unattended scale.",
        ],
        "diagnostics": list(diagnostics),
        "jsonl_diagnostic_row_counts": {role: len(rows) for role, rows in jsonl_payloads.items()},
        **FALSE_SAFETY_FLAGS,
    }


def validate_s07_outputs(summary_path: Path, diagnostics_path: Path, report_path: Path, *, root: Path) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    summary, loaded = load_json(summary_path)
    findings.extend(loaded)
    diagnostic_rows, loaded_rows = load_jsonl(diagnostics_path)
    findings.extend(loaded_rows)
    if not report_path.exists() or not report_path.is_file():
        findings.append(diagnostic("missing_report_artifact", "S07 markdown report is missing", artifact_path=report_path, failure_source_class="filesystem"))
    else:
        try:
            report_text = report_path.read_text(encoding="utf-8")
        except OSError as exc:
            findings.append(diagnostic("report_artifact_unreadable", f"S07 markdown report is unreadable: {exc}", artifact_path=report_path, failure_source_class="filesystem"))
        else:
            for heading in ("# M027 Pipeline Readiness Synthesis", "## Ready now", "## Ready with blockers/conditions", "## Not ready", "## Requirement coverage", "## Health, failure, and recovery", "## Next-cycle recommendations"):
                if heading not in report_text:
                    findings.append(diagnostic("report_required_section_missing", f"S07 report missing required section: {heading}", artifact_path=report_path, failure_source_class="report_contract"))
            findings.extend(validate_no_payload_leakage({"report": report_text}, serialized=report_text, where=report_path))
    if summary is None:
        return findings
    if summary.get("schema_version") != SCHEMA_VERSION or summary.get("slice_id") != SLICE_ID:
        findings.append(diagnostic("s07_summary_contract_mismatch", "S07 summary has unexpected schema_version or slice_id", artifact_path=summary_path, json_path="$.schema_version", failure_source_class="artifact_contract"))
    findings.extend(false_flag_diagnostics(summary, where=summary_path))
    findings.extend(validate_no_payload_leakage(summary, serialized=json.dumps(summary, sort_keys=True), where=summary_path))
    rows = summary.get("provenance", {}).get("output_artifacts") if isinstance(summary.get("provenance"), dict) else None
    if not isinstance(rows, list):
        findings.append(diagnostic("missing_output_artifact_provenance", "S07 summary lacks provenance.output_artifacts", artifact_path=summary_path, json_path="$.provenance.output_artifacts", failure_source_class="provenance_hash"))
    else:
        roles = {str(row.get("role")) for row in rows if isinstance(row, dict)}
        for required in {"summary", "diagnostics", "report"} - roles:
            findings.append(diagnostic("output_artifact_role_missing", f"S07 output artifact role missing: {required}", artifact_path=summary_path, json_path="$.provenance.output_artifacts", failure_source_class="provenance_hash"))
        for index, row in enumerate(rows):
            if not isinstance(row, dict):
                findings.append(diagnostic("malformed_output_artifact_row", "S07 output artifact row must be an object", artifact_path=summary_path, json_path=f"$.provenance.output_artifacts[{index}]", failure_source_class="provenance_hash"))
                continue
            try:
                path = safe_under_root(root, row.get("path"), label="output_artifact_path")
            except ValueError as exc:
                findings.append(diagnostic(f"unsafe_output_artifact_path:{exc}", f"unsafe output artifact path: {exc}", artifact_path=summary_path, json_path=f"$.provenance.output_artifacts[{index}].path", failure_source_class="path_safety"))
                continue
            if not path.exists() or not path.is_file():
                findings.append(diagnostic("missing_output_artifact", "S07 output artifact is missing", artifact_path=path, json_path=f"$.provenance.output_artifacts[{index}].path", failure_source_class="filesystem"))
                continue
            if row.get("role") == "summary" and summary.get("provenance", {}).get("self_hash_excluded") is True:
                continue
            if row.get("sha256") != sha256_file(path):
                findings.append(diagnostic("output_artifact_sha256_mismatch", "S07 output artifact hash is stale", artifact_path=path, json_path=f"$.provenance.output_artifacts[{index}].sha256", failure_source_class="provenance_hash"))
            if row.get("byte_size") != path.stat().st_size:
                findings.append(diagnostic("output_artifact_byte_size_mismatch", "S07 output artifact byte size is stale", artifact_path=path, json_path=f"$.provenance.output_artifacts[{index}].byte_size", failure_source_class="provenance_hash"))
    expected_diagnostics = summary.get("health", {}).get("diagnostic_count") if isinstance(summary.get("health"), dict) else None
    if isinstance(expected_diagnostics, int) and expected_diagnostics != len(diagnostic_rows):
        findings.append(diagnostic("diagnostic_row_count_mismatch", "S07 diagnostics JSONL row count does not match summary health", artifact_path=diagnostics_path, json_path="$.health.diagnostic_count", failure_source_class="artifact_contract"))
    return findings


def write_json(path: Path, payload: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_jsonl(path: Path, rows: Sequence[Mapping[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8")


def write_report(path: Path, summary: Mapping[str, Any]) -> None:
    health = summary.get("health", {}) if isinstance(summary.get("health"), dict) else {}
    functional = summary.get("functional_readiness", {}) if isinstance(summary.get("functional_readiness"), dict) else {}
    lines: list[str] = []
    lines.append("# M027 Pipeline Readiness Synthesis")
    lines.append("")
    lines.append(f"Status: **{summary.get('status')}**. Readiness state: **{health.get('readiness_state')}**.")
    lines.append("")
    lines.append("This report is metadata-only. It summarizes local S01-S06 preprocessing evidence and does not claim graph import, trusted facts, production readiness, or unattended scale.")
    lines.append("")
    lines.append("## Ready now")
    for item in functional.get("ready_now", []):
        lines.append(f"- {item}")
    lines.append("")
    lines.append("## Ready with blockers/conditions")
    for item in functional.get("ready_with_blockers_conditions", []):
        lines.append(f"- {item}")
    lines.append("")
    lines.append("## Not ready")
    for item in functional.get("not_ready", []):
        lines.append(f"- {item}")
    lines.append("")
    lines.append("## Functional readiness by slice")
    slice_readiness = summary.get("slice_readiness", {}) if isinstance(summary.get("slice_readiness"), dict) else {}
    for slice_id, row in slice_readiness.items():
        lines.append(f"- **{slice_id}**: `{json.dumps(row, sort_keys=True)}`")
    lines.append("")
    lines.append("## Module boundaries")
    for row in summary.get("module_boundaries", []):
        if isinstance(row, dict):
            lines.append(f"- **{row.get('owned_by')} / {row.get('boundary')}**: {row.get('status')}")
    lines.append("")
    lines.append("## Integration gaps")
    for item in summary.get("integration_gaps", []):
        lines.append(f"- {item}")
    lines.append("")
    lines.append("## Requirement coverage")
    for row in summary.get("requirement_coverage", []):
        if isinstance(row, dict):
            lines.append(f"- **{row.get('requirement_id')}** ({row.get('status')}): {row.get('coverage')}")
    lines.append("")
    lines.append("## Provenance evidence and drill-down paths")
    for role, path_value in (summary.get("drill_down_paths", {}) if isinstance(summary.get("drill_down_paths"), dict) else {}).items():
        lines.append(f"- `{role}`: `{path_value}`")
    lines.append("")
    lines.append("## Health, failure, and recovery")
    lines.append(f"- Diagnostic count: `{health.get('diagnostic_count')}`; errors: `{health.get('error_count')}`")
    lines.append(f"- Failure phase counts: `{json.dumps(health.get('failure_phase_counts', {}), sort_keys=True)}`")
    lines.append(f"- Diagnostic codes: `{json.dumps(health.get('diagnostic_codes', {}), sort_keys=True)}`")
    diagnostics = summary.get("diagnostics", []) if isinstance(summary.get("diagnostics"), list) else []
    if diagnostics:
        lines.append("- Blocking diagnostics:")
        for row in diagnostics[:20]:
            if isinstance(row, dict):
                lines.append(f"  - `{row.get('diagnostic_code')}` at `{row.get('artifact_path')}` `{row.get('json_path')}`: {row.get('message')} Recovery: {row.get('recovery_guidance')}")
    else:
        lines.append("- No S07 validation diagnostics were emitted.")
    lines.append("")
    lines.append("## Failure Modes")
    for key, value in (summary.get("failure_modes", {}) if isinstance(summary.get("failure_modes"), dict) else {}).items():
        lines.append(f"- **{key}**: {value}")
    lines.append("")
    lines.append("## Load Profile")
    load_profile = summary.get("load_profile", {}) if isinstance(summary.get("load_profile"), dict) else {}
    for key, value in load_profile.items():
        lines.append(f"- **{key}**: {value}")
    lines.append("")
    lines.append("## Negative Tests")
    negative = summary.get("negative_tests", {}) if isinstance(summary.get("negative_tests"), dict) else {}
    lines.append(f"- Covered by: `{negative.get('covered_by')}`")
    for item in negative.get("cases", []):
        lines.append(f"- {item}")
    lines.append("")
    lines.append("## Observability Impact")
    lines.append("S07 writes machine-readable health, failure phases, diagnostic codes, artifact paths, JSON paths, SHA-256/byte-size provenance rows, safety flags, and recovery guidance for future agents.")
    lines.append("")
    lines.append("## Next-cycle recommendations")
    for item in summary.get("next_cycle_recommendations", []):
        lines.append(f"- {item}")
    lines.append("")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def generate(args: argparse.Namespace) -> int:
    root = Path(args.root)
    payloads, jsonl_payloads, diagnostics, input_rows = load_inputs(root)
    summary = build_summary(payloads, jsonl_payloads, diagnostics, input_rows, root=root)
    write_json(SUMMARY_PATH, summary)
    write_jsonl(DIAGNOSTICS_PATH, diagnostics)
    write_report(REPORT_PATH, summary)
    output_rows = [
        {"role": "summary", "description": "S07 readiness synthesis summary", "path": rel(SUMMARY_PATH, root), "exists": SUMMARY_PATH.exists(), "sha256": None, "byte_size": SUMMARY_PATH.stat().st_size if SUMMARY_PATH.exists() else None},
        artifact_row(DIAGNOSTICS_PATH, role="diagnostics", description="S07 readiness synthesis diagnostics JSONL", root=root),
        artifact_row(REPORT_PATH, role="report", description="S07 readiness synthesis markdown report", root=root),
    ]
    summary["provenance"]["output_artifacts"] = output_rows
    write_json(SUMMARY_PATH, summary)
    post_findings = validate_s07_outputs(SUMMARY_PATH, DIAGNOSTICS_PATH, REPORT_PATH, root=root)
    blocking_post = [row for row in post_findings if row.get("severity") == "error"]
    if blocking_post:
        combined = list(diagnostics) + blocking_post
        summary = build_summary(payloads, jsonl_payloads, combined, input_rows, root=root)
        summary["provenance"]["output_artifacts"] = output_rows
        write_jsonl(DIAGNOSTICS_PATH, combined)
        write_report(REPORT_PATH, summary)
        write_json(SUMMARY_PATH, summary)
        return 1
    return 0 if summary.get("status") == "passed" else 1


def validate_only(args: argparse.Namespace) -> int:
    root = Path(args.root)
    # Validate upstream freshness too, without rewriting any output.
    _payloads, _jsonl_payloads, upstream_findings, _input_rows = load_inputs(root)
    output_findings = validate_s07_outputs(SUMMARY_PATH, DIAGNOSTICS_PATH, REPORT_PATH, root=root)
    all_findings = list(upstream_findings) + list(output_findings)
    if all_findings:
        for row in all_findings:
            print(json.dumps(row, sort_keys=True), file=sys.stderr)
    return 0 if not [row for row in all_findings if row.get("severity") == "error"] else 1


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--validate-only", action="store_true", help="validate existing S07 outputs and upstream artifacts without rewriting outputs")
    parser.add_argument("--root", default=str(ROOT), help="project root for path safety and relative provenance")
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    return validate_only(args) if args.validate_only else generate(args)


if __name__ == "__main__":
    raise SystemExit(main())
