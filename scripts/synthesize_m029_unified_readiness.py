#!/usr/bin/env python3
"""Synthesize S06 readiness artifacts for the M029 unified corpus.

This script is metadata-only and fail-closed. It reads the authoritative unified
selection plus S04 runtime-smoke and S05 replay summaries, then writes a final
readiness summary, decision, and fresh-reader report. It does not fetch network
content, import facts, write LadybugDB, or embed article body/binary payloads.
"""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
from collections import Counter, defaultdict
from collections.abc import Iterable, Mapping, Sequence
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

MILESTONE_ID = "M029-eb0ljz"
SLICE_ID = "S06"
SELECTION_ID = "m029-unified-corpus-v1"
SUMMARY_SCHEMA_VERSION = "m029-unified-readiness-summary.v1"
DECISION_SCHEMA_VERSION = "m029-unified-readiness-decision.v1"
DIAGNOSTIC_SCHEMA_VERSION = "m029-unified-readiness-diagnostic.v1"
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
    "ready_for_graph_import",
    "ready_for_production_import",
    "ready_for_trusted_kg",
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
        article_ref = article_key_for(article)
        if article_ref in seen:
            raise ValueError(f"duplicate selection article identity: {article_ref}")
        seen.add(article_ref)
        normalized.append(dict(article))
    return normalized


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


def readiness_paths(output_dir: Path) -> tuple[Path, Path, Path, Path]:
    corpus_dir = output_dir.parent
    return (
        corpus_dir / "readiness-summary.json",
        corpus_dir / "readiness-decision.json",
        corpus_dir / "readiness-report.md",
        output_dir / "readiness-diagnostics.jsonl",
    )


def provenance_counts(articles: Sequence[Mapping[str, Any]]) -> dict[str, int]:
    counts: Counter[str] = Counter()
    for article in articles:
        sources = article.get("provenance_sources")
        if isinstance(sources, list):
            for source in sources:
                if isinstance(source, str) and source:
                    counts[source] += 1
    return dict(sorted(counts.items()))


def build_rows(
    *,
    selection: Mapping[str, Any],
    replay_summary: Mapping[str, Any],
    runtime_smoke_summary: Mapping[str, Any],
) -> list[dict[str, Any]]:
    articles = selected_articles(selection)
    replay_rows = replay_summary.get("results")
    runtime_rows = runtime_smoke_summary.get("results")
    if not isinstance(replay_rows, list):
        raise ValueError("replay summary results must be a list")
    if not isinstance(runtime_rows, list):
        raise ValueError("runtime smoke summary results must be a list")
    replay_by_ref, replay_by_identity = index_by_identity(
        [row for row in replay_rows if isinstance(row, Mapping)]
    )
    runtime_by_ref, runtime_by_identity = index_by_identity(
        [row for row in runtime_rows if isinstance(row, Mapping)]
    )
    rows: list[dict[str, Any]] = []
    for article in articles:
        article_ref = article_key_for(article)
        identity_key = str(article.get("identity_key") or "")
        replay_row = replay_by_ref.get(article_ref) or replay_by_identity.get(identity_key)
        runtime_row = runtime_by_ref.get(article_ref) or runtime_by_identity.get(identity_key)
        if replay_row is None:
            raise ValueError(f"missing replay row for selected article: {article_ref}")
        if runtime_row is None:
            raise ValueError(f"missing runtime row for selected article: {article_ref}")
        unsafe = sorted(set(row_unsafe_flags(replay_row) + row_unsafe_flags(runtime_row)))
        if unsafe:
            raise ValueError(f"unsafe readiness input flags for {article_ref}: {','.join(unsafe)}")
        runtime_chunk_count = int(
            replay_row.get("runtime_chunk_count", runtime_row.get("runtime_chunk_count", 0)) or 0
        )
        runtime_evidence_count = int(
            replay_row.get("runtime_evidence_count", runtime_row.get("runtime_evidence_count", 0))
            or 0
        )
        zero_chunk = bool(
            replay_row.get("zero_chunk") is True
            or runtime_row.get("zero_chunk") is True
            or runtime_chunk_count == 0
        )
        if zero_chunk:
            readiness_status = "partial_zero_chunk_blocked"
            readiness_category = "partial"
            block_reason = str(
                replay_row.get("failure_reason")
                or runtime_row.get("failure_reason")
                or "zero chunks"
            )
        else:
            readiness_status = "ready_for_local_replay_review"
            readiness_category = "ready"
            block_reason = None
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
            "provenance_sources": article.get("provenance_sources", []),
            "provenance_url_count": article.get("provenance_url_count"),
            "readiness_status": readiness_status,
            "readiness_category": readiness_category,
            "block_reason": block_reason,
            "runtime_status": runtime_row.get("status"),
            "runtime_diagnostic_code": runtime_row.get("diagnostic_code")
            or runtime_row.get("code"),
            "replay_status": replay_row.get("status"),
            "replay_diagnostic_code": replay_row.get("diagnostic_code") or replay_row.get("code"),
            "runtime_evidence_count": runtime_evidence_count,
            "runtime_chunk_count": runtime_chunk_count,
            "zero_chunk": zero_chunk,
            "parser_ready_from_conversion": replay_row.get("parser_ready_from_conversion") is True
            or runtime_row.get("parser_ready_from_conversion") is True,
            "evidence_path": replay_row.get("evidence_path"),
            "replay_record_path": replay_row.get("replay_record_path"),
            "runtime_event_log_path": replay_row.get("runtime_event_log_path")
            or runtime_row.get("runtime_event_log_path"),
            **FAIL_CLOSED_SAFETY_FLAGS,
            "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
        }
        rows.append(row)
    return rows


def build_summary(
    *,
    selection: Mapping[str, Any],
    replay_summary: Mapping[str, Any],
    runtime_smoke_summary: Mapping[str, Any],
    rows: Sequence[Mapping[str, Any]],
    output_dir: Path,
) -> dict[str, Any]:
    summary_path, decision_path, report_path, diagnostics_path = readiness_paths(output_dir)
    category_counts = Counter(str(row.get("readiness_category", "unknown")) for row in rows)
    status_counts = Counter(str(row.get("readiness_status", "unknown")) for row in rows)
    source_strategy_counts = Counter(str(row.get("source_strategy", "unknown")) for row in rows)
    block_reason_counts: defaultdict[str, int] = defaultdict(int)
    for row in rows:
        if row.get("readiness_category") != "ready":
            block_reason_counts[str(row.get("block_reason") or "unspecified")] += 1
    return {
        "schema_version": SUMMARY_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": selection.get("selection_id", SELECTION_ID),
        "status": "partial" if category_counts.get("partial", 0) else "ready",
        "created_at": utc_now(),
        "article_count": len(rows),
        "selection_article_count": len(selection.get("articles", []))
        if isinstance(selection.get("articles"), list)
        else None,
        "unique_identity_count": len({str(row.get("identity_key")) for row in rows}),
        "dedupe_rule": "one selected article per article_ref/identity_key; provenance_sources preserve earlier milestone subset membership without inflating article_count",
        "ready_count": category_counts.get("ready", 0),
        "partial_count": category_counts.get("partial", 0),
        "blocked_count": category_counts.get("blocked", 0),
        "zero_chunk_count": sum(1 for row in rows if row.get("zero_chunk") is True),
        "runtime_loaded_count": sum(1 for row in rows if row.get("zero_chunk") is not True),
        "runtime_evidence_count": sum(int(row.get("runtime_evidence_count", 0)) for row in rows),
        "runtime_chunk_count": sum(int(row.get("runtime_chunk_count", 0)) for row in rows),
        "counts": dict(sorted(status_counts.items())),
        "readiness_category_counts": dict(sorted(category_counts.items())),
        "block_reason_counts": dict(sorted(block_reason_counts.items())),
        "source_strategy_counts": dict(sorted(source_strategy_counts.items())),
        "provenance_source_counts": provenance_counts(selected_articles(selection)),
        "upstream_artifacts": {
            "selection": rel(
                Path(
                    str(
                        selection.get(
                            "selection_path",
                            "data/article_corpora/m029-unified-corpus-v1/selection.json",
                        )
                    )
                )
            )
            if selection.get("selection_path")
            else None,
            "replay_summary": rel(
                Path(
                    str(
                        replay_summary.get(
                            "replay_summary_path",
                            "data/article_corpora/m029-unified-corpus-v1/replay-summary.json",
                        )
                    )
                )
            )
            if replay_summary.get("replay_summary_path")
            else None,
            "runtime_smoke_summary": rel(
                Path(
                    str(
                        runtime_smoke_summary.get(
                            "runtime_summary_path",
                            "data/article_corpora/m029-unified-corpus-v1/runtime-smoke-summary.json",
                        )
                    )
                )
            )
            if runtime_smoke_summary.get("runtime_summary_path")
            else None,
        },
        "readiness_summary_path": rel(summary_path),
        "readiness_decision_path": rel(decision_path),
        "readiness_report_path": rel(report_path),
        "readiness_diagnostics_path": rel(diagnostics_path),
        **FAIL_CLOSED_SAFETY_FLAGS,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
        "results": list(rows),
    }


def build_decision(summary: Mapping[str, Any]) -> dict[str, Any]:
    partial_count = int(summary.get("partial_count", 0) or 0)
    ready_count = int(summary.get("ready_count", 0) or 0)
    article_count = int(summary.get("article_count", 0) or 0)
    if partial_count:
        decision = "partial_preprocessing_ready"
        headline = f"{ready_count} of {article_count} articles are ready for local replay review; {partial_count} remain partial because they have zero chunks."
        allowed_next_steps = [
            "Use ready articles for metadata-only local replay review.",
            "Inspect zero-chunk partial articles via the provenance and diagnostic paths before any parser/readiness promotion.",
        ]
    else:
        decision = "local_replay_ready"
        headline = f"All {article_count} articles are ready for local replay review."
        allowed_next_steps = ["Use the corpus for metadata-only local replay review."]
    return {
        "schema_version": DECISION_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": summary.get("selection_id", SELECTION_ID),
        "created_at": summary.get("created_at"),
        "decision": decision,
        "headline": headline,
        "ready_count": ready_count,
        "partial_count": partial_count,
        "blocked_count": int(summary.get("blocked_count", 0) or 0),
        "article_count": article_count,
        "block_reason_counts": summary.get("block_reason_counts", {}),
        "dedupe_rule": summary.get("dedupe_rule"),
        "provenance_source_counts": summary.get("provenance_source_counts", {}),
        "allowed_next_steps": allowed_next_steps,
        "disallowed_next_steps": [
            "Do not claim graph import readiness.",
            "Do not write LadybugDB or production graph state from these artifacts.",
            "Do not collapse zero-chunk partial articles into ready counts.",
            "Do not fetch network content during readiness verification.",
        ],
        **FAIL_CLOSED_SAFETY_FLAGS,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
    }


def render_report(
    summary: Mapping[str, Any], decision: Mapping[str, Any], rows: Sequence[Mapping[str, Any]]
) -> str:
    lines = [
        "# M029 Unified Corpus Readiness Report",
        "",
        f"- Decision: `{decision['decision']}`",
        f"- Headline: {decision['headline']}",
        f"- Article count: {summary['article_count']}",
        f"- Ready count: {summary['ready_count']}",
        f"- Partial count: {summary['partial_count']}",
        f"- Blocked count: {summary['blocked_count']}",
        f"- Runtime evidence count: {summary['runtime_evidence_count']}",
        f"- Runtime chunk count: {summary['runtime_chunk_count']}",
        "",
        "## Dedupe and Provenance",
        "",
        str(summary["dedupe_rule"]),
        "",
        "### Provenance source counts",
        "",
    ]
    for source, count in dict(summary.get("provenance_source_counts", {})).items():
        lines.append(f"- `{source}`: {count}")
    lines.extend(
        [
            "",
            "## Final Counts and Block Reasons",
            "",
            "| Category | Count |",
            "|---|---:|",
            f"| Ready | {summary['ready_count']} |",
            f"| Partial | {summary['partial_count']} |",
            f"| Blocked | {summary['blocked_count']} |",
            f"| Zero chunk | {summary['zero_chunk_count']} |",
            "",
            "### Block reasons",
            "",
        ]
    )
    block_reasons = dict(summary.get("block_reason_counts", {}))
    if block_reasons:
        for reason, count in block_reasons.items():
            lines.append(f"- `{reason}`: {count}")
    else:
        lines.append("- None.")
    lines.extend(
        [
            "",
            "## Article Readiness",
            "",
            "| Article | Identity | Provenance | Source strategy | Readiness | Evidence | Chunks | Block reason |",
            "|---|---|---|---|---|---:|---:|---|",
        ]
    )
    for row in rows:
        provenance = ",".join(str(item) for item in row.get("provenance_sources", []) if item)
        lines.append(
            "| "
            + " | ".join(
                [
                    str(row.get("article_ref") or row.get("identity_key")),
                    str(row.get("identity_key")),
                    provenance,
                    str(row.get("source_strategy")),
                    str(row.get("readiness_status")),
                    str(row.get("runtime_evidence_count", 0)),
                    str(row.get("runtime_chunk_count", 0)),
                    str(row.get("block_reason") or ""),
                ]
            )
            + " |"
        )
    lines.extend(
        [
            "",
            "## Boundary Decision",
            "",
            "This readiness decision is preprocessing/local-replay only. Graph import, trusted KG promotion, production import, LadybugDB writes, network fetches, and raw payload embedding remain fail-closed and out of scope.",
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


def run(args: argparse.Namespace) -> int:
    selection_path = Path(args.selection)
    replay_summary_path = Path(args.replay_summary)
    runtime_smoke_summary_path = Path(args.runtime_smoke_summary)
    output_dir = Path(args.output_dir)
    corpus_dir = output_dir.parent
    artifact_root = corpus_dir.parents[2] if len(corpus_dir.parents) >= 3 else ROOT
    if not output_dir.resolve().is_relative_to(corpus_dir.resolve()):
        raise ValueError("output_dir_outside_corpus")
    for path, label in [
        (selection_path, "selection"),
        (replay_summary_path, "replay_summary"),
        (runtime_smoke_summary_path, "runtime_smoke_summary"),
    ]:
        if not path.resolve().is_relative_to(artifact_root.resolve()):
            raise ValueError(f"{label}_outside_artifact_root")
    selection = load_json(selection_path)
    replay_summary = load_json(replay_summary_path)
    runtime_smoke_summary = load_json(runtime_smoke_summary_path)
    selection["selection_path"] = rel(selection_path)
    replay_summary["replay_summary_path"] = rel(replay_summary_path)
    runtime_smoke_summary["runtime_summary_path"] = rel(runtime_smoke_summary_path)
    for source_name, source in [
        ("replay_summary", replay_summary),
        ("runtime_smoke_summary", runtime_smoke_summary),
    ]:
        unsafe = row_unsafe_flags(source)
        if unsafe:
            raise ValueError(f"unsafe {source_name} flags: {','.join(unsafe)}")
    rows = build_rows(
        selection=selection,
        replay_summary=replay_summary,
        runtime_smoke_summary=runtime_smoke_summary,
    )
    summary = build_summary(
        selection=selection,
        replay_summary=replay_summary,
        runtime_smoke_summary=runtime_smoke_summary,
        rows=rows,
        output_dir=output_dir,
    )
    decision = build_decision(summary)
    summary_path, decision_path, report_path, diagnostics_path = readiness_paths(output_dir)
    write_json(summary_path, summary)
    write_json(decision_path, decision)
    write_jsonl(diagnostics_path, rows)
    atomic_write_text(report_path, render_report(summary, decision, rows))
    sys.stdout.write(
        json.dumps(
            {
                "status": summary["status"],
                "decision": decision["decision"],
                "article_count": summary["article_count"],
                "ready_count": summary["ready_count"],
                "partial_count": summary["partial_count"],
                "summary_path": rel(summary_path),
                "decision_path": rel(decision_path),
                "report_path": rel(report_path),
                "diagnostics_path": rel(diagnostics_path),
            },
            sort_keys=True,
        )
        + "\n"
    )
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True)
    parser.add_argument("--replay-summary", required=True)
    parser.add_argument("--runtime-smoke-summary", required=True)
    parser.add_argument("--output-dir", required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    parsed = parser.parse_args(argv[1:] if argv else None)
    try:
        return run(parsed)
    except Exception as exc:
        sys.stderr.write(f"unified readiness synthesis failed: {type(exc).__name__}: {exc}\n")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
