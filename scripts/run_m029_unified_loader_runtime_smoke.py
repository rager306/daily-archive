#!/usr/bin/env python3
"""Runtime loader smoke for the M029 unified converted corpus.

This command replays the unified selection through the local runtime article
loader without fetching network resources or writing graph/import state.  It
uses S03 conversion-quality metadata to choose one parser-ready converted text
artifact per article when available, and records explicit zero-chunk diagnostics
for articles that are metadata-only, blocked, or otherwise not loader-ready.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tempfile
from collections import Counter, defaultdict
from collections.abc import Iterable, Mapping, Sequence
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT / "src") not in sys.path:
    sys.path.insert(0, str(ROOT / "src"))

from research_graph.corpus.ingestion.loader import load_article_source  # noqa: E402

MILESTONE_ID = "M029-eb0ljz"
SLICE_ID = "S04"
SELECTION_ID = "m029-unified-corpus-v1"
SCHEMA_VERSION = "m029-runtime-smoke.v1"
FAIL_CLOSED_SAFETY_FLAGS: dict[str, bool] = {
    "graph_import_allowed": False,
    "production_ladybugdb_write_allowed": False,
    "trusted_kg_import_allowed": False,
    "production_import_attempted": False,
    "ladybugdb_written": False,
    "network_fetch_attempted": False,
    "raw_text_embedded_in_metadata": False,
    "raw_binary_embedded_in_metadata": False,
}
UNSAFE_TRUE_FLAGS = set(FAIL_CLOSED_SAFETY_FLAGS)


def utc_now() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


def atomic_write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    tmp_path = Path(tmp_name)
    try:
        with open(fd, "w", encoding="utf-8", closefd=True) as handle:
            handle.write(text)
            handle.flush()
        tmp_path.replace(path)
    except Exception:
        tmp_path.unlink(missing_ok=True)
        raise


def write_json(path: Path, payload: Mapping[str, Any]) -> None:
    atomic_write_text(path, json.dumps(payload, indent=2, sort_keys=False) + "\n")


def write_jsonl(path: Path, rows: Iterable[Mapping[str, Any]]) -> None:
    atomic_write_text(path, "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows))


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"expected JSON object: {path}")
    return value


def rel(path: Path, root: Path = ROOT) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return str(path)


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
    normalized = safe_relative_path(value, label=label)
    root_resolved = root.resolve()
    resolved = (root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root_resolved):
        raise ValueError(f"{label}_escapes_root")
    return resolved


def slug(value: str) -> str:
    safe = re.sub(r"[^A-Za-z0-9._-]+", "_", value).strip("_")
    return safe or "article"


def summary_output_paths(output_dir: Path) -> tuple[Path, Path, Path]:
    corpus_dir = output_dir.parent
    return (
        corpus_dir / "runtime-smoke-summary.json",
        corpus_dir / "runtime-smoke-diagnostics.jsonl",
        corpus_dir / "runtime-smoke-report.md",
    )



def article_key_for(row: Mapping[str, Any]) -> str:
    value = row.get("article_ref") or row.get("identity_key")
    if not isinstance(value, str) or not value:
        raise ValueError("missing_article_ref_or_identity_key")
    return value

def selection_articles(selection: Mapping[str, Any]) -> list[dict[str, Any]]:
    articles = selection.get("articles")
    if not isinstance(articles, list):
        raise ValueError("selection articles must be a list")
    normalized: list[dict[str, Any]] = []
    seen: set[str] = set()
    for index, article in enumerate(articles):
        if not isinstance(article, dict):
            raise ValueError(f"selection article at index {index} is not an object")
        article_ref = article_key_for(article)
        if article_ref in seen:
            raise ValueError(f"duplicate selection article identity: {article_ref}")
        seen.add(article_ref)
        normalized.append(dict(article))
    return normalized


def conversion_rows_by_article(conversion_summary: Mapping[str, Any]) -> dict[str, list[dict[str, Any]]]:
    rows = conversion_summary.get("results")
    if not isinstance(rows, list):
        raise ValueError("conversion summary results must be a list")
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for index, row in enumerate(rows):
        if not isinstance(row, dict):
            raise ValueError(f"conversion result at index {index} is not an object")
        article_ref = article_key_for(row)
        grouped[article_ref].append(dict(row))
    return dict(grouped)


def source_rows_by_variant(source_summary: Mapping[str, Any]) -> dict[str, dict[str, Any]]:
    rows = source_summary.get("results", [])
    if not isinstance(rows, list):
        raise ValueError("source summary results must be a list")
    by_variant: dict[str, dict[str, Any]] = {}
    for index, row in enumerate(rows):
        if not isinstance(row, dict):
            raise ValueError(f"source result at index {index} is not an object")
        variant_id = row.get("variant_id")
        if isinstance(variant_id, str) and variant_id:
            by_variant[variant_id] = dict(row)
    return by_variant


def unsafe_flags(payload: Mapping[str, Any]) -> list[str]:
    found: list[str] = []
    for key in UNSAFE_TRUE_FLAGS:
        if payload.get(key) is True:
            found.append(key)
    nested = payload.get("fail_closed_safety_flags")
    if isinstance(nested, Mapping):
        for key in UNSAFE_TRUE_FLAGS:
            if nested.get(key) is True:
                found.append(f"fail_closed_safety_flags.{key}")
    return sorted(set(found))


def best_runtime_row(rows: Sequence[Mapping[str, Any]]) -> Mapping[str, Any] | None:
    parser_ready = [row for row in rows if row.get("parser_ready") is True and row.get("converted_text_path")]
    if parser_ready:
        return sorted(parser_ready, key=lambda row: str(row.get("variant_id", "")))[0]
    converted = [row for row in rows if row.get("status") == "converted" and row.get("converted_text_path")]
    if converted:
        return sorted(converted, key=lambda row: str(row.get("variant_id", "")))[0]
    return None


def quality_payload(result: Any) -> dict[str, Any] | None:
    quality = getattr(result, "quality", None)
    if quality is None:
        return None
    return {
        "status": quality.status,
        "char_count": quality.char_count,
        "line_count": quality.line_count,
        "heading_count": quality.heading_count,
        "non_heading_nonempty_line_count": quality.non_heading_nonempty_line_count,
        "warnings": list(quality.warnings),
        "fallback_reason": quality.fallback_reason,
    }


def runtime_loaded_row(
    *,
    article: Mapping[str, Any],
    selected: Mapping[str, Any],
    source_row: Mapping[str, Any] | None,
    converted_path: Path,
    output_dir: Path,
    artifact_root: Path,
) -> dict[str, Any]:
    event_log = output_dir / f"{slug(article_key_for(article))}.loader-events.jsonl"
    result = load_article_source(
        converted_path,
        log_path=event_log,
        source_type="text",
        paper_id=str(article.get("identity_key") or article_key_for(article)),
    )
    line_count = 0
    if result.text:
        line_count = sum(1 for line in result.text.splitlines() if line.strip())
    evidence_count = 1 if result.outcome == "loaded" and line_count > 0 else 0
    diagnostic_code = "runtime_loader_loaded" if evidence_count else "runtime_loader_zero_chunk"
    failure_reason = result.failure_reason if result.failure_reason else (None if evidence_count else "zero_runtime_chunks")
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "article_ref": article.get("article_ref"),
        "article_key": article.get("article_key"),
        "identity_key": article.get("identity_key"),
        "canonical_url": article.get("canonical_url") or article.get("seed_url"),
        "seed_url": article.get("seed_url"),
        "source_code": article.get("source_code"),
        "source_strategy": article.get("source_strategy"),
        "selected_variant_id": selected.get("variant_id"),
        "selected_source_role": selected.get("source_role"),
        "source_summary_status": source_row.get("status") if source_row else None,
        "status": "loaded" if evidence_count else "zero_chunk",
        "diagnostic_code": diagnostic_code,
        "code": diagnostic_code,
        "failure_reason": failure_reason,
        "runtime_loader_outcome": result.outcome,
        "runtime_loader_failure_reason": result.failure_reason,
        "runtime_loader_warnings": list(result.warnings),
        "runtime_loader_warning_count": result.warning_count,
        "runtime_loader_name": result.loader_name,
        "runtime_parser_name": result.parser_name,
        "runtime_source_type": result.source_type,
        "runtime_media_type": result.media_type,
        "runtime_source_sha256": result.sha256,
        "runtime_source_byte_size": result.byte_size,
        "runtime_duration_ms": result.duration_ms,
        "runtime_event_log_path": rel(event_log, artifact_root),
        "converted_text_path": rel(converted_path, artifact_root),
        "converted_text_sha256": selected.get("converted_text_sha256"),
        "converted_text_byte_size": selected.get("converted_text_byte_size"),
        "parser_ready_from_conversion": selected.get("parser_ready") is True,
        "conversion_status": selected.get("status"),
        "conversion_diagnostic_code": selected.get("diagnostic_code") or selected.get("code"),
        "runtime_evidence_count": evidence_count,
        "runtime_chunk_count": line_count if evidence_count else 0,
        "zero_chunk": evidence_count == 0,
        "quality": quality_payload(result),
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "graph_import_allowed": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
    }


def zero_chunk_row(*, article: Mapping[str, Any], rows: Sequence[Mapping[str, Any]], reason: str) -> dict[str, Any]:
    status_counts = Counter(str(row.get("status", "unknown")) for row in rows)
    role_counts = Counter(str(row.get("source_role", "unknown")) for row in rows)
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "article_ref": article.get("article_ref"),
        "article_key": article.get("article_key"),
        "identity_key": article.get("identity_key"),
        "canonical_url": article.get("canonical_url") or article.get("seed_url"),
        "seed_url": article.get("seed_url"),
        "source_code": article.get("source_code"),
        "source_strategy": article.get("source_strategy"),
        "selected_variant_id": None,
        "selected_source_role": None,
        "status": "zero_chunk",
        "diagnostic_code": "runtime_loader_zero_chunk",
        "code": "runtime_loader_zero_chunk",
        "failure_reason": reason,
        "runtime_loader_outcome": "not_attempted",
        "runtime_loader_failure_reason": reason,
        "runtime_loader_warnings": [reason],
        "runtime_loader_warning_count": 1,
        "runtime_loader_name": None,
        "runtime_parser_name": None,
        "runtime_source_type": None,
        "runtime_media_type": None,
        "runtime_source_sha256": None,
        "runtime_source_byte_size": 0,
        "runtime_duration_ms": 0,
        "runtime_event_log_path": None,
        "converted_text_path": None,
        "converted_text_sha256": None,
        "converted_text_byte_size": 0,
        "parser_ready_from_conversion": False,
        "conversion_status_counts": dict(sorted(status_counts.items())),
        "conversion_source_role_counts": dict(sorted(role_counts.items())),
        "runtime_evidence_count": 0,
        "runtime_chunk_count": 0,
        "zero_chunk": True,
        "quality": None,
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "graph_import_allowed": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
    }


def build_runtime_rows(
    *,
    selection: Mapping[str, Any],
    conversion_summary: Mapping[str, Any],
    source_summary: Mapping[str, Any],
    corpus_dir: Path,
    output_dir: Path,
) -> list[dict[str, Any]]:
    artifact_root = corpus_dir.parents[2] if len(corpus_dir.parents) >= 3 else ROOT
    articles = selection_articles(selection)
    grouped = conversion_rows_by_article(conversion_summary)
    source_by_variant = source_rows_by_variant(source_summary)
    rows: list[dict[str, Any]] = []
    for article in articles:
        article_ref = article_key_for(article)
        conversion_rows = grouped.get(article_ref, [])
        if not conversion_rows:
            rows.append(zero_chunk_row(article=article, rows=[], reason="missing_conversion_rows"))
            continue
        unsafe = [flag for row in conversion_rows for flag in unsafe_flags(row)]
        if unsafe:
            rows.append(zero_chunk_row(article=article, rows=conversion_rows, reason=f"unsafe_conversion_flags:{','.join(sorted(set(unsafe)))}"))
            continue
        selected = best_runtime_row(conversion_rows)
        if selected is None:
            rows.append(zero_chunk_row(article=article, rows=conversion_rows, reason="no_parser_ready_converted_text"))
            continue
        try:
            converted_path = safe_under_root(artifact_root, selected.get("converted_text_path"), label="converted_text_path")
            if not converted_path.is_relative_to(corpus_dir.resolve()):
                raise ValueError("converted_text_path_outside_corpus")
        except ValueError as exc:
            rows.append(zero_chunk_row(article=article, rows=conversion_rows, reason=str(exc)))
            continue
        if not converted_path.exists() or not converted_path.is_file():
            rows.append(zero_chunk_row(article=article, rows=conversion_rows, reason="converted_text_missing"))
            continue
        rows.append(
            runtime_loaded_row(
                article=article,
                selected=selected,
                source_row=source_by_variant.get(str(selected.get("variant_id"))),
                converted_path=converted_path,
                output_dir=output_dir,
                artifact_root=artifact_root,
            )
        )
    return rows


def render_report(summary: Mapping[str, Any], rows: Sequence[Mapping[str, Any]]) -> str:
    lines = [
        "# M029 Unified Loader Runtime Smoke",
        "",
        f"- Schema: `{summary['schema_version']}`",
        f"- Selection: `{summary['selection_id']}`",
        f"- Article count: {summary['article_count']}",
        f"- Runtime loaded count: {summary['runtime_loaded_count']}",
        f"- Zero-chunk count: {summary['zero_chunk_count']}",
        f"- Runtime evidence count: {summary['runtime_evidence_count']}",
        f"- Network fetch attempted: `{str(summary['network_fetch_attempted']).lower()}`",
        f"- Production import attempted: `{str(summary['production_import_attempted']).lower()}`",
        f"- LadybugDB written: `{str(summary['ladybugdb_written']).lower()}`",
        "",
        "## Article Outcomes",
        "",
        "| Article | Identity | Source strategy | Outcome | Evidence | Chunks | Diagnostic |",
        "|---|---|---|---:|---:|---:|---|",
    ]
    for row in rows:
        lines.append(
            "| "
            + " | ".join(
                [
                    str(row.get("article_ref") or row.get("identity_key")),
                    str(row.get("identity_key")),
                    str(row.get("source_strategy")),
                    str(row.get("status")),
                    str(row.get("runtime_evidence_count", 0)),
                    str(row.get("runtime_chunk_count", 0)),
                    str(row.get("diagnostic_code")),
                ]
            )
            + " |"
        )
    lines.extend(
        [
            "",
            "## Fail-Closed Boundaries",
            "",
            "Runtime smoke is metadata-only: it does not fetch network sources, does not import graph state, "
            "does not write LadybugDB, and does not embed raw converted text in summary/diagnostic/report metadata.",
            "",
        ]
    )
    return "\n".join(lines)


def build_summary(
    *,
    selection: Mapping[str, Any],
    conversion_summary: Mapping[str, Any],
    source_summary: Mapping[str, Any],
    output_dir: Path,
    rows: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    status_counts = Counter(str(row.get("status", "unknown")) for row in rows)
    source_strategy_counts = Counter(str(row.get("source_strategy", "unknown")) for row in rows)
    artifact_root = output_dir.parent.parents[2] if len(output_dir.parent.parents) >= 3 else ROOT
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": selection.get("selection_id", SELECTION_ID),
        "status": "passed" if rows else "failed",
        "created_at": utc_now(),
        "article_count": len(rows),
        "selection_article_count": len(selection.get("articles", [])) if isinstance(selection.get("articles"), list) else None,
        "conversion_article_count": conversion_summary.get("article_count"),
        "source_article_count": source_summary.get("article_count"),
        "runtime_loaded_count": status_counts.get("loaded", 0),
        "zero_chunk_count": status_counts.get("zero_chunk", 0),
        "runtime_evidence_count": sum(int(row.get("runtime_evidence_count", 0)) for row in rows),
        "runtime_chunk_count": sum(int(row.get("runtime_chunk_count", 0)) for row in rows),
        "counts": dict(sorted(status_counts.items())),
        "source_strategy_counts": dict(sorted(source_strategy_counts.items())),
        "runtime_event_dir": rel(output_dir, artifact_root),
        "runtime_summary_path": rel(output_dir.parent / "runtime-smoke-summary.json", artifact_root),
        "runtime_diagnostics_path": rel(output_dir.parent / "runtime-smoke-diagnostics.jsonl", artifact_root),
        "runtime_report_path": rel(output_dir.parent / "runtime-smoke-report.md", artifact_root),
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "graph_import_allowed": False,
        "raw_text_embedded_in_metadata": False,
        "raw_binary_embedded_in_metadata": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
        "results": list(rows),
    }


def run(args: argparse.Namespace) -> int:
    selection_path = Path(args.selection)
    conversion_summary_path = Path(args.conversion_summary)
    source_summary_path = Path(args.source_summary)
    output_dir = Path(args.output_dir)
    corpus_dir = output_dir.parent
    selection = load_json(selection_path)
    conversion_summary = load_json(conversion_summary_path)
    source_summary = load_json(source_summary_path)
    output_dir.mkdir(parents=True, exist_ok=True)
    rows = build_runtime_rows(
        selection=selection,
        conversion_summary=conversion_summary,
        source_summary=source_summary,
        corpus_dir=corpus_dir,
        output_dir=output_dir,
    )
    summary = build_summary(
        selection=selection,
        conversion_summary=conversion_summary,
        source_summary=source_summary,
        output_dir=output_dir,
        rows=rows,
    )
    summary_path, diagnostics_path, report_path = summary_output_paths(output_dir)
    write_json(summary_path, summary)
    write_jsonl(diagnostics_path, rows)
    atomic_write_text(report_path, render_report(summary, rows))
    sys.stdout.write(
        json.dumps(
            {
                "status": summary["status"],
                "article_count": summary["article_count"],
                "runtime_loaded_count": summary["runtime_loaded_count"],
                "zero_chunk_count": summary["zero_chunk_count"],
                "runtime_evidence_count": summary["runtime_evidence_count"],
                "summary_path": rel(summary_path),
                "diagnostics_path": rel(diagnostics_path),
                "report_path": rel(report_path),
            },
            sort_keys=True,
        )
        + "\n"
    )
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True)
    parser.add_argument("--conversion-summary", required=True)
    parser.add_argument("--source-summary", required=True)
    parser.add_argument("--output-dir", required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv[1:] if argv else None)
    try:
        return run(args)
    except Exception as exc:
        sys.stderr.write(f"runtime smoke failed: {type(exc).__name__}: {exc}\n")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
