#!/usr/bin/env python3
"""Verify S06 readiness artifacts for the M029 unified corpus."""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from collections.abc import Mapping, Sequence
from pathlib import Path, PurePosixPath
from typing import Any

MILESTONE_ID = "M029-eb0ljz"
SLICE_ID = "S06"
SELECTION_ID = "m029-unified-corpus-v1"
SUMMARY_SCHEMA_VERSION = "m029-unified-readiness-summary.v1"
DECISION_SCHEMA_VERSION = "m029-unified-readiness-decision.v1"
DIAGNOSTIC_SCHEMA_VERSION = "m029-unified-readiness-verifier.v1"
ROOT = Path(__file__).resolve().parents[1]

UNSAFE_TRUE_FLAGS = {
    "network_fetch_attempted",
    "production_import_attempted",
    "ladybugdb_written",
    "trusted_kg_import_allowed",
    "graph_import_allowed",
    "production_ladybugdb_write_allowed",
    "raw_text_embedded_in_metadata",
    "raw_binary_embedded_in_metadata",
    "raw_text_embedded",
    "raw_binary_embedded",
    "raw_payload_embedded_in_metadata",
    "ready_for_graph_import",
    "ready_for_production_import",
    "ready_for_trusted_kg",
}
FORBIDDEN_REPORT_SNIPPETS = {
    "<html",
    "</html",
    "<!doctype html",
    "%pdf-",
    "base64,",
    "raw_article_text",
    "raw_pdf_bytes",
    "chunk_text",
    "model_output",
}
REQUIRED_REPORT_PHRASES = {
    "Dedupe and Provenance",
    "Final Counts and Block Reasons",
    "Article Readiness",
    "Boundary Decision",
    "Safety Flags",
}


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"expected JSON object: {path}")
    return value


def rel(path: Path, root: Path = ROOT) -> str:
    resolved = path.resolve()
    for base in (root.resolve(), Path.cwd().resolve()):
        try:
            return resolved.relative_to(base).as_posix()
        except ValueError:
            continue
    return str(path)


def diagnostic(code: str, message: str, *, path: Path | str | None = None, json_path: str = "$", article_ref: str | None = None) -> dict[str, Any]:
    return {
        "schema_version": DIAGNOSTIC_SCHEMA_VERSION,
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


def safe_relative_path(value: Any, *, label: str) -> PurePosixPath:
    if value is None:
        raise ValueError(f"missing_{label}")
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"missing_{label}")
    if "://" in value:
        raise ValueError(f"url_not_allowed_as_{label}")
    normalized = PurePosixPath(value.replace("\\", "/"))
    if normalized.is_absolute() or ".." in normalized.parts or any(part == "" for part in normalized.parts):
        raise ValueError(f"unsafe_{label}")
    return normalized


def safe_under_root(root: Path, value: Any, *, label: str) -> Path:
    normalized = safe_relative_path(value, label=label)
    root_resolved = root.resolve()
    candidate = (root_resolved / normalized.as_posix()).resolve()
    if not candidate.is_relative_to(root_resolved):
        raise ValueError(f"{label}_escapes_root")
    return candidate


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
        if article_ref in by_article:
            raise ValueError(f"duplicate selection article identity: {article_ref}")
        by_article[article_ref] = dict(article)
    return by_article


def index_by_identity(rows: Sequence[Mapping[str, Any]]) -> tuple[dict[str, Mapping[str, Any]], dict[str, Mapping[str, Any]]]:
    by_article_ref: dict[str, Mapping[str, Any]] = {}
    by_identity_key: dict[str, Mapping[str, Any]] = {}
    for row in rows:
        article_ref = row.get("article_ref")
        identity_key = row.get("identity_key")
        if isinstance(article_ref, str) and article_ref:
            by_article_ref[article_ref] = row
        if isinstance(identity_key, str) and identity_key:
            by_identity_key[identity_key] = row
    return by_article_ref, by_identity_key


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


def check_summary_shape(summary: Mapping[str, Any], path: Path) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    if summary.get("schema_version") != SUMMARY_SCHEMA_VERSION:
        problems.append(diagnostic("invalid_summary_schema_version", "readiness summary schema version is invalid", path=path, json_path="$.schema_version"))
    if summary.get("milestone_id") != MILESTONE_ID or summary.get("slice_id") != SLICE_ID:
        problems.append(diagnostic("invalid_milestone_or_slice", "readiness summary milestone/slice identifiers are invalid", path=path))
    results = summary.get("results")
    if not isinstance(results, list):
        problems.append(diagnostic("missing_summary_results", "readiness summary results must be a list", path=path, json_path="$.results"))
        return problems
    if summary.get("article_count") != len(results):
        problems.append(diagnostic("article_count_mismatch", "summary article_count does not match results length", path=path, json_path="$.article_count"))
    ready_count = sum(1 for row in results if isinstance(row, Mapping) and row.get("readiness_category") == "ready")
    partial_count = sum(1 for row in results if isinstance(row, Mapping) and row.get("readiness_category") == "partial")
    blocked_count = sum(1 for row in results if isinstance(row, Mapping) and row.get("readiness_category") == "blocked")
    zero_chunk_count = sum(1 for row in results if isinstance(row, Mapping) and row.get("zero_chunk") is True)
    if summary.get("ready_count") != ready_count:
        problems.append(diagnostic("ready_count_mismatch", "ready_count does not match rows", path=path))
    if summary.get("partial_count") != partial_count:
        problems.append(diagnostic("partial_count_mismatch", "partial_count does not match rows", path=path))
    if summary.get("blocked_count") != blocked_count:
        problems.append(diagnostic("blocked_count_mismatch", "blocked_count does not match rows", path=path))
    if summary.get("zero_chunk_count") != zero_chunk_count:
        problems.append(diagnostic("zero_chunk_count_mismatch", "zero_chunk_count does not match rows", path=path))
    evidence_count = sum(int(row.get("runtime_evidence_count", 0)) for row in results if isinstance(row, Mapping))
    if summary.get("runtime_evidence_count") != evidence_count:
        problems.append(diagnostic("runtime_evidence_count_mismatch", "runtime_evidence_count does not match rows", path=path))
    if "provenance" not in str(summary.get("dedupe_rule", "")).lower():
        problems.append(diagnostic("dedupe_rule_missing_provenance", "dedupe rule must preserve provenance", path=path, json_path="$.dedupe_rule"))
    return problems


def check_decision(summary: Mapping[str, Any], decision: Mapping[str, Any], path: Path) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    if decision.get("schema_version") != DECISION_SCHEMA_VERSION:
        problems.append(diagnostic("invalid_decision_schema_version", "readiness decision schema version is invalid", path=path, json_path="$.schema_version"))
    for key in ["milestone_id", "slice_id", "selection_id", "article_count", "ready_count", "partial_count", "blocked_count", "dedupe_rule"]:
        if decision.get(key) != summary.get(key):
            problems.append(diagnostic("decision_summary_mismatch", f"decision {key} does not match summary", path=path, json_path=f"$.{key}"))
    expected_decision = "partial_preprocessing_ready" if int(summary.get("partial_count", 0) or 0) else "local_replay_ready"
    if decision.get("decision") != expected_decision:
        problems.append(diagnostic("invalid_readiness_decision", "decision does not match final counts", path=path, json_path="$.decision"))
    disallowed = decision.get("disallowed_next_steps")
    if not isinstance(disallowed, list) or not any("graph" in str(item).lower() for item in disallowed):
        problems.append(diagnostic("missing_graph_import_block", "decision must explicitly block graph import readiness", path=path, json_path="$.disallowed_next_steps"))
    return problems


def check_selection_alignment(selection: Mapping[str, Any], rows: Sequence[Mapping[str, Any]], path: Path) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    selected = selection_by_article(selection)
    row_by_ref, row_by_identity = index_by_identity(rows)
    resolved: dict[str, Mapping[str, Any]] = {}
    for article_ref, article in selected.items():
        identity_key = str(article.get("identity_key") or "")
        row = row_by_ref.get(article_ref) or row_by_identity.get(identity_key)
        if row is not None:
            resolved[article_ref] = row
    if set(resolved) != set(selected):
        missing = sorted(set(selected) - set(resolved))
        problems.append(diagnostic("readiness_selection_article_mismatch", f"readiness rows do not cover selected articles; missing={missing}", path=path))
    if len(rows) != len(selected):
        problems.append(diagnostic("readiness_selection_count_mismatch", "readiness row count does not match deduped selection", path=path))
    for article_ref, article in selected.items():
        row = resolved.get(article_ref)
        if row is None:
            continue
        for key in ["article_key", "identity_key", "source_strategy"]:
            if article.get(key) != row.get(key):
                problems.append(diagnostic("article_identity_mismatch", f"{key} mismatch for {article_ref}", path=path, article_ref=article_ref))
        if article.get("canonical_url") or article.get("seed_url"):
            expected_url = article.get("canonical_url") or article.get("seed_url")
            if expected_url != row.get("canonical_url"):
                problems.append(diagnostic("canonical_url_mismatch", f"canonical_url mismatch for {article_ref}", path=path, article_ref=article_ref))
    return problems


def check_dedupe_rule(selection: Mapping[str, Any], summary: Mapping[str, Any], rows: Sequence[Mapping[str, Any]], path: Path) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    selected = selection_by_article(selection)
    identity_counts = Counter(article_key_for(row) for row in rows)
    duplicate_identities = sorted(identity for identity, count in identity_counts.items() if count > 1)
    if duplicate_identities:
        problems.append(diagnostic("duplicate_readiness_identity", f"readiness rows contain duplicate article identities: {duplicate_identities}", path=path))
    if summary.get("article_count") != len(selected):
        problems.append(diagnostic("dedupe_article_count_mismatch", "summary article_count must equal one row per selected article_ref/identity_key", path=path, json_path="$.article_count"))
    if summary.get("unique_identity_count") != len(selected):
        problems.append(diagnostic("unique_identity_count_mismatch", "unique_identity_count must equal the deduped selection count", path=path, json_path="$.unique_identity_count"))
    dedupe_rule = str(summary.get("dedupe_rule") or "")
    if "article_ref/identity_key" not in dedupe_rule or "provenance_sources" not in dedupe_rule:
        problems.append(diagnostic("dedupe_rule_missing_required_terms", "dedupe rule must name article_ref/identity_key and provenance_sources", path=path, json_path="$.dedupe_rule"))
    return problems


def check_provenance(selection: Mapping[str, Any], summary: Mapping[str, Any], rows: Sequence[Mapping[str, Any]], path: Path) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    expected_counts: Counter[str] = Counter()
    row_counts: Counter[str] = Counter()
    for article in selection_by_article(selection).values():
        sources = article.get("provenance_sources")
        if not isinstance(sources, list) or not sources:
            problems.append(diagnostic("selection_missing_provenance_sources", "selection article must preserve provenance_sources", path=path, article_ref=str(article.get("article_ref") or article.get("identity_key"))))
            continue
        expected_counts.update(str(source) for source in sources)
    for row in rows:
        sources = row.get("provenance_sources")
        if not isinstance(sources, list) or not sources:
            problems.append(diagnostic("readiness_missing_provenance_sources", "readiness row must preserve provenance_sources", path=path, article_ref=str(row.get("article_ref") or row.get("identity_key"))))
            continue
        row_counts.update(str(source) for source in sources)
    summary_counts = summary.get("provenance_source_counts")
    if dict(sorted(expected_counts.items())) != summary_counts:
        problems.append(diagnostic("selection_provenance_count_mismatch", "summary provenance_source_counts do not match selected articles", path=path, json_path="$.provenance_source_counts"))
    if dict(sorted(row_counts.items())) != summary_counts:
        problems.append(diagnostic("readiness_provenance_count_mismatch", "summary provenance_source_counts do not match readiness rows", path=path, json_path="$.provenance_source_counts"))
    return problems


def write_verify_summary(path: Path, payload: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def check_runtime_and_replay_parity(summary: Mapping[str, Any], rows: Sequence[Mapping[str, Any]], path: Path) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    status_counts = Counter(str(row.get("readiness_status", "unknown")) for row in rows)
    if dict(sorted(status_counts.items())) != summary.get("counts"):
        problems.append(diagnostic("readiness_counts_mismatch", "summary counts do not match readiness statuses", path=path, json_path="$.counts"))
    for index, row in enumerate(rows):
        article_ref = str(row.get("article_ref") or row.get("identity_key"))
        zero_chunk = row.get("zero_chunk") is True or int(row.get("runtime_chunk_count", 0) or 0) == 0
        if zero_chunk and row.get("readiness_category") != "partial":
            problems.append(diagnostic("zero_chunk_not_partial", "zero-chunk article must remain partial", path=path, json_path=f"$.results[{index}]", article_ref=article_ref))
        if not zero_chunk and row.get("readiness_category") != "ready":
            problems.append(diagnostic("loaded_article_not_ready", "non-zero-chunk article should be ready for local replay review", path=path, json_path=f"$.results[{index}]", article_ref=article_ref))
        if zero_chunk and not row.get("block_reason"):
            problems.append(diagnostic("missing_block_reason", "partial row must include a block reason", path=path, json_path=f"$.results[{index}].block_reason", article_ref=article_ref))
    return problems


def check_fail_closed(summary: Mapping[str, Any], decision: Mapping[str, Any], rows: Sequence[Mapping[str, Any]], path: Path) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    for label, row in [("summary", summary), ("decision", decision)]:
        for flag in row_unsafe_flags(row):
            problems.append(diagnostic(f"unsafe_{label}_flag_true", f"unsafe {label} flag is true: {flag}", path=path))
    for index, row in enumerate(rows):
        for flag in row_unsafe_flags(row):
            problems.append(diagnostic("unsafe_readiness_row_flag_true", f"unsafe readiness row flag is true: {flag}", path=path, json_path=f"$.results[{index}]", article_ref=str(row.get("article_ref") or row.get("identity_key"))))
    return problems


def check_report(report: str, summary: Mapping[str, Any], path: Path) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    lowered = report.lower()
    for snippet in FORBIDDEN_REPORT_SNIPPETS:
        if snippet in lowered:
            problems.append(diagnostic("forbidden_report_payload_snippet", f"report contains forbidden payload snippet: {snippet}", path=path))
    for phrase in REQUIRED_REPORT_PHRASES:
        if phrase not in report:
            problems.append(diagnostic("missing_report_section", f"report missing required section phrase: {phrase}", path=path))
    for value in [summary.get("ready_count"), summary.get("partial_count"), summary.get("article_count")]:
        if str(value) not in report:
            problems.append(diagnostic("report_missing_final_count", f"report does not expose count {value}", path=path))
    if "Graph import" not in report and "graph import" not in report:
        problems.append(diagnostic("report_missing_graph_boundary", "report must explicitly mention graph import boundary", path=path))
    return problems


def check_artifact_paths(rows: Sequence[Mapping[str, Any]], artifact_root: Path, path: Path) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    for index, row in enumerate(rows):
        for field in ["evidence_path", "replay_record_path"]:
            if not row.get(field):
                continue
            try:
                candidate = safe_under_root(artifact_root, row.get(field), label=field)
            except ValueError as exc:
                problems.append(diagnostic("unsafe_artifact_path", f"unsafe {field}: {exc}", path=path, json_path=f"$.results[{index}].{field}", article_ref=str(row.get("article_ref") or row.get("identity_key"))))
                continue
            if not candidate.exists():
                problems.append(diagnostic("missing_referenced_artifact", f"referenced {field} does not exist: {rel(candidate)}", path=path, json_path=f"$.results[{index}].{field}", article_ref=str(row.get("article_ref") or row.get("identity_key"))))
    return problems


def run(args: argparse.Namespace) -> int:
    selection_path = Path(args.selection)
    summary_path = Path(args.readiness_summary)
    decision_path = Path(args.readiness_decision)
    report_path = Path(args.readiness_report)
    artifact_root = Path.cwd()
    selection = load_json(selection_path)
    summary = load_json(summary_path)
    decision = load_json(decision_path)
    report = report_path.read_text(encoding="utf-8")
    rows_value = summary.get("results")
    rows = [row for row in rows_value if isinstance(row, Mapping)] if isinstance(rows_value, list) else []
    problems: list[dict[str, Any]] = []
    problems.extend(check_summary_shape(summary, summary_path))
    problems.extend(check_decision(summary, decision, decision_path))
    problems.extend(check_selection_alignment(selection, rows, summary_path))
    if args.check_dedupe_rule:
        problems.extend(check_dedupe_rule(selection, summary, rows, summary_path))
    if args.check_provenance:
        problems.extend(check_provenance(selection, summary, rows, summary_path))
    problems.extend(check_runtime_and_replay_parity(summary, rows, summary_path))
    if args.require_no_network or args.require_no_import_flags:
        problems.extend(check_fail_closed(summary, decision, rows, summary_path))
    problems.extend(check_report(report, summary, report_path))
    problems.extend(check_artifact_paths(rows, artifact_root, summary_path))
    verify_summary = {
        "schema_version": DIAGNOSTIC_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "failed" if problems else "passed",
        "article_count": summary.get("article_count"),
        "decision": decision.get("decision"),
        "ready_count": summary.get("ready_count"),
        "partial_count": summary.get("partial_count"),
        "blocked_count": summary.get("blocked_count"),
        "zero_chunk_count": summary.get("zero_chunk_count"),
        "block_reason_counts": summary.get("block_reason_counts"),
        "provenance_source_counts": summary.get("provenance_source_counts"),
        "dedupe_rule": summary.get("dedupe_rule"),
        "checks": {
            "summary_shape": True,
            "decision_alignment": True,
            "selection_alignment": True,
            "dedupe_rule": bool(args.check_dedupe_rule),
            "provenance": bool(args.check_provenance),
            "runtime_and_replay_parity": True,
            "fail_closed": bool(args.require_no_network or args.require_no_import_flags),
            "report": True,
            "artifact_paths": True,
        },
        "unsafe_flag_count": sum(1 for row in rows for _flag in row_unsafe_flags(row)) + len(row_unsafe_flags(summary)) + len(row_unsafe_flags(decision)),
        "diagnostics": problems,
        "summary_path": rel(summary_path),
        "decision_path": rel(decision_path),
        "report_path": rel(report_path),
    }
    if args.write_verify_summary:
        write_verify_summary(Path(args.write_verify_summary), verify_summary)
    if problems:
        sys.stderr.write("unified readiness verification failed:\n")
        for problem in problems:
            sys.stderr.write(json.dumps(problem, sort_keys=True) + "\n")
        return 1
    sys.stdout.write(json.dumps(verify_summary, sort_keys=True) + "\n")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True)
    parser.add_argument("--readiness-summary", required=True)
    parser.add_argument("--readiness-decision", required=True)
    parser.add_argument("--readiness-report", required=True)
    parser.add_argument("--require-no-network", action="store_true")
    parser.add_argument("--require-no-import-flags", action="store_true")
    parser.add_argument("--check-dedupe-rule", action="store_true")
    parser.add_argument("--check-provenance", action="store_true")
    parser.add_argument("--write-verify-summary")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    parsed = parser.parse_args(argv[1:] if argv else None)
    try:
        return run(parsed)
    except Exception as exc:
        sys.stderr.write(f"unified readiness verification failed: {type(exc).__name__}: {exc}\n")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
