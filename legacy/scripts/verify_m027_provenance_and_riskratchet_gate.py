#!/usr/bin/env python3
"""Generate or validate the M027 S06 provenance and riskratchet gate artifacts.

The gate is local-only. Default mode validates existing S05 replay artifacts,
runs the diagnostic-only local maintainability quality gate over an explicit
Python scope, and writes S06 summary/diagnostic/report artifacts. --validate-only
reads those artifacts back without rerunning riskratchet or rewriting outputs.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections.abc import Mapping, Sequence
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))
if str(ROOT / "src") not in sys.path:
    sys.path.insert(0, str(ROOT / "src"))

# pyrefly: ignore [missing-import]
from scripts import run_quality_gate  # noqa: E402

MILESTONE_ID = "M027-aakeky"
SLICE_ID = "S06"
SOURCE_SLICE_ID = "S05"
SELECTION_ID = "m027-mixed-source-corpus-v1"
SCHEMA_VERSION = "m027-provenance-riskratchet-gate.v1"
DIAGNOSTIC_SCHEMA_VERSION = "m027-provenance-riskratchet-gate-diagnostic.v1"
CORPUS_DIR = ROOT / "data" / "article_corpora" / SELECTION_ID
GATE_DIR = CORPUS_DIR / "provenance-riskratchet-gate"
SUMMARY_PATH = GATE_DIR / "provenance-riskratchet-gate-summary.json"
DIAGNOSTICS_PATH = GATE_DIR / "provenance-riskratchet-gate-diagnostics.jsonl"
REPORT_PATH = GATE_DIR / "provenance-riskratchet-gate-report.md"
MAINTAINABILITY_JSON_PATH = GATE_DIR / run_quality_gate.JSON_REPORT_NAME
MAINTAINABILITY_REPORT_PATH = GATE_DIR / run_quality_gate.HUMAN_REPORT_NAME
S05_SUMMARY_PATH = CORPUS_DIR / "end-to-end-mixed-replay-summary.json"
S05_DIAGNOSTICS_PATH = CORPUS_DIR / "end-to-end-mixed-replay-diagnostics.jsonl"
S05_EVENTS_PATH = CORPUS_DIR / "end-to-end-mixed-replay-events.jsonl"
S05_DECISION_PATH = CORPUS_DIR / "end-to-end-mixed-replay-readiness-decision.json"
S05_VERIFICATION_PATH = CORPUS_DIR / "end-to-end-mixed-replay-verification.json"
QUALITY_SCOPE = (
    Path("scripts/verify_m027_provenance_and_riskratchet_gate.py"),
    Path("scripts/replay_m027_end_to_end_mixed_replay.py"),
    Path("scripts/verify_m027_end_to_end_mixed_replay.py"),
    Path("scripts/run_quality_gate.py"),
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
FORBIDDEN_SNIPPETS = (
    "<html",
    "</html",
    "%PDF-",
    "base64,",
    "RAW_ARXIV_ABS_SECRET",
    "RAW_NATURE_BODY_SECRET",
    "RAW_PDF_SECRET",
)


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


def git_commit_from_head(root: Path) -> str | None:
    head_path = root / ".git" / "HEAD"
    try:
        head = head_path.read_text(encoding="utf-8").strip()
    except OSError:
        return None
    if head.startswith("ref: "):
        ref_path = root / ".git" / head.removeprefix("ref: ").strip()
        try:
            return ref_path.read_text(encoding="utf-8").strip() or None
        except OSError:
            return None
    return head or None


def safe_relative_path(value: Any, *, label: str) -> PurePosixPath:
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
    source: str = "provenance",
) -> dict[str, Any]:
    path_value = rel(artifact_path) if isinstance(artifact_path, Path) else artifact_path
    return {
        "schema_version": DIAGNOSTIC_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "phase": phase,
        "artifact_path": path_value,
        "path": path_value,
        "diagnostic_code": code,
        "code": code,
        "json_path": json_path,
        "severity": severity,
        "source": source,
        "failure_domain": source,
        "message": message,
        **FALSE_SAFETY_FLAGS,
    }


def load_json(path: Path) -> tuple[dict[str, Any] | None, list[dict[str, Any]]]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return None, [
            diagnostic(
                "missing_json_artifact", "required JSON artifact is missing", artifact_path=path
            )
        ]
    except json.JSONDecodeError as exc:
        return None, [
            diagnostic(
                "malformed_json_artifact",
                f"required JSON artifact is malformed: {exc}",
                artifact_path=path,
            )
        ]
    except OSError as exc:
        return None, [
            diagnostic(
                "json_artifact_unreadable",
                f"required JSON artifact is unreadable: {exc}",
                artifact_path=path,
            )
        ]
    if not isinstance(value, dict):
        return None, [
            diagnostic(
                "json_artifact_not_object",
                "required JSON artifact must be a JSON object",
                artifact_path=path,
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
            diagnostic(
                "missing_jsonl_artifact", "required JSONL artifact is missing", artifact_path=path
            )
        ]
    except OSError as exc:
        return rows, [
            diagnostic(
                "jsonl_artifact_unreadable",
                f"required JSONL artifact is unreadable: {exc}",
                artifact_path=path,
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
                    "malformed_jsonl_artifact",
                    f"JSONL row is malformed: {exc}",
                    artifact_path=path,
                    json_path=f"$[{line_number}]",
                )
            )
            continue
        if not isinstance(value, dict):
            findings.append(
                diagnostic(
                    "malformed_jsonl_artifact",
                    "JSONL row must be an object",
                    artifact_path=path,
                    json_path=f"$[{line_number}]",
                )
            )
            continue
        rows.append(value)
    return rows, findings


def validate_no_payload_leakage(
    value: Any, *, serialized: str, where: Path | str
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []

    def walk(node: Any, path: str) -> None:
        if isinstance(node, dict):
            for key, item in node.items():
                if key in FORBIDDEN_PAYLOAD_KEYS:
                    findings.append(
                        diagnostic(
                            "metadata_payload_key_leakage",
                            f"metadata artifact includes forbidden payload key {key!r}",
                            artifact_path=where,
                            json_path=f"{path}.{key}",
                            source="redaction",
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
                    "metadata artifact includes a forbidden raw payload sentinel",
                    artifact_path=where,
                    source="redaction",
                )
            )
    return findings


def false_flag_diagnostics(
    value: Mapping[str, Any], *, where: Path | str, json_prefix: str = "$"
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    for key in sorted(UNSAFE_TRUE_FLAGS):
        if value.get(key) is True:
            findings.append(
                diagnostic(
                    "unsafe_safety_flag_true",
                    f"unsafe fail-closed flag is true: {key}",
                    artifact_path=where,
                    json_path=f"{json_prefix}.{key}",
                    source="safety_flags",
                )
            )
    return findings


def artifact_row(path: Path, *, role: str, root: Path) -> dict[str, Any]:
    return {
        "role": role,
        "path": rel(path, root),
        "exists": path.exists() and path.is_file(),
        "sha256": sha256_file(path) if path.exists() and path.is_file() else None,
        "byte_size": path.stat().st_size if path.exists() and path.is_file() else None,
    }


def validate_s05_artifacts(
    args: argparse.Namespace,
) -> tuple[dict[str, Any], list[dict[str, Any]], list[dict[str, Any]]]:
    diagnostics: list[dict[str, Any]] = []
    summary, findings = load_json(Path(args.s05_summary))
    diagnostics.extend(findings)
    jsonl_rows, findings = load_jsonl(Path(args.s05_diagnostics))
    diagnostics.extend(findings)
    events_rows, findings = load_jsonl(Path(args.s05_events))
    diagnostics.extend(findings)
    decision, findings = load_json(Path(args.s05_readiness_decision))
    diagnostics.extend(findings)
    verification, findings = load_json(Path(args.s05_verification))
    diagnostics.extend(findings)

    artifacts = [
        (Path(args.s05_summary), summary, "s05_summary"),
        (Path(args.s05_diagnostics), jsonl_rows, "s05_diagnostics"),
        (Path(args.s05_events), events_rows, "s05_events"),
        (Path(args.s05_readiness_decision), decision, "s05_readiness_decision"),
        (Path(args.s05_verification), verification, "s05_verification"),
    ]
    for path, payload, _role in artifacts:
        if payload is not None:
            diagnostics.extend(
                validate_no_payload_leakage(
                    payload, serialized=json.dumps(payload, sort_keys=True), where=path
                )
            )
    if summary:
        diagnostics.extend(false_flag_diagnostics(summary, where=Path(args.s05_summary)))
        if (
            summary.get("schema_version") != "m027-end-to-end-mixed-replay.v1"
            or summary.get("slice_id") != SOURCE_SLICE_ID
        ):
            diagnostics.append(
                diagnostic(
                    "s05_summary_contract_mismatch",
                    "S05 replay summary has an unexpected contract",
                    artifact_path=Path(args.s05_summary),
                    json_path="$.schema_version",
                )
            )
        for index, row in enumerate(
            summary.get("output_artifacts")
            if isinstance(summary.get("output_artifacts"), list)
            else []  # ty:ignore[invalid-argument-type]
        ):
            if not isinstance(row, dict):
                diagnostics.append(
                    diagnostic(
                        "malformed_s05_output_artifact",
                        "S05 output_artifacts row is not an object",
                        artifact_path=Path(args.s05_summary),
                        json_path=f"$.output_artifacts[{index}]",
                    )
                )
                continue
            try:
                artifact_path = safe_under_root(
                    Path(args.root), row.get("path"), label="s05_output_artifact_path"
                )
            except ValueError as exc:
                diagnostics.append(
                    diagnostic(
                        f"unsafe_s05_output_artifact_path:{exc}",
                        f"unsafe S05 output artifact path: {exc}",
                        artifact_path=Path(args.s05_summary),
                        json_path=f"$.output_artifacts[{index}].path",
                    )
                )
                continue
            if not artifact_path.exists() or not artifact_path.is_file():
                diagnostics.append(
                    diagnostic(
                        "missing_s05_output_artifact",
                        "S05 output artifact is missing",
                        artifact_path=artifact_path,
                        json_path=f"$.output_artifacts[{index}].path",
                    )
                )
                continue
            if row.get("role") == "summary":
                continue
            if row.get("sha256") != sha256_file(artifact_path):
                diagnostics.append(
                    diagnostic(
                        "s05_output_artifact_sha256_mismatch",
                        "S05 output artifact hash is stale",
                        artifact_path=artifact_path,
                        json_path=f"$.output_artifacts[{index}].sha256",
                        source="output_hash_validation",
                    )
                )
            if row.get("byte_size") != artifact_path.stat().st_size:
                diagnostics.append(
                    diagnostic(
                        "s05_output_artifact_byte_size_mismatch",
                        "S05 output artifact byte size is stale",
                        artifact_path=artifact_path,
                        json_path=f"$.output_artifacts[{index}].byte_size",
                        source="output_hash_validation",
                    )
                )
    if decision:
        diagnostics.extend(
            false_flag_diagnostics(decision, where=Path(args.s05_readiness_decision))
        )
    if verification:
        diagnostics.extend(false_flag_diagnostics(verification, where=Path(args.s05_verification)))
        if verification.get("status") != "passed":
            diagnostics.append(
                diagnostic(
                    "s05_verification_not_passed",
                    "S05 validate-only verification must be passed before S06 provenance gate succeeds",
                    artifact_path=Path(args.s05_verification),
                    json_path="$.status",
                )
            )
    return summary or {}, jsonl_rows, diagnostics


def validate_riskratchet_payload(
    report: Mapping[str, Any], *, where: Path | str
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    findings.extend(
        validate_no_payload_leakage(
            report, serialized=json.dumps(report, sort_keys=True), where=where
        )
    )
    if report.get("diagnostic_only") is not True:
        findings.append(
            diagnostic(
                "riskratchet_not_diagnostic_only",
                "riskratchet report must be diagnostic-only",
                artifact_path=where,
                json_path="$.diagnostic_only",
                source="riskratchet_telemetry",
            )
        )
    if report.get("blocking") is True:
        findings.append(
            diagnostic(
                "riskratchet_blocking_true",
                "riskratchet telemetry must not become blocking",
                artifact_path=where,
                json_path="$.blocking",
                source="riskratchet_telemetry",
            )
        )
    if report.get("pass_fail_affected") is True:
        findings.append(
            diagnostic(
                "riskratchet_pass_fail_affected_true",
                "riskratchet telemetry must not affect pass/fail",
                artifact_path=where,
                json_path="$.pass_fail_affected",
                source="riskratchet_telemetry",
            )
        )
    gate = report.get("quality_gate") if isinstance(report.get("quality_gate"), dict) else {}
    if gate.get("blocking") is True:  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        findings.append(
            diagnostic(
                "riskratchet_quality_gate_blocking_true",
                "quality_gate.blocking must remain false",
                artifact_path=where,
                json_path="$.quality_gate.blocking",
                source="riskratchet_telemetry",
            )
        )
    if gate.get("pass_fail_affected") is True:  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        findings.append(
            diagnostic(
                "riskratchet_quality_gate_pass_fail_affected_true",
                "quality_gate.pass_fail_affected must remain false",
                artifact_path=where,
                json_path="$.quality_gate.pass_fail_affected",
                source="riskratchet_telemetry",
            )
        )
    return findings


def run_riskratchet(args: argparse.Namespace) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    try:
        report = run_quality_gate.run_quality_gate(
            paths=[str(path) for path in QUALITY_SCOPE], output_dir=Path(args.output_dir)
        )
    except (
        Exception
    ) as exc:  # diagnostic-only telemetry, but missing outputs remain validated below.
        report = {
            "schema_version": "m027-riskratchet-unavailable.v1",
            "status": "diagnostic_unavailable",
            "diagnostic_only": True,
            "blocking": False,
            "pass_fail_affected": False,
            "tool_status": "unavailable",
            "tool_error": str(exc),
            "quality_gate": {
                "diagnostic_only": True,
                "blocking": False,
                "pass_fail_affected": False,
                "touched_modules": [str(path) for path in QUALITY_SCOPE],
            },
            "summary": {},
            "riskratchet": {"blocking": False, "functions": []},
        }
        return report, [
            diagnostic(
                "riskratchet_runner_unavailable",
                f"riskratchet wrapper reported unavailable telemetry: {exc}",
                severity="warning",
                artifact_path=Path(args.output_dir),
                source="riskratchet_telemetry",
            )
        ]
    return report, []


def riskratchet_summary(report: Mapping[str, Any]) -> dict[str, Any]:
    summary = report.get("summary") if isinstance(report.get("summary"), dict) else {}
    gate = report.get("quality_gate") if isinstance(report.get("quality_gate"), dict) else {}
    return {
        "diagnostic_only": report.get("diagnostic_only") is True,
        "blocking": report.get("blocking") is True or gate.get("blocking") is True,  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "pass_fail_affected": report.get("pass_fail_affected") is True
        or gate.get("pass_fail_affected") is True,  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "tool_status": report.get("tool_status"),
        "status": report.get("status"),
        "touched_module_count": gate.get("touched_module_count"),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "touched_modules": gate.get("touched_modules", [str(path) for path in QUALITY_SCOPE]),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "total_functions": summary.get("total_functions", 0),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "max_score": summary.get("max_score", 0.0),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "average_score": summary.get("average_score", 0.0),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "severity_bands": summary.get("by_severity", {}),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "json_report": rel(
            Path(gate.get("json_report") or MAINTAINABILITY_JSON_PATH),  # pyrefly: ignore [missing-attribute]  # ty:ignore[unresolved-attribute]
            Path(args_root := ROOT),  # pyrefly: ignore[bad-assignment]
        )
        if isinstance(gate.get("json_report"), str)  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        else None,
        "human_report": rel(
            Path(gate.get("human_report") or MAINTAINABILITY_REPORT_PATH),  # pyrefly: ignore [missing-attribute]  # ty:ignore[unresolved-attribute]
            Path(args_root),  # pyrefly: ignore [bad-assignment, unbound-name]
        )
        if isinstance(gate.get("human_report"), str)  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        else None,
    }


def validate_output_artifacts(
    summary: Mapping[str, Any], *, root: Path, summary_path: Path
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    rows = (
        summary.get("provenance", {}).get("output_artifacts")
        if isinstance(summary.get("provenance"), dict)
        else None
    )
    if not isinstance(rows, list) or not rows:
        return [
            diagnostic(
                "missing_output_artifact_provenance",
                "summary lacks provenance.output_artifacts",
                artifact_path=summary_path,
                json_path="$.provenance.output_artifacts",
            )
        ]
    roles = {str(row.get("role")) for row in rows if isinstance(row, dict)}
    for required in {
        "summary",
        "diagnostics",
        "report",
        "maintainability_json",
        "maintainability_report",
    } - roles:
        findings.append(
            diagnostic(
                "output_artifact_role_missing",
                f"S06 output artifact role is missing: {required}",
                artifact_path=summary_path,
                json_path="$.provenance.output_artifacts",
            )
        )
    for index, row in enumerate(rows):
        if not isinstance(row, dict):
            findings.append(
                diagnostic(
                    "malformed_output_artifact_row",
                    "output_artifacts row must be an object",
                    artifact_path=summary_path,
                    json_path=f"$.provenance.output_artifacts[{index}]",
                )
            )
            continue
        try:
            path = safe_under_root(root, row.get("path"), label="output_artifact_path")
        except ValueError as exc:
            findings.append(
                diagnostic(
                    f"unsafe_output_artifact_path:{exc}",
                    f"unsafe output artifact path: {exc}",
                    artifact_path=summary_path,
                    json_path=f"$.provenance.output_artifacts[{index}].path",
                )
            )
            continue
        if not path.exists() or not path.is_file():
            findings.append(
                diagnostic(
                    "missing_output_artifact",
                    "S06 output artifact is missing",
                    artifact_path=path,
                    json_path=f"$.provenance.output_artifacts[{index}].path",
                )
            )
            continue
        if (
            row.get("role") == "summary"
            and summary.get("provenance", {}).get("self_hash_excluded") is True
        ):
            continue
        if row.get("sha256") != sha256_file(path):
            findings.append(
                diagnostic(
                    "output_artifact_sha256_mismatch",
                    "S06 output artifact hash is stale",
                    artifact_path=path,
                    json_path=f"$.provenance.output_artifacts[{index}].sha256",
                    source="output_hash_validation",
                )
            )
        if row.get("byte_size") != path.stat().st_size:
            findings.append(
                diagnostic(
                    "output_artifact_byte_size_mismatch",
                    "S06 output artifact byte size is stale",
                    artifact_path=path,
                    json_path=f"$.provenance.output_artifacts[{index}].byte_size",
                    source="output_hash_validation",
                )
            )
    return findings


def build_summary(
    args: argparse.Namespace,
    s05_summary: Mapping[str, Any],
    s05_diagnostic_rows: Sequence[Mapping[str, Any]],
    risk_report: Mapping[str, Any],
    diagnostics: list[dict[str, Any]],
) -> dict[str, Any]:
    input_paths = [
        Path(args.s05_summary),
        Path(args.s05_diagnostics),
        Path(args.s05_events),
        Path(args.s05_readiness_decision),
        Path(args.s05_verification),
    ]
    risk = riskratchet_summary(risk_report)
    status = (
        "passed" if not [row for row in diagnostics if row.get("severity") == "error"] else "failed"
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": status,
        "verification_result": status,
        "created_at": utc_now(),
        "provenance": {
            "command": " ".join(sys.argv),
            "cwd": str(Path.cwd()),
            "git_commit": git_commit_from_head(Path(args.root)),
            "exit_code": 0 if status == "passed" else 1,
            "exit_status": status,
            "input_artifacts": [
                artifact_row(path, role=role, root=Path(args.root))
                for path, role in zip(
                    input_paths,
                    [
                        "s05_summary",
                        "s05_diagnostics",
                        "s05_events",
                        "s05_readiness_decision",
                        "s05_verification",
                    ],
                    strict=True,
                )
            ],
            "output_artifacts": [],
            "self_hash_excluded": True,
            "self_hash_excluded_reason": "summary output provenance is self-referential; non-summary outputs are hash-enforced validate-only",
        },
        "s05": {
            "schema_version": s05_summary.get("schema_version"),
            "status": s05_summary.get("status"),
            "verification_status": "passed",
            "diagnostic_row_count": len(s05_diagnostic_rows),
            "article_count": s05_summary.get("article_count"),
            "variant_count": s05_summary.get("variant_count"),
        },
        "safety": dict(FALSE_SAFETY_FLAGS),
        "riskratchet": risk,
        "diagnostics": diagnostics,
        "diagnostic_codes": sorted(
            {str(row.get("diagnostic_code")) for row in diagnostics if row.get("diagnostic_code")}
        ),
        "failure_modes": {
            "filesystem": "Missing/malformed S05 JSON/JSONL, stale output hashes, unsafe paths, unreadable reports, and malformed S06 artifacts emit stable diagnostics and non-zero exit.",
            "network": "No network dependency is used; safety flags remain false and URL/path traversal artifact paths are rejected.",
            "subprocess": "Default mode calls the local diagnostic-only quality gate through its Python API; wrapper unavailability is represented as warning telemetry, while missing/malformed telemetry files fail validation.",
            "graph_database": "Graph, production import, and LadybugDB writes are not invoked and are represented by fail-closed false safety flags.",
        },
        "load_profile": {
            "expected_articles": 6,
            "first_10x_saturation": "local file hashing and markdown/JSON report size grow linearly with replay artifact count; riskratchet scans a fixed explicit Python scope",
            "protection": "safe relative path checks, local-only hashing, diagnostic-only riskratchet, no network, no database pools, and no graph writers",
        },
        **FALSE_SAFETY_FLAGS,
    }


def write_json(path: Path, payload: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_jsonl(path: Path, rows: Sequence[Mapping[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8"
    )


def write_report(path: Path, summary: Mapping[str, Any]) -> None:
    diagnostics = summary.get("diagnostics") if isinstance(summary.get("diagnostics"), list) else []
    risk = summary.get("riskratchet") if isinstance(summary.get("riskratchet"), dict) else {}
    lines = [
        "# M027 S06 Provenance And Riskratchet Gate Report",
        "",
        f"- status: `{summary.get('status')}`",
        f"- verification_result: `{summary.get('verification_result')}`",
        f"- milestone_id: `{MILESTONE_ID}`",
        f"- slice_id: `{SLICE_ID}`",
        f"- selection_id: `{SELECTION_ID}`",
        f"- network_fetch_attempted: `{summary.get('network_fetch_attempted')}`",
        f"- production_import_attempted: `{summary.get('production_import_attempted')}`",
        f"- graph_import_allowed: `{summary.get('graph_import_allowed')}`",
        f"- ladybugdb_written: `{summary.get('ladybugdb_written')}`",
        "",
        "This is a validate-only, local-only audit artifact. Riskratchet telemetry is diagnostic-only and non-blocking, and this report is not an import/readiness approval.",
        "",
        "## Provenance",
        f"- Command: `{summary.get('provenance', {}).get('command')}`",
        f"- CWD: `{summary.get('provenance', {}).get('cwd')}`",
        f"- Git commit: `{summary.get('provenance', {}).get('git_commit')}`",
        f"- Self hash excluded: `{summary.get('provenance', {}).get('self_hash_excluded')}`",
        f"- Self hash reason: {summary.get('provenance', {}).get('self_hash_excluded_reason')}",
        "",
        "## Riskratchet",
        f"- Diagnostic only: `{risk.get('diagnostic_only')}`",  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        f"- Blocking: `{risk.get('blocking')}`",  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        f"- Pass/fail affected: `{risk.get('pass_fail_affected')}`",  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        f"- Tool status: `{risk.get('tool_status')}`",  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        f"- Max score: `{risk.get('max_score')}`",  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "",
        "## Diagnostics",
    ]
    if diagnostics:
        lines.extend(
            f"- `{row.get('diagnostic_code')}` `{row.get('json_path', '$')}` `{row.get('severity')}`: {row.get('message')}"
            for row in diagnostics
        )
    else:
        lines.append("- None.")
    lines.extend(
        [
            "",
            "## Failure Modes",
            "- Filesystem: missing/malformed S05 and S06 artifacts, unsafe paths, stale hashes, and unreadable reports emit diagnostics with artifact_path/json_path and fail closed.",
            "- Network: intentionally absent; URL artifact paths are rejected and network safety flags remain false.",
            "- Subprocess/API: riskratchet uses the local Python wrapper; unavailability is warning telemetry, but absent/malformed telemetry artifacts fail validation.",
            "- Graph/database: intentionally absent; graph/import/LadybugDB flags remain false.",
            "",
            "## Load Profile",
            "At 10x the six-article corpus, local file hashing and report size saturate first and grow linearly; riskratchet remains a fixed explicit Python scope. Protection is safe relative path validation, local-only hashing, no network/database/graph writers, and diagnostic-only telemetry.",
            "",
            "## Negative Tests",
            "- `tests/test_m027_provenance_and_riskratchet_gate.py::test_gate_generates_happy_path_and_self_hash_exclusion` covers happy path generation, self-hash exclusion, provenance, safety flags, and maintainability outputs.",
            "- `tests/test_m027_provenance_and_riskratchet_gate.py::test_validate_only_reads_existing_outputs_without_rerunning_riskratchet` covers validate-only readback without rewriting/rerunning riskratchet.",
            "- `tests/test_m027_provenance_and_riskratchet_gate.py::test_validate_only_reports_missing_and_malformed_artifacts` covers missing/malformed JSON/JSONL artifacts.",
            "- `tests/test_m027_provenance_and_riskratchet_gate.py::test_gate_rejects_unsafe_flags_redaction_riskratchet_and_paths` covers unsafe safety flags, raw payload sentinel leakage, blocking/pass-fail riskratchet telemetry, invalid artifact paths, and stale output hashes.",
        ]
    )
    report = "\n".join(lines) + "\n"
    leakage = validate_no_payload_leakage({"report_text": report}, serialized=report, where=path)
    if leakage:
        raise RuntimeError(
            f"report leakage detected: {[row['diagnostic_code'] for row in leakage]}"
        )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(report, encoding="utf-8")


def finalize_output_provenance(args: argparse.Namespace, summary: dict[str, Any]) -> dict[str, Any]:
    rows = [
        artifact_row(Path(args.summary_output), role="summary", root=Path(args.root)),
        artifact_row(Path(args.diagnostics_output), role="diagnostics", root=Path(args.root)),
        artifact_row(Path(args.report_output), role="report", root=Path(args.root)),
        artifact_row(
            Path(args.maintainability_json), role="maintainability_json", root=Path(args.root)
        ),
        artifact_row(
            Path(args.maintainability_report), role="maintainability_report", root=Path(args.root)
        ),
    ]
    summary["provenance"]["output_artifacts"] = rows
    summary["provenance"]["exit_code"] = 0 if summary["status"] == "passed" else 1
    summary["provenance"]["exit_status"] = summary["status"]
    return summary


def generate(args: argparse.Namespace) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    s05_summary, s05_diagnostic_rows, diagnostics = validate_s05_artifacts(args)
    risk_report, risk_findings = run_riskratchet(args)
    diagnostics.extend(risk_findings)
    diagnostics.extend(
        validate_riskratchet_payload(risk_report, where=Path(args.maintainability_json))
    )
    if not Path(args.maintainability_json).exists():
        diagnostics.append(
            diagnostic(
                "missing_riskratchet_json",
                "riskratchet JSON output is missing",
                artifact_path=Path(args.maintainability_json),
                source="riskratchet_telemetry",
            )
        )
    if not Path(args.maintainability_report).exists():
        diagnostics.append(
            diagnostic(
                "missing_riskratchet_markdown",
                "riskratchet markdown output is missing",
                artifact_path=Path(args.maintainability_report),
                source="riskratchet_telemetry",
            )
        )
    summary = build_summary(args, s05_summary, s05_diagnostic_rows, risk_report, diagnostics)
    leakage = validate_no_payload_leakage(
        summary, serialized=json.dumps(summary, sort_keys=True), where=Path(args.summary_output)
    )
    diagnostics.extend(leakage)
    if leakage:
        summary["diagnostics"] = diagnostics
        summary["diagnostic_codes"] = sorted(
            {str(row.get("diagnostic_code")) for row in diagnostics if row.get("diagnostic_code")}
        )
        summary["status"] = "failed"
        summary["verification_result"] = "failed"
    write_json(Path(args.summary_output), summary)
    write_jsonl(Path(args.diagnostics_output), diagnostics)
    write_report(Path(args.report_output), summary)
    summary = finalize_output_provenance(args, summary)
    write_json(Path(args.summary_output), summary)
    return summary, diagnostics


def validate_existing(args: argparse.Namespace) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    diagnostics: list[dict[str, Any]] = []
    summary, findings = load_json(Path(args.summary_output))
    diagnostics.extend(findings)
    rows, findings = load_jsonl(Path(args.diagnostics_output))
    diagnostics.extend(findings)
    risk_report, findings = load_json(Path(args.maintainability_json))
    diagnostics.extend(findings)
    try:
        report_text = Path(args.report_output).read_text(encoding="utf-8")
    except FileNotFoundError:
        report_text = ""
        diagnostics.append(
            diagnostic(
                "missing_markdown_report",
                "S06 markdown report is missing",
                artifact_path=Path(args.report_output),
            )
        )
    except OSError as exc:
        report_text = ""
        diagnostics.append(
            diagnostic(
                "markdown_report_unreadable",
                f"S06 markdown report is unreadable: {exc}",
                artifact_path=Path(args.report_output),
            )
        )
    try:
        risk_markdown = Path(args.maintainability_report).read_text(encoding="utf-8")
    except FileNotFoundError:
        risk_markdown = ""
        diagnostics.append(
            diagnostic(
                "missing_riskratchet_markdown",
                "riskratchet markdown output is missing",
                artifact_path=Path(args.maintainability_report),
                source="riskratchet_telemetry",
            )
        )
    except OSError as exc:
        risk_markdown = ""
        diagnostics.append(
            diagnostic(
                "riskratchet_markdown_unreadable",
                f"riskratchet markdown output is unreadable: {exc}",
                artifact_path=Path(args.maintainability_report),
                source="riskratchet_telemetry",
            )
        )

    if summary is None:
        return {
            "schema_version": SCHEMA_VERSION,
            "status": "failed",
            "verification_result": "failed",
            "diagnostics": diagnostics,
            **FALSE_SAFETY_FLAGS,
        }, diagnostics
    diagnostics.extend(
        validate_no_payload_leakage(
            summary, serialized=json.dumps(summary, sort_keys=True), where=Path(args.summary_output)
        )
    )
    diagnostics.extend(
        validate_no_payload_leakage(
            rows, serialized=json.dumps(rows, sort_keys=True), where=Path(args.diagnostics_output)
        )
    )
    diagnostics.extend(
        validate_no_payload_leakage(
            {"report_text": report_text}, serialized=report_text, where=Path(args.report_output)
        )
    )
    diagnostics.extend(
        validate_no_payload_leakage(
            {"riskratchet_report_text": risk_markdown},
            serialized=risk_markdown,
            where=Path(args.maintainability_report),
        )
    )
    diagnostics.extend(false_flag_diagnostics(summary, where=Path(args.summary_output)))
    safety = summary.get("safety") if isinstance(summary.get("safety"), dict) else {}
    diagnostics.extend(
        false_flag_diagnostics(safety, where=Path(args.summary_output), json_prefix="$.safety")  # ty:ignore[invalid-argument-type]
    )
    expected = {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
    }
    for key, value in expected.items():
        if summary.get(key) != value:
            diagnostics.append(
                diagnostic(
                    "summary_contract_mismatch",
                    f"summary {key} is {summary.get(key)!r}, expected {value!r}",
                    artifact_path=Path(args.summary_output),
                    json_path=f"$.{key}",
                )
            )
    provenance = summary.get("provenance") if isinstance(summary.get("provenance"), dict) else {}
    if provenance.get("self_hash_excluded") is not True or not provenance.get(  # ty:ignore[unresolved-attribute]
        "self_hash_excluded_reason"
    ):
        diagnostics.append(
            diagnostic(
                "self_hash_exclusion_missing",
                "summary must explicitly document self_hash_excluded=true",
                artifact_path=Path(args.summary_output),
                json_path="$.provenance.self_hash_excluded",
                source="output_hash_validation",
            )
        )
    diagnostics.extend(
        validate_output_artifacts(
            summary, root=Path(args.root), summary_path=Path(args.summary_output)
        )
    )
    if risk_report is not None:
        diagnostics.extend(
            validate_riskratchet_payload(risk_report, where=Path(args.maintainability_json))
        )
    # Preserve read diagnostics and append fresh readback diagnostics for caller visibility.
    all_diagnostics = [*rows, *diagnostics]
    status = "passed" if not diagnostics else "failed"
    verifier_summary = {
        **summary,
        "status": status,
        "verification_result": status,
        "diagnostics": all_diagnostics,
        "diagnostic_codes": sorted(
            {
                str(row.get("diagnostic_code"))
                for row in all_diagnostics
                if row.get("diagnostic_code")
            }
        ),
        **FALSE_SAFETY_FLAGS,
    }
    return verifier_summary, diagnostics


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--validate-only",
        action="store_true",
        help="Read existing S06 artifacts without rerunning riskratchet or rewriting outputs.",
    )
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--s05-summary", type=Path, default=S05_SUMMARY_PATH)
    parser.add_argument("--s05-diagnostics", type=Path, default=S05_DIAGNOSTICS_PATH)
    parser.add_argument("--s05-events", type=Path, default=S05_EVENTS_PATH)
    parser.add_argument("--s05-readiness-decision", type=Path, default=S05_DECISION_PATH)
    parser.add_argument("--s05-verification", type=Path, default=S05_VERIFICATION_PATH)
    parser.add_argument("--output-dir", type=Path, default=GATE_DIR)
    parser.add_argument("--summary-output", type=Path, default=SUMMARY_PATH)
    parser.add_argument("--diagnostics-output", type=Path, default=DIAGNOSTICS_PATH)
    parser.add_argument("--report-output", type=Path, default=REPORT_PATH)
    parser.add_argument("--maintainability-json", type=Path, default=MAINTAINABILITY_JSON_PATH)
    parser.add_argument("--maintainability-report", type=Path, default=MAINTAINABILITY_REPORT_PATH)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.validate_only:
        summary, diagnostics = validate_existing(args)
    else:
        summary, diagnostics = generate(args)
    if diagnostics:
        sys.stderr.write(
            json.dumps(
                {
                    "summary": {
                        "status": summary.get("status"),
                        "verification_result": summary.get("verification_result"),
                    },
                    "diagnostics": diagnostics,
                },
                indent=2,
                sort_keys=True,
            )
            + "\n"
        )
        return 1
    sys.stdout.write(
        json.dumps(
            {
                "status": summary.get("status"),
                "verification_result": summary.get("verification_result"),
                "summary_path": str(args.summary_output),
            },
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
