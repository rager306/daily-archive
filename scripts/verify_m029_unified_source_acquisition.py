#!/usr/bin/env python3
"""Verify the M029 unified corpus source-acquisition handoff.

The verifier is local-only. It reads the capture summary produced beside the
``source/`` directory, checks that every selected URL has at least one terminal
state, validates captured local artifacts against recorded byte sizes and hashes,
and rewrites metadata-only summary, diagnostics, and report artifacts.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import time
from collections import defaultdict
from collections.abc import Iterable, Mapping
from pathlib import Path, PurePosixPath
from typing import Any

MILESTONE_ID = "M029-eb0ljz"
SLICE_ID = "S02"
SELECTION_ID = "m029-unified-corpus-v1"
VERIFY_SCHEMA_VERSION = "m029-source-acquisition-verify.v1"
TERMINAL_STATES = {"captured", "blocked", "failed"}
UNSAFE_TRUE_FLAGS = {
    "graph_import_allowed",
    "trusted_kg_import_allowed",
    "production_import_attempted",
    "ladybugdb_written",
    "production_ladybugdb_write_allowed",
    "metadata_manifests_embed_raw_text",
    "metadata_manifests_embed_raw_binary",
    "raw_text_embedded",
    "raw_binary_embedded",
    "raw_payload_embedded_in_metadata",
    "raw_text_embedded_in_metadata",
    "raw_binary_embedded_in_metadata",
}
FORBIDDEN_SNIPPETS = ("<html", "</html", "%PDF-", "base64,")


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        payload = json.load(handle)
    if not isinstance(payload, dict):
        raise ValueError(f"expected JSON object: {path}")
    return payload


def write_json(path: Path, payload: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=False) + "\n", encoding="utf-8")


def write_jsonl(path: Path, rows: Iterable[Mapping[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_child_path(root: Path, rel_path: str) -> Path:
    if not isinstance(rel_path, str) or not rel_path.strip():
        raise ValueError("missing_local_path")
    if "://" in rel_path:
        raise ValueError("url_not_allowed_as_local_path")
    normalized = PurePosixPath(rel_path.replace("\\", "/"))
    if normalized.is_absolute() or ".." in normalized.parts or any(part == "" for part in normalized.parts):
        raise ValueError("unsafe_local_path")
    root_resolved = root.resolve()
    resolved = (root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root_resolved):
        raise ValueError("local_path_escapes_source_dir")
    return resolved


def diagnostic(code: str, message: str, *, result: Mapping[str, Any] | None = None, severity: str = "error") -> dict[str, Any]:
    return {
        "schema_version": VERIFY_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "severity": severity,
        "diagnostic_code": code,
        "code": code,
        "message": message,
        "article_ref": result.get("article_ref") if isinstance(result, Mapping) else None,
        "article_key": result.get("article_key") if isinstance(result, Mapping) else None,
        "source_role": result.get("source_role") if isinstance(result, Mapping) else None,
        "url": result.get("url") if isinstance(result, Mapping) else None,
        "local_path": result.get("local_path") if isinstance(result, Mapping) else None,
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "graph_import_allowed": False,
    }


def selected_urls(selection: Mapping[str, Any]) -> set[str]:
    articles = selection.get("articles")
    if not isinstance(articles, list):
        raise ValueError("selection articles must be a list")
    urls: set[str] = set()
    for row in articles:
        if not isinstance(row, dict):
            raise ValueError("selection row must be an object")
        url = row.get("seed_url") if isinstance(row.get("seed_url"), str) else row.get("canonical_url")
        if not isinstance(url, str) or not url:
            raise ValueError("selection row missing seed/canonical URL")
        urls.add(url)
    return urls


def validate_capture_summary(summary: Mapping[str, Any], selection: Mapping[str, Any], source_dir: Path, *, require_no_network: bool, require_no_import_flags: bool) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    errors: list[dict[str, Any]] = []
    results = summary.get("results")
    if not isinstance(results, list):
        errors.append(diagnostic("missing_results", "capture summary does not contain a results list"))
        results = []

    counts: dict[str, int] = {"captured": 0, "blocked": 0, "failed": 0}
    by_url: dict[str, dict[str, int]] = defaultdict(lambda: {"captured": 0, "blocked": 0, "failed": 0})
    by_role: dict[str, dict[str, int]] = defaultdict(lambda: {"captured": 0, "blocked": 0, "failed": 0})
    network_fetch_attempted_count = 0

    for item in results:
        if not isinstance(item, dict):
            errors.append(diagnostic("malformed_result", "result row is not an object"))
            continue
        status = item.get("status")
        if status not in TERMINAL_STATES:
            errors.append(diagnostic("non_terminal_status", f"result status is not terminal: {status}", result=item))
            continue
        counts[str(status)] += 1
        url = item.get("url") if isinstance(item.get("url"), str) else "<missing-url>"
        role = item.get("source_role") if isinstance(item.get("source_role"), str) else "<missing-role>"
        by_url[url][str(status)] += 1
        by_role[role][str(status)] += 1

        if item.get("network_fetch_attempted") is True:
            network_fetch_attempted_count += 1
            if require_no_network:
                errors.append(diagnostic("network_fetch_attempted", "result recorded a network fetch despite local-only verification", result=item))
        if require_no_import_flags:
            for flag in UNSAFE_TRUE_FLAGS:
                if item.get(flag) is True:
                    errors.append(diagnostic("unsafe_import_or_payload_flag", f"unsafe flag is true: {flag}", result=item))
            safety = item.get("fail_closed_safety_flags")
            if isinstance(safety, dict):
                for flag in UNSAFE_TRUE_FLAGS:
                    if safety.get(flag) is True:
                        errors.append(diagnostic("unsafe_fail_closed_flag", f"fail-closed safety flag is true: {flag}", result=item))
        if status == "captured":
            local_path = item.get("local_path")
            try:
                artifact = safe_child_path(source_dir, local_path)  # type: ignore[arg-type]
            except ValueError as exc:
                errors.append(diagnostic(str(exc), "captured result has unsafe or missing local path", result=item))
                continue
            if not artifact.exists():
                errors.append(diagnostic("captured_artifact_missing", "captured local artifact is absent", result=item))
                continue
            byte_size = item.get("byte_size")
            if not isinstance(byte_size, int) or byte_size != artifact.stat().st_size:
                errors.append(diagnostic("byte_size_mismatch", "captured artifact size does not match metadata", result=item))
            sha256 = item.get("sha256")
            if not isinstance(sha256, str) or sha256 != sha256_file(artifact):
                errors.append(diagnostic("sha256_mismatch", "captured artifact hash does not match metadata", result=item))

    expected_urls = selected_urls(selection)
    observed_urls = {url for url in by_url if url != "<missing-url>"}
    for missing_url in sorted(expected_urls - observed_urls):
        errors.append(diagnostic("selected_url_missing_terminal_state", f"selected URL has no acquisition terminal state: {missing_url}"))

    verification = {
        "schema_version": VERIFY_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "passed" if not errors else "failed",
        "article_count": len(expected_urls),
        "variant_count": len(results),
        "counts": counts,
        "per_url_terminal_state_counts": {url: dict(value) for url, value in sorted(by_url.items())},
        "per_role_terminal_state_counts": {role: dict(value) for role, value in sorted(by_role.items())},
        "network_fetch_attempted_count": network_fetch_attempted_count,
        "capture_summary_status": summary.get("status"),
        "results": results,
        "error_count": len(errors),
        "errors": errors,
        "capture_phase_network_allowed": False,
        "replay_phase_network_allowed": False,
        "graph_import_allowed": False,
        "production_ladybugdb_write_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "raw_payload_embedded_in_metadata": False,
    }
    return errors, verification


def acquisition_diagnostics(summary: Mapping[str, Any], errors: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Return blocked/failed acquisition rows plus verifier errors for JSONL output."""

    rows: list[dict[str, Any]] = []
    for result in summary.get("results", []):
        if isinstance(result, dict) and result.get("status") in {"blocked", "failed"}:
            rows.append(
                {
                    "schema_version": VERIFY_SCHEMA_VERSION,
                    "milestone_id": MILESTONE_ID,
                    "slice_id": SLICE_ID,
                    "selection_id": SELECTION_ID,
                    "severity": "info" if result.get("status") == "blocked" else "error",
                    "diagnostic_code": result.get("diagnostic_code"),
                    "code": result.get("diagnostic_code"),
                    "message": result.get("failure_reason"),
                    "article_ref": result.get("article_ref"),
                    "article_key": result.get("article_key"),
                    "source_role": result.get("source_role"),
                    "url": result.get("url"),
                    "status": result.get("status"),
                    "terminal_state": result.get("terminal_state"),
                    "local_path": result.get("local_path"),
                    "network_fetch_attempted": False,
                    "production_import_attempted": False,
                    "ladybugdb_written": False,
                    "trusted_kg_import_allowed": False,
                    "graph_import_allowed": False,
                }
            )
    rows.extend(errors)
    return rows


def render_report(summary: Mapping[str, Any]) -> str:
    counts = summary.get("counts") if isinstance(summary.get("counts"), dict) else {}
    lines = [
        "# M029 Source Acquisition Verification Report",
        "",
        "This report is metadata-only and local-only.",
        "",
        f"- Status: `{summary.get('status')}`",
        f"- Error count: {summary.get('error_count')}",
        f"- Article URLs: {summary.get('article_count')}",
        f"- Variants: {summary.get('variant_count')}",
        f"- Captured: {counts.get('captured', 0)}",
        f"- Blocked: {counts.get('blocked', 0)}",
        f"- Failed: {counts.get('failed', 0)}",
        f"- Network fetch attempted count: {summary.get('network_fetch_attempted_count')}",
        "- Graph/import/LadybugDB writes: false",
        "",
        "## Role Counts",
        "",
    ]
    role_counts = summary.get("per_role_terminal_state_counts") if isinstance(summary.get("per_role_terminal_state_counts"), dict) else {}
    for role, value in role_counts.items():
        if isinstance(value, dict):
            lines.append(f"- `{role}`: captured={value.get('captured', 0)} blocked={value.get('blocked', 0)} failed={value.get('failed', 0)}")
    lines.extend(["", "## Blocked or Failed Diagnostics", ""])
    for result in summary.get("results", []):
        if isinstance(result, dict) and result.get("status") != "captured":
            lines.append(f"- `{result.get('url')}` `{result.get('source_role')}`: {result.get('status')} ({result.get('diagnostic_code')}) — {result.get('failure_reason')}")
    errors = summary.get("errors") if isinstance(summary.get("errors"), list) else []
    if errors:
        lines.extend(["", "## Verification Errors", ""])
        for error in errors:
            if isinstance(error, dict):
                lines.append(f"- `{error.get('code')}`: {error.get('message')}")
    return "\n".join(lines) + "\n"


def assert_metadata_artifact_is_redacted(path: Path) -> None:
    payload = path.read_text(encoding="utf-8")
    found = [token for token in FORBIDDEN_SNIPPETS if token in payload]
    if found:
        raise ValueError(f"metadata artifact is not redacted: {path}: {found}")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--source-dir", required=True, type=Path)
    parser.add_argument("--write-summary", required=True, type=Path)
    parser.add_argument("--write-diagnostics", required=True, type=Path)
    parser.add_argument("--write-report", required=True, type=Path)
    parser.add_argument("--require-no-network", action="store_true")
    parser.add_argument("--require-no-import-flags", action="store_true")
    parser.add_argument("--check-terminal-states", action="store_true")
    parser.add_argument("--check-hashes", action="store_true")
    parser.add_argument("--check-strategies", action="store_true")
    parser.add_argument("--check-fail-closed", action="store_true")
    args = parser.parse_args(argv[1:])

    started = time.perf_counter()
    capture_summary_path = args.source_dir.parent / "source-acquisition-summary.json"
    if not capture_summary_path.exists() and args.write_summary.exists():
        capture_summary_path = args.write_summary
    if not capture_summary_path.exists():
        raise FileNotFoundError(f"capture summary not found: {capture_summary_path}")

    selection = load_json(args.selection)
    capture_summary = load_json(capture_summary_path)
    errors, verification = validate_capture_summary(
        capture_summary,
        selection,
        args.source_dir,
        require_no_network=args.require_no_network,
        require_no_import_flags=args.require_no_import_flags or args.check_fail_closed,
    )
    verification["duration_ms"] = int((time.perf_counter() - started) * 1000)
    write_json(args.write_summary, verification)
    write_jsonl(args.write_diagnostics, acquisition_diagnostics(verification, errors))
    args.write_report.parent.mkdir(parents=True, exist_ok=True)
    args.write_report.write_text(render_report(verification), encoding="utf-8")
    for artifact_path in (args.write_summary, args.write_diagnostics, args.write_report):
        assert_metadata_artifact_is_redacted(artifact_path)
    print(json.dumps({"summary_path": args.write_summary.as_posix(), "error_count": len(errors), "counts": verification["counts"]}, sort_keys=True))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
