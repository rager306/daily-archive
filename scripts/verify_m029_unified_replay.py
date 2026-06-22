#!/usr/bin/env python3
"""Verify S05 replay closure artifacts for the M029 unified corpus."""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from collections.abc import Mapping, Sequence
from pathlib import Path, PurePosixPath
from typing import Any

MILESTONE_ID = "M029-eb0ljz"
SLICE_ID = "S05"
SELECTION_ID = "m029-unified-corpus-v1"
SUMMARY_SCHEMA_VERSION = "m029-unified-replay.v1"
DIAGNOSTIC_SCHEMA_VERSION = "m029-unified-replay-verifier.v1"
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
    resolved = path.resolve()
    for base in (root.resolve(), Path.cwd().resolve()):
        try:
            return resolved.relative_to(base).as_posix()
        except ValueError:
            continue
    return str(path)


def diagnostic(
    code: str,
    message: str,
    *,
    path: Path | str | None = None,
    json_path: str = "$",
    article_ref: str | None = None,
) -> dict[str, Any]:
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
        by_article[article_key_for(article)] = dict(article)
    return by_article


def index_by_identity(
    rows: Sequence[Mapping[str, Any]],
) -> tuple[dict[str, Mapping[str, Any]], dict[str, Mapping[str, Any]]]:
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


def check_summary_shape(
    summary: Mapping[str, Any], diagnostics_rows: Sequence[Mapping[str, Any]], path: Path
) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    if summary.get("schema_version") != SUMMARY_SCHEMA_VERSION:
        problems.append(
            diagnostic(
                "invalid_schema_version",
                "replay summary schema version is invalid",
                path=path,
                json_path="$.schema_version",
            )
        )
    if summary.get("milestone_id") != MILESTONE_ID or summary.get("slice_id") != SLICE_ID:
        problems.append(
            diagnostic(
                "invalid_milestone_or_slice",
                "replay summary milestone/slice identifiers are invalid",
                path=path,
            )
        )
    results = summary.get("results")
    if not isinstance(results, list):
        problems.append(
            diagnostic(
                "missing_summary_results",
                "replay summary results must be a list",
                path=path,
                json_path="$.results",
            )
        )
        return problems
    if len(results) != len(diagnostics_rows):
        problems.append(
            diagnostic(
                "summary_diagnostics_count_mismatch",
                "summary results and diagnostics row counts differ",
                path=path,
            )
        )
    if summary.get("article_count") != len(results):
        problems.append(
            diagnostic(
                "article_count_mismatch",
                "summary article_count does not match result count",
                path=path,
                json_path="$.article_count",
            )
        )
    zero_chunk_count = sum(
        1 for row in results if isinstance(row, Mapping) and row.get("zero_chunk") is True
    )
    loaded_count = len(results) - zero_chunk_count
    if summary.get("runtime_loaded_count") != loaded_count:
        problems.append(
            diagnostic(
                "runtime_loaded_count_mismatch",
                "runtime_loaded_count does not match replay rows",
                path=path,
            )
        )
    if summary.get("zero_chunk_count") != zero_chunk_count:
        problems.append(
            diagnostic(
                "zero_chunk_count_mismatch",
                "zero_chunk_count does not match replay rows",
                path=path,
            )
        )
    evidence_count = sum(
        int(row.get("runtime_evidence_count", 0)) for row in results if isinstance(row, Mapping)
    )
    if summary.get("runtime_evidence_count") != evidence_count:
        problems.append(
            diagnostic(
                "runtime_evidence_count_mismatch",
                "runtime_evidence_count does not match replay rows",
                path=path,
            )
        )
    return problems


def check_fail_closed(
    summary: Mapping[str, Any], rows: Sequence[Mapping[str, Any]], path: Path
) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    for flag in row_unsafe_flags(summary):
        problems.append(
            diagnostic(
                "unsafe_summary_flag_true", f"unsafe summary flag is true: {flag}", path=path
            )
        )
    for index, row in enumerate(rows):
        for flag in row_unsafe_flags(row):
            problems.append(
                diagnostic(
                    "unsafe_replay_flag_true",
                    f"unsafe replay row flag is true: {flag}",
                    path=path,
                    json_path=f"$.results[{index}]",
                    article_ref=str(row.get("article_ref") or row.get("identity_key")),
                )
            )
    return problems


def check_selection_alignment(
    selection: Mapping[str, Any], rows: Sequence[Mapping[str, Any]], path: Path
) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    selected = selection_by_article(selection)
    row_by_ref, row_by_identity = index_by_identity(rows)
    resolved_rows: dict[str, Mapping[str, Any]] = {}
    for article_ref, article in selected.items():
        identity_key = str(article.get("identity_key") or "")
        row = row_by_ref.get(article_ref) or row_by_identity.get(identity_key)
        if row is not None:
            resolved_rows[article_ref] = row
    if set(resolved_rows) != set(selected):
        missing = sorted(set(selected) - set(resolved_rows))
        problems.append(
            diagnostic(
                "replay_selection_article_mismatch",
                f"replay rows do not cover selected articles; missing={missing}",
                path=path,
            )
        )
    for article_ref, article in selected.items():
        row = resolved_rows.get(article_ref)
        if row is None:
            continue
        for key in ["article_key", "identity_key", "source_strategy"]:
            if article.get(key) != row.get(key):
                problems.append(
                    diagnostic(
                        "article_identity_mismatch",
                        f"{key} mismatch for {article_ref}",
                        path=path,
                        article_ref=article_ref,
                    )
                )
        expected_url = article.get("canonical_url") or article.get("seed_url")
        if expected_url != row.get("canonical_url"):
            problems.append(
                diagnostic(
                    "canonical_url_mismatch",
                    f"canonical_url mismatch for {article_ref}",
                    path=path,
                    article_ref=article_ref,
                )
            )
    return problems


def check_runtime_parity(
    runtime_summary: Mapping[str, Any], rows: Sequence[Mapping[str, Any]], path: Path
) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    runtime_rows = runtime_summary.get("results")
    if not isinstance(runtime_rows, list):
        problems.append(
            diagnostic(
                "missing_runtime_results", "runtime smoke summary results must be a list", path=path
            )
        )
        return problems
    replay_by_ref, replay_by_identity = index_by_identity(rows)
    for runtime_row in [row for row in runtime_rows if isinstance(row, Mapping)]:
        identity_key = str(runtime_row.get("identity_key") or "")
        try:
            runtime_key = article_key_for(runtime_row)
        except ValueError:
            runtime_key = identity_key
        replay_row = replay_by_ref.get(runtime_key) or replay_by_identity.get(identity_key)
        if replay_row is None:
            problems.append(
                diagnostic(
                    "runtime_row_missing_from_replay",
                    f"runtime row missing from replay: {runtime_key}",
                    path=path,
                    article_ref=runtime_key,
                )
            )
            continue
        for key in ["runtime_evidence_count", "runtime_chunk_count", "zero_chunk"]:
            if replay_row.get(key) != runtime_row.get(key):
                problems.append(
                    diagnostic(
                        "runtime_replay_count_mismatch",
                        f"{key} mismatch for {runtime_key}",
                        path=path,
                        article_ref=runtime_key,
                    )
                )
        if replay_row.get("source_strategy") != runtime_row.get("source_strategy"):
            problems.append(
                diagnostic(
                    "runtime_replay_source_strategy_mismatch",
                    f"source_strategy mismatch for {runtime_key}",
                    path=path,
                    article_ref=runtime_key,
                )
            )
    if len(rows) != len(runtime_rows):
        problems.append(
            diagnostic(
                "runtime_replay_row_count_mismatch",
                "runtime and replay row counts differ",
                path=path,
            )
        )
    for key in ["article_count", "runtime_evidence_count", "zero_chunk_count"]:
        runtime_value = runtime_summary.get(key if key == "article_count" else key)
        replay_value = (
            len(rows)
            if key == "article_count"
            else sum(int(row.get("runtime_evidence_count", 0)) for row in rows)
            if key == "runtime_evidence_count"
            else sum(1 for row in rows if row.get("zero_chunk") is True)
        )
        if runtime_value != replay_value:
            problems.append(
                diagnostic(
                    "runtime_summary_replay_summary_mismatch",
                    f"{key} mismatch between runtime and replay",
                    path=path,
                    json_path=f"$.{key}",
                )
            )
    return problems


def check_artifact_paths(
    rows: Sequence[Mapping[str, Any]], corpus_dir: Path, artifact_root: Path, path: Path
) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    for index, row in enumerate(rows):
        for field in ["evidence_path", "replay_record_path"]:
            try:
                candidate = safe_under_root(artifact_root, row.get(field), label=field)
                if not candidate.is_relative_to(corpus_dir.resolve()):
                    raise ValueError(f"{field}_outside_corpus")
            except ValueError as exc:
                problems.append(
                    diagnostic(
                        f"unsafe_{field}",
                        str(exc),
                        path=path,
                        json_path=f"$.results[{index}].{field}",
                        article_ref=str(row.get("article_ref") or row.get("identity_key")),
                    )
                )
                continue
            if not candidate.exists() or not candidate.is_file():
                problems.append(
                    diagnostic(
                        f"missing_{field}",
                        f"{field} does not exist",
                        path=candidate,
                        json_path=f"$.results[{index}].{field}",
                        article_ref=str(row.get("article_ref") or row.get("identity_key")),
                    )
                )
        for optional_field in ["runtime_event_log_path", "converted_text_path"]:
            value = row.get(optional_field)
            if value in (None, ""):
                continue
            try:
                candidate = safe_under_root(artifact_root, value, label=optional_field)
                if not candidate.is_relative_to(corpus_dir.resolve()):
                    raise ValueError(f"{optional_field}_outside_corpus")
            except ValueError as exc:
                problems.append(
                    diagnostic(
                        f"unsafe_{optional_field}",
                        str(exc),
                        path=path,
                        json_path=f"$.results[{index}].{optional_field}",
                        article_ref=str(row.get("article_ref") or row.get("identity_key")),
                    )
                )
                continue
            if not candidate.exists() or not candidate.is_file():
                problems.append(
                    diagnostic(
                        f"missing_{optional_field}",
                        f"{optional_field} does not exist",
                        path=candidate,
                        json_path=f"$.results[{index}].{optional_field}",
                        article_ref=str(row.get("article_ref") or row.get("identity_key")),
                    )
                )
    return problems


def check_report(report_path: Path) -> list[dict[str, Any]]:
    problems: list[dict[str, Any]] = []
    text = report_path.read_text(encoding="utf-8")
    required = [
        "# M029 Unified Replay Closure",
        "## Article Coverage",
        "## Evidence Surfaces",
        "## Safety Flags",
    ]
    for marker in required:
        if marker not in text:
            problems.append(
                diagnostic(
                    "replay_report_missing_section",
                    f"report missing section marker: {marker}",
                    path=report_path,
                )
            )
    lowered = text.lower()
    for snippet in FORBIDDEN_REPORT_SNIPPETS:
        if snippet.lower() in lowered:
            problems.append(
                diagnostic(
                    "replay_report_embeds_raw_payload",
                    f"report contains forbidden raw payload marker: {snippet}",
                    path=report_path,
                )
            )
    return problems


def verify(args: argparse.Namespace) -> list[dict[str, Any]]:
    selection_path = Path(args.selection)
    summary_path = Path(args.replay_summary)
    diagnostics_path = Path(args.replay_diagnostics)
    report_path = Path(args.replay_report)
    runtime_summary_path = (
        Path(args.compare_runtime_smoke)
        if args.compare_runtime_smoke
        else summary_path.with_name("runtime-smoke-summary.json")
    )
    corpus_dir = summary_path.parent
    artifact_root = corpus_dir.parents[2] if len(corpus_dir.parents) >= 3 else ROOT
    selection = load_json(selection_path)
    summary = load_json(summary_path)
    diagnostics_rows = load_jsonl(diagnostics_path)
    runtime_summary = load_json(runtime_summary_path)
    rows = summary.get("results") if isinstance(summary.get("results"), list) else []
    row_mappings = [row for row in rows if isinstance(row, Mapping)]
    problems: list[dict[str, Any]] = []
    problems.extend(check_summary_shape(summary, diagnostics_rows, summary_path))
    problems.extend(check_fail_closed(summary, row_mappings, summary_path))
    problems.extend(check_selection_alignment(selection, row_mappings, summary_path))
    problems.extend(check_runtime_parity(runtime_summary, row_mappings, summary_path))
    problems.extend(check_artifact_paths(row_mappings, corpus_dir, artifact_root, summary_path))
    problems.extend(check_report(report_path))
    if args.require_no_network and summary.get("network_fetch_attempted") is not False:
        problems.append(
            diagnostic(
                "network_fetch_attempted",
                "replay summary indicates network fetch",
                path=summary_path,
            )
        )
    if args.require_no_import_flags:
        for key in [
            "production_import_attempted",
            "ladybugdb_written",
            "graph_import_allowed",
            "trusted_kg_import_allowed",
        ]:
            if summary.get(key) is not False:
                problems.append(
                    diagnostic(
                        "import_flag_not_fail_closed",
                        f"{key} is not false",
                        path=summary_path,
                        json_path=f"$.{key}",
                    )
                )
    return problems


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True)
    parser.add_argument("--replay-summary", required=True)
    parser.add_argument("--replay-diagnostics", required=True)
    parser.add_argument("--replay-report", required=True)
    parser.add_argument("--compare-runtime-smoke")
    parser.add_argument("--require-no-network", action="store_true")
    parser.add_argument("--require-no-import-flags", action="store_true")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    parsed = parser.parse_args(argv[1:] if argv else None)
    try:
        problems = verify(parsed)
    except Exception as exc:
        sys.stderr.write(f"unified replay verification crashed: {type(exc).__name__}: {exc}\n")
        return 1
    if problems:
        for problem in problems:
            sys.stderr.write(json.dumps(problem, sort_keys=True) + "\n")
        return 1
    summary = load_json(Path(parsed.replay_summary))
    counts = Counter(
        str(row.get("status", "unknown"))
        for row in summary.get("results", [])
        if isinstance(row, Mapping)
    )
    sys.stdout.write(
        json.dumps(
            {
                "status": "passed",
                "selection": parsed.selection,
                "replay_summary": parsed.replay_summary,
                "replay_diagnostics": parsed.replay_diagnostics,
                "replay_report": parsed.replay_report,
                "article_count": summary.get("article_count"),
                "counts": dict(sorted(counts.items())),
            },
            sort_keys=True,
        )
        + "\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
