#!/usr/bin/env python3
"""Replay the local article loader for M025 catalog source variants.

The replay resolves article records exclusively through ``index.json`` and writes
redacted loader events/summaries under each catalog article directory.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

from arxiv_archive.article_loader import load_article_source


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def safe_catalog_path(root: Path, rel_path: str) -> Path:
    normalized = PurePosixPath(rel_path.replace("\\", "/"))
    if normalized.is_absolute() or ".." in normalized.parts:
        raise ValueError(f"unsafe catalog-relative path: {rel_path}")
    resolved = (root / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root.resolve()):
        raise ValueError(f"path escapes catalog root: {rel_path}")
    return resolved


def redact_result(result: Any, variant: dict[str, Any]) -> dict[str, Any]:
    return {
        "variant_id": variant.get("variant_id"),
        "source_role": variant.get("source_role"),
        "source_format": variant.get("source_format"),
        "source_path": str(result.source_path),
        "source_type": result.source_type,
        "media_type": result.media_type,
        "sha256": result.sha256,
        "byte_size": result.byte_size,
        "source_id": result.source_id,
        "parser_name": result.parser_name,
        "loader_name": result.loader_name,
        "outcome": result.outcome,
        "failure_reason": result.failure_reason,
        "warning_count": result.warning_count,
        "warnings": result.warnings,
        "duration_ms": result.duration_ms,
        "text_present": result.text is not None,
    }


def selected_article_entries(index: dict[str, Any], selection: dict[str, Any]) -> list[dict[str, Any]]:
    by_ref = {row["article_ref"]: row for row in index.get("articles", []) if isinstance(row, dict) and "article_ref" in row}
    entries: list[dict[str, Any]] = []
    for row in selection.get("articles", []):
        article_ref = row.get("article_ref") if isinstance(row, dict) else None
        if not isinstance(article_ref, str) or article_ref not in by_ref:
            raise ValueError(f"selection article not present in index: {article_ref}")
        entries.append(by_ref[article_ref])
    return entries


def replay_article(catalog_root: Path, entry: dict[str, Any]) -> dict[str, Any]:
    article_path = safe_catalog_path(catalog_root, str(entry["article_path"]))
    article = load_json(article_path)
    variants = article.get("source_variants")
    if not isinstance(variants, list) or not variants:
        raise ValueError(f"article has no source variants: {entry['article_ref']}")
    loader_dir = article_path.parent / "loader"
    event_path = loader_dir / "events.jsonl"
    if event_path.exists():
        event_path.unlink()

    variant_results: list[dict[str, Any]] = []
    for variant in variants:
        source_path = safe_catalog_path(article_path.parent, str(variant["path"]))
        result = load_article_source(source_path, log_path=event_path, paper_id=str(article.get("article_key")))
        redacted = redact_result(result, variant)
        variant_results.append(redacted)
        variant["loader_outcome"] = result.outcome
        variant["loader_failure_reason"] = result.failure_reason
        variant["loader_warning_count"] = result.warning_count
        variant["loader_text_present"] = result.text is not None
        variant["loader_event_log"] = "loader/events.jsonl"
        variant["loader_summary"] = "loader/summary.json"
        variant["raw_text_embedded"] = False
        variant["raw_binary_embedded"] = False

    counts = Counter(row["outcome"] for row in variant_results)
    summary = {
        "schema_version": "m025-article-loader-summary.v00.01",
        "article_ref": entry["article_ref"],
        "article_path": entry["article_path"],
        "generated_at": datetime.now(UTC).isoformat().replace("+00:00", "Z"),
        "lookup_surface": "index.json",
        "full_tree_scan_attempted": False,
        "network_fetch_attempted": False,
        "variant_count": len(variant_results),
        "outcome_counts": dict(sorted(counts.items())),
        "variants": variant_results,
    }
    write_json(loader_dir / "summary.json", summary)
    article["source_variants"] = variants
    article["loader_summary"] = {
        "status": "replayed",
        "path": "loader/summary.json",
        "events_path": "loader/events.jsonl",
        "outcome_counts": dict(sorted(counts.items())),
        "raw_payload_logged": False,
        "updated_at": datetime.now(UTC).isoformat().replace("+00:00", "Z"),
    }
    write_json(article_path, article)
    return summary


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog", required=True, type=Path)
    parser.add_argument("--index", required=True, type=Path)
    parser.add_argument("--selection", required=True, type=Path)
    args = parser.parse_args(argv[1:])

    index = load_json(args.index)
    selection = load_json(args.selection)
    catalog_root = args.catalog.parent.resolve()
    summaries = [replay_article(catalog_root, entry) for entry in selected_article_entries(index, selection)]
    total = sum(summary["variant_count"] for summary in summaries)
    counts = Counter()
    for summary in summaries:
        counts.update(summary["outcome_counts"])
    print(f"replayed loader for {len(summaries)} articles / {total} variants; outcomes={dict(sorted(counts.items()))}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
