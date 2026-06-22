#!/usr/bin/env python3
"""Replay M031 loader evidence only for already-captured local artifacts.

This command consumes the T01 M031 selection contract and T02 acquisition
summary. It invokes the local article loader only for acquisition rows whose
terminal state is ``captured`` and converts every non-captured acquisition row
into deterministic loader-blocker evidence. All emitted artifacts are
metadata-only: loader text is reduced to a boolean ``text_present`` signal and
no graph/import/LadybugDB readiness is claimed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import tempfile
import time
from collections import Counter, defaultdict
from collections.abc import Iterable, Mapping
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

from research_graph.infrastructure.corpus.ingestion.loader import (
    ArticleLoadSource,
    load_article_source,
)

MILESTONE_ID = "M031-vwpd8e"
SLICE_ID = "S02"
SELECTION_ID = "m031-catalog-backed-replay-v1"
SCHEMA_VERSION = "m031-catalog-backed-loader-evidence.v1"

FAIL_CLOSED_SAFETY_FLAGS: dict[str, bool] = {
    "metadata_only_loader_evidence": True,
    "network_fetch_attempted": False,
    "raw_article_text_embedded": False,
    "raw_article_html_embedded": False,
    "raw_pdf_bytes_embedded": False,
    "binary_payload_embedded": False,
    "base64_payload_embedded": False,
    "parser_ready_claimed": False,
    "chunk_ready_claimed": False,
    "kg_readiness_claimed": False,
    "graph_import_allowed": False,
    "production_ladybugdb_write_allowed": False,
    "trusted_kg_import_allowed": False,
    "production_import_attempted": False,
    "ladybugdb_written": False,
    "graph_write_attempted": False,
    "production_persistence_attempted": False,
}

FORBIDDEN_OUTPUT_KEYS = {
    "text",
    "raw_text",
    "html",
    "raw_html",
    "pdf",
    "binary",
    "bytes",
    "base64",
    "payload",
    "content",
    "body",
}
FORBIDDEN_OUTPUT_SNIPPETS = ("<html", "</html", "%PDF-", "base64,")
SAFE_TEXT_KEYS = {
    "media_type",
    "source_type",
    "source_role",
    "parser_name",
    "diagnostic_code",
    "blocker_code",
    "failure_reason",
    "local_path",
    "article_ref",
    "source_path",
    "event_path",
}


class LoaderEvidenceError(ValueError):
    """Typed validation error for deterministic CLI diagnostics."""

    def __init__(self, code: str, message: str, *, identity: str | None = None, article_ref: str | None = None):
        super().__init__(message)
        self.code = code
        self.identity = identity
        self.article_ref = article_ref


class RedactedLoaderEventLogger:
    """Write loader events with source paths confined to the replay source tree."""

    def __init__(self, log_path: Path, *, source_dir: Path) -> None:
        self.log_path = Path(log_path)
        self.source_dir = Path(source_dir).resolve()
        self.log_path.parent.mkdir(parents=True, exist_ok=True)

    def emit_article_event(self, payload: dict[str, Any]) -> dict[str, Any]:
        safe_payload = dict(payload)
        source_path = safe_payload.get("source_path")
        if isinstance(source_path, str):
            resolved = Path(source_path).resolve()
            if not resolved.is_relative_to(self.source_dir):
                raise LoaderEvidenceError("loader_event_source_path_escape", f"loader event source path escapes source-dir: {source_path}")
            safe_payload["source_path"] = resolved.relative_to(self.source_dir).as_posix()
        with self.log_path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(safe_payload, ensure_ascii=False, sort_keys=True) + "\n")
        return safe_payload

    def close(self) -> None:
        return None


def utc_now() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


def load_json_object(path: Path) -> dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            payload = json.load(handle)
    except json.JSONDecodeError as exc:
        raise LoaderEvidenceError("malformed_json", f"malformed JSON at {path}: {exc}") from exc
    except OSError as exc:
        raise LoaderEvidenceError("json_read_failed", f"failed to read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise LoaderEvidenceError("malformed_json_object", f"expected JSON object at {path}")
    return payload


def atomic_write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    tmp_path = Path(tmp_name)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        tmp_path.replace(path)
    except Exception:
        tmp_path.unlink(missing_ok=True)
        raise


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


def safe_child_path(root: Path, rel_path: str, *, code: str = "unsafe_relative_path") -> Path:
    if not isinstance(rel_path, str) or not rel_path.strip():
        raise LoaderEvidenceError(code, f"empty unsafe relative path: {rel_path!r}")
    if "://" in rel_path:
        raise LoaderEvidenceError("url_not_allowed_as_local_path", f"URL cannot be used as a local path: {rel_path}")
    normalized = PurePosixPath(rel_path.replace("\\", "/"))
    if normalized.is_absolute() or ".." in normalized.parts or any(part in ("", ".") for part in normalized.parts):
        raise LoaderEvidenceError(code, f"unsafe relative path: {rel_path}")
    root_resolved = root.resolve()
    resolved = (root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root_resolved):
        raise LoaderEvidenceError(code, f"path escapes root: {rel_path}")
    return resolved


def safe_article_segment(article_ref: str | None, article_key: str | None) -> str:
    raw = article_ref or f"unresolved/{article_key or 'unknown'}"
    normalized = PurePosixPath(raw.replace("\\", "/"))
    parts = [part for part in normalized.parts if part not in ("", ".")]
    if not parts or normalized.is_absolute() or ".." in parts:
        return f"unsafe/{hashlib.sha256(raw.encode('utf-8')).hexdigest()[:16]}"
    return "/".join(parts)


def event_log_path(output_dir: Path, article_ref: str | None, article_key: str | None) -> tuple[Path, str]:
    rel_path = f"{safe_article_segment(article_ref, article_key)}/events.jsonl"
    target = safe_child_path(output_dir, rel_path, code="unsafe_event_output_path")
    return target, target.relative_to(output_dir.resolve()).as_posix()


def string_value(row: Mapping[str, Any], key: str) -> str | None:
    value = row.get(key)
    return value if isinstance(value, str) else None


def source_type_for_row(row: Mapping[str, Any]) -> str:
    media_type = (string_value(row, "media_type") or "").lower()
    local_path = string_value(row, "local_path") or ""
    suffix = Path(local_path).suffix.lower()
    if media_type == "application/pdf" or suffix == ".pdf":
        return "pdf"
    if media_type in {"text/html", "application/xhtml+xml"} or suffix in {".html", ".htm"}:
        return "html"
    if media_type in {"text/markdown", "text/x-markdown"} or suffix == ".md":
        return "markdown"
    if media_type.startswith("text/") or suffix == ".txt":
        return "text"
    return "auto"


def result_common(row: Mapping[str, Any], *, status: str, diagnostic_code: str, blocker_code: str | None, failure_reason: str | None) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "identity": string_value(row, "identity"),
        "requested_ref_id": string_value(row, "requested_ref_id"),
        "requested_url": string_value(row, "requested_url"),
        "article_ref": string_value(row, "article_ref"),
        "article_key": string_value(row, "article_key"),
        "article_path": string_value(row, "article_path"),
        "variant_id": string_value(row, "variant_id"),
        "source_role": string_value(row, "source_role"),
        "url": string_value(row, "url") or string_value(row, "requested_url"),
        "status": status,
        "terminal_state": status,
        "diagnostic_code": diagnostic_code,
        "blocker_code": blocker_code,
        "failure_reason": failure_reason,
        "local_path": string_value(row, "local_path"),
        "safe_local_paths": [string_value(row, "local_path")] if string_value(row, "local_path") else [],
        "acquisition_status": string_value(row, "status"),
        "acquisition_diagnostic_code": string_value(row, "diagnostic_code"),
        "acquisition_sha256": string_value(row, "sha256"),
        "acquisition_byte_size": row.get("byte_size") if isinstance(row.get("byte_size"), int) else None,
        "is_metadata_only": row.get("is_metadata_only") if isinstance(row.get("is_metadata_only"), bool) else None,
        "requires_conversion": row.get("requires_conversion") if isinstance(row.get("requires_conversion"), bool) else None,
        "network_fetch_attempted": False,
        "network_fetch_allowed": False,
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "raw_payload_embedded_in_metadata": False,
        "parser_ready_claimed": False,
        "chunk_ready_claimed": False,
        "kg_readiness_claimed": False,
        "graph_import_allowed": False,
        "production_ladybugdb_write_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_write_attempted": False,
        "production_persistence_attempted": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
    }


def loader_blocker_from_acquisition(row: Mapping[str, Any], *, diagnostic_code: str | None = None, failure_reason: str | None = None) -> dict[str, Any]:
    blocker = result_common(
        row,
        status="blocked",
        diagnostic_code=diagnostic_code or "acquisition_not_captured",
        blocker_code=string_value(row, "blocker_code") or string_value(row, "diagnostic_code") or "acquisition_not_captured",
        failure_reason=failure_reason or string_value(row, "failure_reason") or "acquisition row was not captured; loader not attempted",
    )
    blocker.update(
        {
            "loader_attempted": False,
            "loader_outcome": "not_loaded",
            "source_id": None,
            "source_type": None,
            "media_type": string_value(row, "media_type"),
            "parser_name": None,
            "sha256": None,
            "byte_size": None,
            "duration_ms": 0,
            "warning_count": 0,
            "text_present": False,
            "event_path": None,
        }
    )
    return blocker


def loader_result_for_capture(row: Mapping[str, Any], *, source_dir: Path, output_dir: Path) -> dict[str, Any]:
    local_path = string_value(row, "local_path")
    if local_path is None:
        return loader_blocker_from_acquisition(row, diagnostic_code="captured_missing_local_path", failure_reason="captured row is missing local_path")
    try:
        source_path = safe_child_path(source_dir, local_path, code="unsafe_captured_source_path")
    except LoaderEvidenceError as exc:
        return loader_blocker_from_acquisition(row, diagnostic_code=exc.code, failure_reason=str(exc))

    if not source_path.exists():
        return loader_blocker_from_acquisition(row, diagnostic_code="captured_source_missing", failure_reason="captured artifact is absent from source-dir")
    actual_sha = sha256_file(source_path)
    expected_sha = string_value(row, "sha256")
    if expected_sha and actual_sha != expected_sha:
        return loader_blocker_from_acquisition(row, diagnostic_code="captured_hash_mismatch", failure_reason="captured artifact sha256 does not match acquisition summary")
    expected_size = row.get("byte_size") if isinstance(row.get("byte_size"), int) else None
    actual_size = source_path.stat().st_size
    if expected_size is not None and actual_size != expected_size:
        return loader_blocker_from_acquisition(row, diagnostic_code="captured_size_mismatch", failure_reason="captured artifact byte_size does not match acquisition summary")

    log_path, event_rel = event_log_path(output_dir, string_value(row, "article_ref"), string_value(row, "article_key"))
    result = load_article_source(
        ArticleLoadSource(source_path, paper_id=string_value(row, "article_key"), source_type=source_type_for_row(row)),
        logger=RedactedLoaderEventLogger(log_path, source_dir=source_dir),
    )
    diagnostic_code = "loader_loaded" if result.outcome == "loaded" else ("loader_loaded_metadata_only" if result.outcome == "loaded_metadata_only" else f"loader_{result.failure_reason or 'failed'}")
    status = "loaded" if result.outcome == "loaded" else ("loaded_metadata_only" if result.outcome == "loaded_metadata_only" else "failed")
    evidence = result_common(row, status=status, diagnostic_code=diagnostic_code, blocker_code=None, failure_reason=result.failure_reason)
    evidence.update(
        {
            "loader_attempted": True,
            "loader_outcome": result.outcome,
            "source_id": result.source_id,
            "source_type": result.source_type,
            "media_type": result.media_type,
            "parser_name": result.parser_name,
            "loader_name": result.loader_name,
            "sha256": result.sha256,
            "byte_size": result.byte_size,
            "duration_ms": result.duration_ms,
            "warning_count": result.warning_count,
            "text_present": result.text is not None,
            "event_path": event_rel,
        }
    )
    return evidence


def validate_selection_alignment(selection: Mapping[str, Any], acquisition: Mapping[str, Any]) -> None:
    if selection.get("selection_id") != acquisition.get("selection_id"):
        raise LoaderEvidenceError("selection_acquisition_mismatch", "selection_id mismatch between selection and acquisition summary")
    if acquisition.get("schema_version") != "m031-catalog-backed-acquisition.v1":
        raise LoaderEvidenceError("unexpected_acquisition_schema", "acquisition summary schema is not m031-catalog-backed-acquisition.v1")
    results = acquisition.get("results")
    if not isinstance(results, list):
        raise LoaderEvidenceError("malformed_acquisition_results", "acquisition summary results must be a list")


def replay_loader_evidence(
    *,
    selection_path: Path,
    acquisition_summary_path: Path,
    source_dir: Path,
    output_dir: Path,
) -> list[dict[str, Any]]:
    selection = load_json_object(selection_path)
    acquisition = load_json_object(acquisition_summary_path)
    validate_selection_alignment(selection, acquisition)
    output_dir.mkdir(parents=True, exist_ok=True)
    for event_file in output_dir.glob("**/events.jsonl"):
        event_file.unlink()

    rows: list[dict[str, Any]] = []
    for row in acquisition["results"]:
        if not isinstance(row, Mapping):
            raise LoaderEvidenceError("malformed_acquisition_result", "acquisition result rows must be objects")
        if string_value(row, "status") == "captured":
            rows.append(loader_result_for_capture(row, source_dir=source_dir, output_dir=output_dir))
        else:
            rows.append(loader_blocker_from_acquisition(row))
    return rows


def build_summary(
    rows: list[dict[str, Any]],
    *,
    selection_path: Path,
    acquisition_summary_path: Path,
    source_dir: Path,
    output_dir: Path,
    duration_ms: int,
) -> dict[str, Any]:
    outcomes = Counter(row["status"] for row in rows)
    counts = {
        "loader_attempted": sum(1 for row in rows if row.get("loader_attempted") is True),
        "loaded": outcomes.get("loaded", 0),
        "loaded_metadata_only": outcomes.get("loaded_metadata_only", 0),
        "failed": outcomes.get("failed", 0),
        "loader_blocked": outcomes.get("blocked", 0),
    }
    per_identity: dict[str, dict[str, int]] = defaultdict(lambda: {"loaded": 0, "loaded_metadata_only": 0, "failed": 0, "blocked": 0})
    per_role: dict[str, dict[str, int]] = defaultdict(lambda: {"loaded": 0, "loaded_metadata_only": 0, "failed": 0, "blocked": 0})
    for row in rows:
        identity = row.get("identity") if isinstance(row.get("identity"), str) else "<missing-identity>"
        role = row.get("source_role") if isinstance(row.get("source_role"), str) else "<missing-role>"
        status = str(row.get("status"))
        if status in per_identity[identity]:
            per_identity[identity][status] += 1
            per_role[role][status] += 1
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "completed_with_diagnostics" if counts["loader_blocked"] or counts["failed"] else "loaded",
        "loader_row_count": len(rows),
        "counts": counts,
        "per_identity_loader_state_counts": {key: dict(value) for key, value in sorted(per_identity.items())},
        "per_role_loader_state_counts": {key: dict(value) for key, value in sorted(per_role.items())},
        "results": rows,
        "input_paths": {
            "selection": selection_path.as_posix(),
            "acquisition_summary": acquisition_summary_path.as_posix(),
            "source_dir": source_dir.as_posix(),
        },
        "output_paths": {"loader_evidence_dir": output_dir.as_posix()},
        "duration_ms": duration_ms,
        "network_fetch_allowed": False,
        "network_fetch_attempted_count": 0,
        "graph_import_allowed": False,
        "production_ladybugdb_write_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_write_attempted": False,
        "production_persistence_attempted": False,
        "parser_ready_claimed": False,
        "chunk_ready_claimed": False,
        "kg_readiness_claimed": False,
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "raw_payload_embedded_in_metadata": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
        "generated_at": utc_now(),
    }


def render_report(summary: Mapping[str, Any]) -> str:
    counts = summary.get("counts") if isinstance(summary.get("counts"), Mapping) else {}
    lines = [
        "# M031 Catalog-Backed Loader Evidence Replay Report",
        "",
        "This report is metadata-only and local-only. It does not embed article text, HTML snippets, PDF bytes, or base64 payloads.",
        "",
        f"- Milestone: `{summary.get('milestone_id')}`",
        f"- Slice: `{summary.get('slice_id')}`",
        f"- Selection: `{summary.get('selection_id')}`",
        f"- Status: `{summary.get('status')}`",
        f"- Loader attempted: {counts.get('loader_attempted', 0)}",
        f"- Loaded: {counts.get('loaded', 0)}",
        f"- Loaded metadata only: {counts.get('loaded_metadata_only', 0)}",
        f"- Failed: {counts.get('failed', 0)}",
        f"- Loader blocked: {counts.get('loader_blocked', 0)}",
        "- Network fetch attempted count: 0",
        "- Graph/import/LadybugDB writes: false",
        "",
        "## Failure Modes",
        "",
        "- Malformed selection or acquisition JSON fails closed with a typed CLI diagnostic.",
        "- Selection/acquisition ID mismatch fails closed before loader calls.",
        "- Non-captured acquisition rows become loader blocker rows and are never passed to the loader.",
        "- Missing, unsafe, hash-mismatched, or size-mismatched captured files become loader blockers with stable diagnostics.",
        "- Loader failures are terminal evidence rows; they do not imply parser, chunk, graph, import, or LadybugDB readiness.",
        "- Network dependencies are deliberately absent: no fetch code path exists.",
        "",
        "## Load Profile",
        "",
        "The replay is bounded by captured acquisition rows and performs one local loader call per captured artifact. At 10x this four-ref corpus, disk reads and JSONL event volume grow linearly and saturate before CPU; there is no network, subprocess, graph write, or recursive catalog scan path.",
        "",
        "## Negative Tests",
        "",
        "Covered in `tests/test_m031_catalog_backed_acquisition_loader.py`: blocked acquisition rows not loaded, PDF metadata-only classification, missing captured file, acquisition hash mismatch, raw text redaction, unsafe loader event path, malformed acquisition shape, selection/acquisition mismatch, and true safety flag rejection.",
        "",
        "## Role Counts",
        "",
    ]
    role_counts = summary.get("per_role_loader_state_counts") if isinstance(summary.get("per_role_loader_state_counts"), Mapping) else {}
    for role, value in role_counts.items():
        if isinstance(value, Mapping):
            lines.append(
                f"- `{role}`: loaded={value.get('loaded', 0)} loaded_metadata_only={value.get('loaded_metadata_only', 0)} "
                f"failed={value.get('failed', 0)} blocked={value.get('blocked', 0)}"
            )
    lines.extend(["", "## Results", ""])
    for result in summary.get("results", []):
        if isinstance(result, Mapping):
            local_path = result.get("local_path") or "<none>"
            lines.append(
                f"- `{result.get('identity')}` `{result.get('source_role')}`: {result.get('status')} "
                f"({result.get('diagnostic_code')}) -> `{local_path}`; text_present={result.get('text_present')}"
            )
    return "\n".join(lines) + "\n"


def validate_output_metadata_only(payload: Any, *, path: str = "$", in_safe_key: bool = False) -> None:
    if isinstance(payload, Mapping):
        for key, value in payload.items():
            safe_key = key in SAFE_TEXT_KEYS
            if key in FORBIDDEN_OUTPUT_KEYS:
                raise LoaderEvidenceError("raw_payload_output_key", f"forbidden raw-payload output key at {path}.{key}")
            validate_output_metadata_only(value, path=f"{path}.{key}", in_safe_key=safe_key)
    elif isinstance(payload, list):
        for index, item in enumerate(payload):
            validate_output_metadata_only(item, path=f"{path}[{index}]", in_safe_key=in_safe_key)
    elif isinstance(payload, str):
        lowered = payload.lower()
        for snippet in FORBIDDEN_OUTPUT_SNIPPETS:
            if snippet.lower() in lowered and not in_safe_key:
                raise LoaderEvidenceError("raw_payload_output_snippet", f"forbidden raw-payload snippet at {path}")


def assert_metadata_artifact_is_redacted(path: Path) -> None:
    text = path.read_text(encoding="utf-8")
    lowered = text.lower()
    found = [token for token in FORBIDDEN_OUTPUT_SNIPPETS if token.lower() in lowered]
    if found:
        raise LoaderEvidenceError("raw_payload_artifact_snippet", f"metadata artifact is not redacted: {path}: {found}")


def assert_fail_closed_flags(summary: Mapping[str, Any]) -> None:
    flag_map = summary.get("fail_closed_safety_flags")
    if not isinstance(flag_map, Mapping):
        raise LoaderEvidenceError("missing_fail_closed_flags", "summary is missing fail_closed_safety_flags")
    for flag, expected in FAIL_CLOSED_SAFETY_FLAGS.items():
        if flag_map.get(flag) is not expected:
            raise LoaderEvidenceError("unsafe_safety_flag", f"unexpected safety flag {flag}={flag_map.get(flag)!r}")
    for row in summary.get("results", []):
        if not isinstance(row, Mapping):
            continue
        row_flags = row.get("fail_closed_safety_flags")
        if not isinstance(row_flags, Mapping):
            raise LoaderEvidenceError("missing_row_fail_closed_flags", "loader evidence row is missing fail_closed_safety_flags")
        for flag, expected in FAIL_CLOSED_SAFETY_FLAGS.items():
            if row_flags.get(flag) is not expected:
                raise LoaderEvidenceError("unsafe_safety_flag", f"unexpected row safety flag {flag}={row_flags.get(flag)!r}")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--acquisition-summary", required=True, type=Path)
    parser.add_argument("--source-dir", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--write-summary", required=True, type=Path)
    parser.add_argument("--write-diagnostics", required=True, type=Path)
    parser.add_argument("--write-report", required=True, type=Path)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    started = time.perf_counter()
    try:
        for cli_path in (args.selection, args.acquisition_summary, args.source_dir, args.output_dir, args.write_summary, args.write_diagnostics, args.write_report):
            if not cli_path.is_absolute() and ".." in PurePosixPath(str(cli_path).replace("\\", "/")).parts:
                raise LoaderEvidenceError("unsafe_cli_path", f"unsafe CLI path: {cli_path}")
        output_dir = args.output_dir.resolve()
        rows = replay_loader_evidence(
            selection_path=args.selection,
            acquisition_summary_path=args.acquisition_summary,
            source_dir=args.source_dir,
            output_dir=output_dir,
        )
        summary = build_summary(
            rows,
            selection_path=args.selection,
            acquisition_summary_path=args.acquisition_summary,
            source_dir=args.source_dir,
            output_dir=args.output_dir,
            duration_ms=int((time.perf_counter() - started) * 1000),
        )
        validate_output_metadata_only(summary)
        assert_fail_closed_flags(summary)
        report = render_report(summary)
        write_json(args.write_summary, summary)
        write_jsonl(args.write_diagnostics, rows)
        atomic_write_text(args.write_report, report)
        for artifact_path in (args.write_summary, args.write_diagnostics, args.write_report):
            assert_metadata_artifact_is_redacted(artifact_path)
        sys.stdout.write(json.dumps({"status": summary["status"], "counts": summary["counts"], "summary": args.write_summary.as_posix()}, sort_keys=True) + "\n")
        return 0 if summary["counts"]["failed"] == 0 else 1
    except LoaderEvidenceError as exc:
        sys.stderr.write(
            json.dumps(
                {
                    "status": "failed",
                    "code": exc.code,
                    "message": str(exc),
                    "identity": exc.identity,
                    "article_ref": exc.article_ref,
                },
                sort_keys=True,
            )
            + "\n"
        )
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
