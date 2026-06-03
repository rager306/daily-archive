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
STRATEGY_SCHEMA_VERSION = "m029-source-strategy-normalization.v1"
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


def _article_url(row: Mapping[str, Any]) -> str:
    url = row.get("seed_url") if isinstance(row.get("seed_url"), str) else row.get("canonical_url")
    if not isinstance(url, str) or not url:
        raise ValueError("selection row missing seed/canonical URL")
    return url


def _catalog_index_by_ref(index: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    articles = index.get("articles")
    if not isinstance(articles, list):
        raise ValueError("catalog index articles must be a list")
    by_ref: dict[str, Mapping[str, Any]] = {}
    for row in articles:
        if isinstance(row, dict) and isinstance(row.get("article_ref"), str):
            by_ref[row["article_ref"]] = row
    return by_ref


def _allowed_roles_by_source(catalog: Mapping[str, Any]) -> dict[str, set[str]]:
    allowed: dict[str, set[str]] = {}
    for source in catalog.get("sources", []):
        if not isinstance(source, dict) or not isinstance(source.get("source_code"), str):
            continue
        roles = source.get("allowed_source_roles")
        allowed[source["source_code"]] = {role for role in roles if isinstance(role, str)} if isinstance(roles, list) else set()
    return allowed


def normalize_source_strategies(
    selection: Mapping[str, Any],
    capture_summary: Mapping[str, Any],
    catalog: Mapping[str, Any],
    index: Mapping[str, Any],
) -> tuple[list[dict[str, Any]], dict[str, Any], list[dict[str, Any]]]:
    """Classify each selected article's source strategy and fallback state.

    The output is metadata-only: it joins selection intent, catalog/index strategy
    roles, and acquisition terminal states without embedding captured content.
    """

    articles = selection.get("articles")
    if not isinstance(articles, list):
        raise ValueError("selection articles must be a list")
    results = capture_summary.get("results")
    if not isinstance(results, list):
        results = []

    index_by_ref = _catalog_index_by_ref(index)
    allowed_roles = _allowed_roles_by_source(catalog)
    results_by_url: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    results_by_article_key: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    results_by_article_ref: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    for item in results:
        if not isinstance(item, dict):
            continue
        if isinstance(item.get("url"), str):
            results_by_url[item["url"]].append(item)
        if isinstance(item.get("article_key"), str):
            results_by_article_key[item["article_key"]].append(item)
        if isinstance(item.get("article_ref"), str):
            results_by_article_ref[item["article_ref"]].append(item)

    rows: list[dict[str, Any]] = []
    diagnostics: list[dict[str, Any]] = []
    counts = {
        "articles": 0,
        "resolved": 0,
        "unresolved": 0,
        "primary_captured": 0,
        "primary_blocked": 0,
        "primary_failed": 0,
        "primary_missing_terminal": 0,
        "fallback_needed": 0,
        "fallback_captured": 0,
        "strategy_mismatch": 0,
    }
    by_primary_role: dict[str, int] = defaultdict(int)
    by_capture_policy: dict[str, int] = defaultdict(int)

    for article in articles:
        if not isinstance(article, dict):
            raise ValueError("selection row must be an object")
        counts["articles"] += 1
        article_ref = article.get("article_ref") if isinstance(article.get("article_ref"), str) else None
        article_key = article.get("article_key") if isinstance(article.get("article_key"), str) else None
        source_code = article.get("source_code") if isinstance(article.get("source_code"), str) else None
        selection_strategy = article.get("source_strategy") if isinstance(article.get("source_strategy"), str) else None
        catalog_resolution = article.get("catalog_resolution") if isinstance(article.get("catalog_resolution"), str) else "unknown"
        url = _article_url(article)
        index_row = index_by_ref.get(article_ref or "")
        index_primary = index_row.get("primary_source_role") if isinstance(index_row, Mapping) and isinstance(index_row.get("primary_source_role"), str) else None
        primary_role = index_primary or selection_strategy or "unknown"
        fallback_roles = index_row.get("content_fallback_roles") if isinstance(index_row, Mapping) else []
        metadata_roles = index_row.get("metadata_roles") if isinstance(index_row, Mapping) else []
        fallback_role_list = [role for role in fallback_roles if isinstance(role, str)] if isinstance(fallback_roles, list) else []
        metadata_role_list = [role for role in metadata_roles if isinstance(role, str)] if isinstance(metadata_roles, list) else []
        terminal_results = results_by_article_ref.get(article_ref or "") or results_by_article_key.get(article_key or "") or results_by_url.get(url, [])
        terminal_by_role: dict[str, dict[str, int]] = defaultdict(lambda: {"captured": 0, "blocked": 0, "failed": 0})
        for result in terminal_results:
            role = result.get("source_role") if isinstance(result.get("source_role"), str) else "unknown"
            status = result.get("status")
            if status in TERMINAL_STATES:
                terminal_by_role[role][str(status)] += 1

        primary_counts = terminal_by_role.get(primary_role, {"captured": 0, "blocked": 0, "failed": 0})
        if primary_counts["captured"]:
            primary_state = "captured"
            counts["primary_captured"] += 1
        elif primary_counts["blocked"]:
            primary_state = "blocked"
            counts["primary_blocked"] += 1
        elif primary_counts["failed"]:
            primary_state = "failed"
            counts["primary_failed"] += 1
        else:
            primary_state = "missing"
            counts["primary_missing_terminal"] += 1

        fallback_captured_roles = [role for role in fallback_role_list if terminal_by_role.get(role, {}).get("captured", 0) > 0]
        fallback_needed = primary_state != "captured" and bool(fallback_captured_roles)
        if fallback_needed:
            counts["fallback_needed"] += 1
        if fallback_captured_roles:
            counts["fallback_captured"] += 1
        if catalog_resolution == "resolved":
            counts["resolved"] += 1
        else:
            counts["unresolved"] += 1
        if selection_strategy and index_primary and selection_strategy != index_primary:
            counts["strategy_mismatch"] += 1
            diagnostics.append(
                strategy_diagnostic(
                    "strategy_primary_mismatch",
                    "selection source_strategy differs from catalog index primary_source_role",
                    article=article,
                    source_role=primary_role,
                    url=url,
                    severity="error",
                )
            )
        if primary_state == "missing":
            diagnostics.append(
                strategy_diagnostic(
                    "primary_source_missing_terminal_state",
                    "intended primary source has no acquisition terminal state",
                    article=article,
                    source_role=primary_role,
                    url=url,
                    severity="error" if catalog_resolution == "resolved" else "info",
                )
            )
        if source_code and primary_role not in allowed_roles.get(source_code, {primary_role}):
            diagnostics.append(
                strategy_diagnostic(
                    "primary_source_role_not_catalog_allowed",
                    "primary source role is not listed in catalog allowed_source_roles for source_code",
                    article=article,
                    source_role=primary_role,
                    url=url,
                    severity="warning",
                )
            )
        if fallback_needed:
            diagnostics.append(
                strategy_diagnostic(
                    "content_fallback_used",
                    "primary source was not captured and a content fallback was captured",
                    article=article,
                    source_role=primary_role,
                    url=url,
                    severity="warning",
                )
            )

        capture_policy = "local_only_no_network"
        by_primary_role[primary_role] += 1
        by_capture_policy[capture_policy] += 1
        rows.append(
            {
                "schema_version": STRATEGY_SCHEMA_VERSION,
                "milestone_id": MILESTONE_ID,
                "slice_id": SLICE_ID,
                "selection_id": SELECTION_ID,
                "article_ref": article_ref,
                "article_key": article_key,
                "identity_key": article.get("identity_key"),
                "url": url,
                "source_code": source_code,
                "catalog_resolution": catalog_resolution,
                "selection_source_strategy": selection_strategy,
                "intended_primary_source_role": primary_role,
                "catalog_primary_source_role": index_primary,
                "content_fallback_roles": fallback_role_list,
                "metadata_roles": metadata_role_list,
                "capture_policy": capture_policy,
                "capture_phase_network_allowed": False,
                "replay_phase_network_allowed": False,
                "terminal_state_counts_by_role": {role: dict(value) for role, value in sorted(terminal_by_role.items())},
                "primary_terminal_state": primary_state,
                "fallback_captured_roles": fallback_captured_roles,
                "fallback_needed": fallback_needed,
                "diagnostic_codes": [row["diagnostic_code"] for row in diagnostics if row.get("article_key") == article_key and row.get("url") == url],
                "graph_import_allowed": False,
                "production_import_attempted": False,
                "ladybugdb_written": False,
                "trusted_kg_import_allowed": False,
                "raw_payload_embedded_in_metadata": False,
            }
        )

    summary = {
        "schema_version": STRATEGY_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "passed" if not [d for d in diagnostics if d.get("severity") == "error"] else "failed",
        "article_count": counts["articles"],
        "counts": counts,
        "by_primary_source_role": dict(sorted(by_primary_role.items())),
        "by_capture_policy": dict(sorted(by_capture_policy.items())),
        "strategy_diagnostic_count": len(diagnostics),
        "error_count": len([d for d in diagnostics if d.get("severity") == "error"]),
        "diagnostics": diagnostics,
        "articles": rows,
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
    return diagnostics, summary, rows


def strategy_diagnostic(
    code: str,
    message: str,
    *,
    article: Mapping[str, Any],
    source_role: str,
    url: str,
    severity: str,
) -> dict[str, Any]:
    return {
        "schema_version": STRATEGY_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "severity": severity,
        "diagnostic_code": code,
        "code": code,
        "message": message,
        "article_ref": article.get("article_ref"),
        "article_key": article.get("article_key"),
        "identity_key": article.get("identity_key"),
        "source_role": source_role,
        "url": url,
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "graph_import_allowed": False,
    }


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
    parser.add_argument("--catalog", type=Path, default=Path("data/article_catalog/catalog.json"))
    parser.add_argument("--index", type=Path, default=Path("data/article_catalog/index.json"))
    parser.add_argument("--write-summary", required=True, type=Path)
    parser.add_argument("--write-diagnostics", required=True, type=Path)
    parser.add_argument("--write-report", type=Path)
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

    if args.check_strategies:
        catalog = load_json(args.catalog)
        index = load_json(args.index)
        strategy_errors, strategy_summary, _strategy_rows = normalize_source_strategies(selection, verification, catalog, index)
        strategy_summary["duration_ms"] = int((time.perf_counter() - started) * 1000)
        write_json(args.write_summary, strategy_summary)
        write_jsonl(args.write_diagnostics, strategy_errors)
        artifact_paths = [args.write_summary, args.write_diagnostics]
        if args.write_report is not None:
            args.write_report.parent.mkdir(parents=True, exist_ok=True)
            args.write_report.write_text(render_report(verification), encoding="utf-8")
            artifact_paths.append(args.write_report)
        for artifact_path in artifact_paths:
            assert_metadata_artifact_is_redacted(artifact_path)
        total_errors = len(errors) + len([row for row in strategy_errors if row.get("severity") == "error"])
        print(
            json.dumps(
                {
                    "summary_path": args.write_summary.as_posix(),
                    "diagnostics_path": args.write_diagnostics.as_posix(),
                    "error_count": total_errors,
                    "counts": strategy_summary["counts"],
                },
                sort_keys=True,
            )
        )
        return 0 if total_errors == 0 else 1

    write_json(args.write_summary, verification)
    write_jsonl(args.write_diagnostics, acquisition_diagnostics(verification, errors))
    if args.write_report is not None:
        args.write_report.parent.mkdir(parents=True, exist_ok=True)
        args.write_report.write_text(render_report(verification), encoding="utf-8")
    artifact_paths = [args.write_summary, args.write_diagnostics]
    if args.write_report is not None:
        artifact_paths.append(args.write_report)
    for artifact_path in artifact_paths:
        assert_metadata_artifact_is_redacted(artifact_path)
    print(json.dumps({"summary_path": args.write_summary.as_posix(), "error_count": len(errors), "counts": verification["counts"]}, sort_keys=True))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
