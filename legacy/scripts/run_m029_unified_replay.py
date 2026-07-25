#!/usr/bin/env python3
"""Generate S05 replay closure artifacts for the M029 unified corpus.

The replay closure is intentionally local-only. It does not re-fetch sources or
reload article text; instead it verifies that every selected article can be
explained from the S04 runtime-smoke summary plus the local evidence bundle and
writes compact metadata-only replay artifacts for downstream readiness synthesis.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tempfile
from collections import Counter
from collections.abc import Iterable, Mapping, Sequence
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

MILESTONE_ID = "M029-eb0ljz"
SLICE_ID = "S05"
SELECTION_ID = "m029-unified-corpus-v1"
SCHEMA_VERSION = "m029-unified-replay.v1"
DIAGNOSTIC_SCHEMA_VERSION = "m029-unified-replay-diagnostic.v1"
ROOT = Path(__file__).resolve().parents[1]

FAIL_CLOSED_SAFETY_FLAGS: dict[str, bool] = {
    "network_fetch_attempted": False,
    "production_import_attempted": False,
    "ladybugdb_written": False,
    "trusted_kg_import_allowed": False,
    "graph_import_allowed": False,
    "raw_text_embedded_in_metadata": False,
    "raw_binary_embedded_in_metadata": False,
}
UNSAFE_TRUE_FLAGS = set(FAIL_CLOSED_SAFETY_FLAGS) | {
    "production_ladybugdb_write_allowed",
    "raw_text_embedded",
    "raw_binary_embedded",
    "raw_payload_embedded_in_metadata",
}


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
    atomic_write_text(path, json.dumps(payload, indent=2, sort_keys=True) + "\n")


def write_jsonl(path: Path, rows: Iterable[Mapping[str, Any]]) -> None:
    atomic_write_text(path, "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows))


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


def slug(value: str) -> str:
    safe = re.sub(r"[^A-Za-z0-9._-]+", "_", value).strip("_")
    return safe or "article"


def article_key_for(row: Mapping[str, Any]) -> str:
    value = row.get("article_ref") or row.get("identity_key")
    if not isinstance(value, str) or not value:
        raise ValueError("missing_article_ref_or_identity_key")
    return value


def selected_articles(selection: Mapping[str, Any]) -> list[dict[str, Any]]:
    articles = selection.get("articles")
    if not isinstance(articles, list):
        raise ValueError("selection articles must be a list")
    normalized: list[dict[str, Any]] = []
    seen: set[str] = set()
    for index, article in enumerate(articles):
        if not isinstance(article, dict):
            raise ValueError(f"selection article at index {index} is not an object")
        article_ref = article_key_for(article)  # ty:ignore[invalid-argument-type]
        if article_ref in seen:
            raise ValueError(f"duplicate selection article identity: {article_ref}")
        seen.add(article_ref)
        normalized.append(dict(article))  # ty:ignore[no-matching-overload]
    return normalized


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


def load_evidence_records(evidence_dir: Path) -> list[dict[str, Any]]:
    if not evidence_dir.exists() or not evidence_dir.is_dir():
        raise ValueError(f"missing evidence directory: {evidence_dir}")
    records: list[dict[str, Any]] = []
    for path in sorted(evidence_dir.glob("*.evidence.json")):
        record = load_json(path)
        record.setdefault("evidence_path", rel(path))
        records.append(record)
    if not records:
        raise ValueError(f"no evidence records found in {evidence_dir}")
    return records


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


def replay_output_paths(output_dir: Path) -> tuple[Path, Path, Path]:
    corpus_dir = output_dir.parent
    return (
        corpus_dir / "replay-summary.json",
        corpus_dir / "replay-diagnostics.jsonl",
        corpus_dir / "replay-report.md",
    )


def replay_record_path(output_dir: Path, article: Mapping[str, Any]) -> Path:
    return output_dir / f"{slug(article_key_for(article))}.replay.json"


def build_replay_rows(
    *,
    selection: Mapping[str, Any],
    runtime_summary: Mapping[str, Any],
    evidence_records: Sequence[Mapping[str, Any]],
    output_dir: Path,
) -> list[dict[str, Any]]:
    articles = selected_articles(selection)
    runtime_rows = runtime_summary.get("results")
    if not isinstance(runtime_rows, list):
        raise ValueError("runtime summary results must be a list")
    runtime_by_ref, runtime_by_identity = index_by_identity(
        [row for row in runtime_rows if isinstance(row, Mapping)]
    )
    evidence_by_ref, evidence_by_identity = index_by_identity(evidence_records)
    rows: list[dict[str, Any]] = []
    for article in articles:
        article_ref = article_key_for(article)
        identity_key = str(article.get("identity_key") or "")
        runtime_row = runtime_by_ref.get(article_ref) or runtime_by_identity.get(identity_key)
        evidence = evidence_by_ref.get(article_ref) or evidence_by_identity.get(identity_key)
        if runtime_row is None:
            raise ValueError(f"missing runtime row for selected article: {article_ref}")
        if evidence is None:
            raise ValueError(f"missing evidence record for selected article: {article_ref}")
        unsafe = sorted(set(row_unsafe_flags(runtime_row) + row_unsafe_flags(evidence)))
        if unsafe:
            raise ValueError(f"unsafe flags in replay inputs for {article_ref}: {','.join(unsafe)}")
        runtime_evidence_count = int(
            evidence.get("runtime_evidence_count", runtime_row.get("runtime_evidence_count", 0))
            or 0
        )
        runtime_chunk_count = int(
            evidence.get("runtime_chunk_count", runtime_row.get("runtime_chunk_count", 0)) or 0
        )
        zero_chunk = bool(
            evidence.get("zero_chunk") is True or runtime_row.get("zero_chunk") is True
        )
        status = "replay_zero_chunk_verified" if zero_chunk else "replay_loaded_verified"
        diagnostic_code = "replay_zero_chunk_verified" if zero_chunk else "replay_loaded_verified"
        replay_path = replay_record_path(output_dir, article)
        row = {
            "schema_version": DIAGNOSTIC_SCHEMA_VERSION,
            "milestone_id": MILESTONE_ID,
            "slice_id": SLICE_ID,
            "selection_id": selection.get("selection_id", SELECTION_ID),
            "article_ref": article.get("article_ref"),
            "article_key": article.get("article_key"),
            "identity_key": article.get("identity_key"),
            "canonical_url": article.get("canonical_url") or article.get("seed_url"),
            "seed_url": article.get("seed_url"),
            "source_code": article.get("source_code"),
            "source_strategy": article.get("source_strategy"),
            "status": status,
            "diagnostic_code": diagnostic_code,
            "code": diagnostic_code,
            "failure_reason": evidence.get("failure_reason") or runtime_row.get("failure_reason"),
            "runtime_status": runtime_row.get("status"),
            "runtime_diagnostic_code": runtime_row.get("diagnostic_code")
            or runtime_row.get("code"),
            "runtime_evidence_count": runtime_evidence_count,
            "runtime_chunk_count": runtime_chunk_count,
            "zero_chunk": zero_chunk,
            "parser_ready_from_conversion": evidence.get("parser_ready_from_conversion") is True
            or runtime_row.get("parser_ready_from_conversion") is True,
            "evidence_path": evidence.get("evidence_path"),
            "runtime_event_log_path": evidence.get("runtime_event_log_path")
            or runtime_row.get("runtime_event_log_path"),
            "converted_text_path": evidence.get("converted_text_path")
            or runtime_row.get("converted_text_path"),
            "replay_record_path": rel(replay_path),
            "network_fetch_attempted": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
            "trusted_kg_import_allowed": False,
            "graph_import_allowed": False,
            "raw_text_embedded_in_metadata": False,
            "raw_binary_embedded_in_metadata": False,
            "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
        }
        rows.append(row)
    return rows


def build_summary(
    *,
    selection: Mapping[str, Any],
    runtime_summary: Mapping[str, Any],
    output_dir: Path,
    rows: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    status_counts = Counter(str(row.get("status", "unknown")) for row in rows)
    source_strategy_counts = Counter(str(row.get("source_strategy", "unknown")) for row in rows)
    zero_chunk_count = sum(1 for row in rows if row.get("zero_chunk") is True)
    loaded_count = len(rows) - zero_chunk_count
    summary_path, diagnostics_path, report_path = replay_output_paths(output_dir)
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": selection.get("selection_id", SELECTION_ID),
        "status": "passed" if rows else "failed",
        "created_at": utc_now(),
        "article_count": len(rows),
        "selection_article_count": len(selection.get("articles", []))
        if isinstance(selection.get("articles"), list)
        else None,
        "runtime_smoke_article_count": runtime_summary.get("article_count"),
        "runtime_smoke_loaded_count": runtime_summary.get("runtime_loaded_count"),
        "runtime_smoke_zero_chunk_count": runtime_summary.get("zero_chunk_count"),
        "runtime_smoke_evidence_count": runtime_summary.get("runtime_evidence_count"),
        "runtime_loaded_count": loaded_count,
        "zero_chunk_count": zero_chunk_count,
        "runtime_evidence_count": sum(int(row.get("runtime_evidence_count", 0)) for row in rows),
        "runtime_chunk_count": sum(int(row.get("runtime_chunk_count", 0)) for row in rows),
        "counts": dict(sorted(status_counts.items())),
        "source_strategy_counts": dict(sorted(source_strategy_counts.items())),
        "replay_dir": rel(output_dir),
        "replay_summary_path": rel(summary_path),
        "replay_diagnostics_path": rel(diagnostics_path),
        "replay_report_path": rel(report_path),
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


def render_report(summary: Mapping[str, Any], rows: Sequence[Mapping[str, Any]]) -> str:
    lines = [
        "# M029 Unified Replay Closure",
        "",
        f"- Schema: `{summary['schema_version']}`",
        f"- Selection: `{summary['selection_id']}`",
        f"- Article coverage: {summary['article_count']} / {summary['selection_article_count']}",
        f"- Runtime loaded count: {summary['runtime_loaded_count']}",
        f"- Zero-chunk count: {summary['zero_chunk_count']}",
        f"- Runtime evidence count: {summary['runtime_evidence_count']}",
        f"- Runtime chunk count: {summary['runtime_chunk_count']}",
        "",
        "## Article Coverage",
        "",
        "| Article | Identity | Source strategy | Replay status | Evidence | Chunks | Runtime diagnostic | Failure reason |",
        "|---|---|---|---:|---:|---:|---|---|",
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
                    str(row.get("runtime_diagnostic_code")),
                    str(row.get("failure_reason") or ""),
                ]
            )
            + " |"
        )
    lines.extend(
        [
            "",
            "## Evidence Surfaces",
            "",
            "Replay records point to local S04 evidence JSON and loader event paths where present. "
            "No article body text, binary payload, model output, graph write, or production import payload is embedded in this report.",
            "",
            "## Safety Flags",
            "",
            f"- Network fetch attempted: `{str(summary['network_fetch_attempted']).lower()}`",
            f"- Production import attempted: `{str(summary['production_import_attempted']).lower()}`",
            f"- LadybugDB written: `{str(summary['ladybugdb_written']).lower()}`",
            f"- Graph import allowed: `{str(summary['graph_import_allowed']).lower()}`",
            f"- Trusted KG import allowed: `{str(summary['trusted_kg_import_allowed']).lower()}`",
            f"- Raw text embedded in metadata: `{str(summary['raw_text_embedded_in_metadata']).lower()}`",
            f"- Raw binary embedded in metadata: `{str(summary['raw_binary_embedded_in_metadata']).lower()}`",
            "",
        ]
    )
    return "\n".join(lines)


def write_replay_records(output_dir: Path, rows: Sequence[Mapping[str, Any]]) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    for stale in output_dir.glob("*.replay.json"):
        stale.unlink()
    for row in rows:
        path = Path(str(row["replay_record_path"]))
        write_json(path, row)


def run(args: argparse.Namespace) -> int:
    selection_path = Path(args.selection)
    runtime_summary_path = Path(args.runtime_smoke_summary)
    evidence_dir = Path(args.evidence_dir)
    output_dir = Path(args.output_dir)
    corpus_dir = output_dir.parent
    _artifact_root = corpus_dir.parents[2] if len(corpus_dir.parents) >= 3 else ROOT
    selection = load_json(selection_path)
    runtime_summary = load_json(runtime_summary_path)
    evidence_records = load_evidence_records(evidence_dir)
    if not evidence_dir.resolve().is_relative_to(corpus_dir.resolve()):
        raise ValueError("evidence_dir_outside_corpus")
    rows = build_replay_rows(
        selection=selection,
        runtime_summary=runtime_summary,
        evidence_records=evidence_records,
        output_dir=output_dir,
    )
    summary = build_summary(
        selection=selection, runtime_summary=runtime_summary, output_dir=output_dir, rows=rows
    )
    summary_path, diagnostics_path, report_path = replay_output_paths(output_dir)
    write_replay_records(output_dir, rows)
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
    parser.add_argument("--runtime-smoke-summary", required=True)
    parser.add_argument("--evidence-dir", required=True)
    parser.add_argument("--output-dir", required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    parsed = parser.parse_args(argv[1:] if argv else None)
    try:
        return run(parsed)
    except Exception as exc:
        sys.stderr.write(f"unified replay failed: {type(exc).__name__}: {exc}\n")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
