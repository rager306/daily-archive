# Formerly: src/arxiv_archive/validation_batch_provenance.py

"""Commit-safe provenance and freshness helpers for validation-batch runs.

This module records file hashes and command metadata only. It never serializes
file contents, raw paper text, chunk text, embeddings, vectors, or secrets.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
from collections.abc import Sequence
from datetime import UTC, datetime
from pathlib import Path
from typing import Any
from uuid import uuid4

from research_graph.application.validation.batch_state import SAFETY_FLAG_KEYS, default_safety_flags

SCHEMA_VERSION = "validation-cli-provenance.v1"
FRESHNESS_SCHEMA_VERSION = "artifact-freshness-report.v1"
_SECRET_MARKERS = ("key", "token", "secret", "password", "credential")


def utc_now_iso() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


def sha256_file(path: str | Path) -> str:
    digest = hashlib.sha256()
    with Path(path).open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return f"sha256:{digest.hexdigest()}"


def fingerprint_file(path: str | Path) -> dict[str, Any]:
    file_path = Path(path)
    stat = file_path.stat()
    return {
        "path": str(file_path),
        "sha256": sha256_file(file_path),
        "size_bytes": stat.st_size,
        "mtime_ns": stat.st_mtime_ns,
        "exists": True,
    }


def _missing_fingerprint(path: str | Path) -> dict[str, Any]:
    return {
        "path": str(Path(path)),
        "sha256": None,
        "size_bytes": None,
        "mtime_ns": None,
        "exists": False,
    }


def redact_cli_args(argv: Sequence[str]) -> list[str]:
    redacted: list[str] = []
    redact_next = False
    for arg in argv:
        if redact_next:
            redacted.append("<redacted>")
            redact_next = False
            continue
        lowered = arg.lower()
        if arg.startswith("--") and "=" in arg:
            flag, _value = arg.split("=", 1)
            if _is_secret_flag(flag):
                redacted.append(f"{flag}=<redacted>")
            else:
                redacted.append(arg)
            continue
        redacted.append(arg)
        if arg.startswith("--") and _is_secret_flag(lowered):
            redact_next = True
    return redacted


def current_git_commit(cwd: str | Path | None = None) -> str | None:
    try:
        result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=str(cwd) if cwd is not None else None,
            check=False,
            capture_output=True,
            text=True,
            timeout=5,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if result.returncode != 0:
        return None
    commit = result.stdout.strip()
    return commit or None


def build_validation_cli_provenance_entry(
    *,
    command: str,
    argv: Sequence[str],
    batch_id: str,
    input_paths: Sequence[str | Path],
    output_paths: Sequence[str | Path],
    status: str,
    started_at: str | datetime,
    completed_at: str | datetime,
    exit_code: int = 0,
    cwd: str | Path | None = None,
    run_id: str | None = None,
    stdout_path: str | Path | None = None,
    stderr_path: str | Path | None = None,
    real_source_acquisition_performed: bool = False,
    real_scan_performed: bool = False,
    expected_artifact_metadata: dict[str, Any] | None = None,
) -> dict[str, Any]:
    started = _isoformat_utc(started_at)
    completed = _isoformat_utc(completed_at)
    started_dt = _parse_iso_datetime(started)
    completed_dt = _parse_iso_datetime(completed)
    duration_ms = max(int((completed_dt - started_dt).total_seconds() * 1000), 0)
    entry = {
        "schema_version": SCHEMA_VERSION,
        "run_id": run_id or f"{started}-{uuid4().hex[:8]}",
        "batch_id": batch_id,
        "command": command,
        "argv": redact_cli_args(argv),
        "cwd": str(Path(cwd or Path.cwd())),
        "git_commit": current_git_commit(cwd),
        "started_at": started,
        "completed_at": completed,
        "duration_ms": duration_ms,
        "status": status,
        "exit_code": exit_code,
        "inputs": [fingerprint_file(path) for path in input_paths],
        "outputs": [fingerprint_file(path) for path in output_paths],
        "stdout_path": str(stdout_path) if stdout_path is not None else None,
        "stderr_path": str(stderr_path) if stderr_path is not None else None,
        "real_source_acquisition_performed": real_source_acquisition_performed,
        "real_scan_performed": real_scan_performed,
        "expected_artifact_metadata": dict(expected_artifact_metadata or {}),
        **default_safety_flags(),
    }
    return entry


def append_validation_cli_provenance(log_path: str | Path, entry: dict[str, Any]) -> Path:
    path = Path(log_path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(entry, sort_keys=True, separators=(",", ":")) + "\n")
    return path


def read_validation_cli_provenance_log(path: str | Path) -> list[dict[str, Any]]:
    entries: list[dict[str, Any]] = []
    for line_number, line in enumerate(
        Path(path).read_text(encoding="utf-8").splitlines(), start=1
    ):
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError as exc:
            raise ValueError(f"invalid provenance JSON on line {line_number}: {exc.msg}") from exc
        if not isinstance(payload, dict):
            raise ValueError(f"invalid provenance entry on line {line_number}: expected object")
        entries.append(payload)
    return entries


def select_provenance_entry(
    entries: Sequence[dict[str, Any]],
    *,
    run_id: str | None = None,
    batch_id: str | None = None,
    command: str | None = None,
) -> dict[str, Any]:
    matches = list(entries)
    if run_id is not None:
        matches = [entry for entry in matches if entry.get("run_id") == run_id]
    if batch_id is not None:
        matches = [entry for entry in matches if entry.get("batch_id") == batch_id]
    if command is not None:
        matches = [entry for entry in matches if entry.get("command") == command]
    if not matches:
        raise ValueError("no_matching_run")
    matches.sort(key=lambda entry: str(entry.get("completed_at") or ""), reverse=True)
    if len(matches) > 1 and matches[0].get("completed_at") == matches[1].get("completed_at"):
        raise ValueError("ambiguous_run")
    return matches[0]


def build_artifact_freshness_report(entry: dict[str, Any]) -> dict[str, Any]:
    diagnostics: list[dict[str, Any]] = []
    diagnostics.extend(_validate_entry_shape(entry))
    diagnostics.extend(_validate_safety(entry))
    input_current, input_diagnostics = _verify_fingerprints(entry.get("inputs", []), role="input")
    output_current, output_diagnostics = _verify_fingerprints(
        entry.get("outputs", []), role="output"
    )
    diagnostics.extend(input_diagnostics)
    diagnostics.extend(output_diagnostics)
    diagnostics.extend(
        _verify_expected_artifact_metadata(
            entry.get("outputs", []), entry.get("expected_artifact_metadata", {})
        )
    )
    missing_count = sum(1 for item in diagnostics if item["code"].startswith("missing_"))
    mismatch_count = sum(
        1
        for item in diagnostics
        if item["code"].endswith("_changed") or item["code"] == "artifact_metadata_mismatch"
    )
    if any(item["code"] in {"invalid_provenance", "unsafe_safety_flag"} for item in diagnostics):
        verdict = "invalid_provenance"
    elif missing_count:
        verdict = "missing"
    elif mismatch_count:
        verdict = "stale"
    else:
        verdict = "fresh"
    return {
        "schema_version": FRESHNESS_SCHEMA_VERSION,
        "run_id": entry.get("run_id"),
        "batch_id": entry.get("batch_id"),
        "command": entry.get("command"),
        "verdict": verdict,
        "checked_at": utc_now_iso(),
        "expected_input_count": len(entry.get("inputs", []) or []),
        "expected_output_count": len(entry.get("outputs", []) or []),
        "matched_input_count": sum(1 for item in input_current if item.get("exists")),
        "matched_output_count": sum(1 for item in output_current if item.get("exists")),
        "missing_count": missing_count,
        "mismatch_count": mismatch_count,
        "diagnostics": diagnostics,
        **default_safety_flags(),
    }


def write_artifact_freshness_report(report: dict[str, Any], path: str | Path) -> Path:
    output_path = Path(path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return output_path


def _verify_fingerprints(
    expected: Sequence[dict[str, Any]] | Any, *, role: str
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    current: list[dict[str, Any]] = []
    diagnostics: list[dict[str, Any]] = []
    if isinstance(expected, (str, bytes)) or not isinstance(expected, Sequence):
        diagnostics.append(
            _diagnostic(
                "blocker",
                "invalid_provenance",
                f"Recorded {role} fingerprints are not a list.",
                "Regenerate provenance with the current schema.",
            )
        )
        return current, diagnostics
    for index, expected_item in enumerate(expected):
        if not isinstance(expected_item, dict):
            diagnostics.append(
                _diagnostic(
                    "blocker",
                    "invalid_provenance",
                    f"Recorded {role} fingerprint at index {index} is not an object.",
                    "Regenerate provenance with the current schema.",
                )
            )
            continue
        path = expected_item.get("path")
        if not path or not Path(path).exists():
            current.append(_missing_fingerprint(path or ""))
            diagnostics.append(
                _diagnostic(
                    "blocker",
                    f"missing_{role}",
                    f"Recorded {role} path is missing.",
                    "Re-run the producing command or restore the file.",
                    path,
                )
            )
            continue
        actual = fingerprint_file(path)
        current.append(actual)
        if actual["sha256"] != expected_item.get("sha256"):
            diagnostics.append(
                _diagnostic(
                    "blocker",
                    f"{role}_hash_changed",
                    f"Recorded {role} hash no longer matches current file hash.",
                    "Re-run the producing command or investigate manual artifact mutation.",
                    path,
                )
            )
        if actual["size_bytes"] != expected_item.get("size_bytes"):
            diagnostics.append(
                _diagnostic(
                    "blocker",
                    f"{role}_size_changed",
                    f"Recorded {role} size no longer matches current file size.",
                    "Re-run the producing command or investigate manual artifact mutation.",
                    path,
                )
            )
    return current, diagnostics


def _verify_expected_artifact_metadata(
    outputs: Sequence[dict[str, Any]] | Any, expected_metadata: dict[str, Any]
) -> list[dict[str, Any]]:
    if not expected_metadata:
        return []
    if isinstance(outputs, (str, bytes)) or not isinstance(outputs, Sequence):
        return []
    diagnostics: list[dict[str, Any]] = []
    for output in outputs:
        if not isinstance(output, dict):
            continue
        path = output.get("path")
        if not path or not Path(path).exists():
            continue
        try:
            payload = json.loads(Path(path).read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            diagnostics.append(
                _diagnostic(
                    "blocker",
                    "artifact_metadata_unreadable",
                    "Recorded output is not readable JSON for metadata verification.",
                    "Regenerate the output artifact or remove metadata expectations for non-JSON outputs.",
                    path,
                )
            )
            continue
        if not isinstance(payload, dict):
            diagnostics.append(
                _diagnostic(
                    "blocker",
                    "artifact_metadata_unreadable",
                    "Recorded output JSON is not an object for metadata verification.",
                    "Regenerate the output artifact or remove metadata expectations for non-object outputs.",
                    path,
                )
            )
            continue
        for key, expected_value in expected_metadata.items():
            if payload.get(key) != expected_value:
                diagnostics.append(
                    _diagnostic(
                        "blocker",
                        "artifact_metadata_mismatch",
                        f"Recorded output metadata {key!r} does not match expected value.",
                        "Re-run the producing command with active lineage metadata or investigate stale artifacts.",
                        path,
                    )
                )
    return diagnostics


def _validate_entry_shape(entry: dict[str, Any]) -> list[dict[str, Any]]:
    required = ("schema_version", "run_id", "batch_id", "command", "inputs", "outputs", "exit_code")
    diagnostics = []
    for key in required:
        if key not in entry:
            diagnostics.append(
                _diagnostic(
                    "blocker",
                    "invalid_provenance",
                    f"Provenance entry is missing required key {key!r}.",
                    "Regenerate provenance with the current schema.",
                )
            )
    if entry.get("schema_version") != SCHEMA_VERSION:
        diagnostics.append(
            _diagnostic(
                "blocker",
                "invalid_provenance",
                "Provenance entry schema version is not supported.",
                "Regenerate provenance with the current schema.",
            )
        )
    if entry.get("exit_code") != 0:
        diagnostics.append(
            _diagnostic(
                "blocker",
                "invalid_provenance",
                "Recorded command exit code was nonzero.",
                "Do not trust artifacts from failed commands.",
            )
        )
    return diagnostics


def _validate_safety(entry: dict[str, Any]) -> list[dict[str, Any]]:
    diagnostics = []
    for key in SAFETY_FLAG_KEYS:
        if entry.get(key, False) is True:
            diagnostics.append(
                _diagnostic(
                    "blocker",
                    "unsafe_safety_flag",
                    f"Provenance safety flag {key} is true.",
                    "Remove unsafe artifact content and rerun with redacted outputs.",
                )
            )
    return diagnostics


def _diagnostic(
    severity: str, code: str, message: str, recommended_action: str, path: str | None = None
) -> dict[str, Any]:
    diagnostic = {
        "severity": severity,
        "code": code,
        "message": message,
        "recommended_action": recommended_action,
    }
    if path is not None:
        diagnostic["path"] = str(path)
    return diagnostic


def _is_secret_flag(flag: str) -> bool:
    lowered = flag.lower()
    return any(marker in lowered for marker in _SECRET_MARKERS)


def _isoformat_utc(value: str | datetime) -> str:
    if isinstance(value, str):
        return value
    if value.tzinfo is None:
        value = value.replace(tzinfo=UTC)
    return value.astimezone(UTC).isoformat().replace("+00:00", "Z")


def _parse_iso_datetime(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))
