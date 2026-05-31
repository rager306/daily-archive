#!/usr/bin/env python3
"""Run the M025 final local preprocessing replay.

This verifier is intentionally filesystem-only. It reads the fixed article catalog,
catalog index, corpus selection, and local artifacts produced by earlier M025
slices, then writes one final per-article replay artifact plus a JSONL event log.
The output is a safety/readiness handoff for S08; it never fetches network data,
never imports graph facts, and never writes to production LadybugDB.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m025-article-preprocessing-final-replay.v00.01"
ARTIFACT_SCHEMA_VERSION = "m025-article-preprocessing-final-artifact.v00.01"
EVENT_SCHEMA_VERSION = "m025-article-preprocessing-final-event.v00.01"
EVIDENCE_TYPES = ("assets", "tables", "links", "identity")
BASELINE_CATEGORIES = {
    "exact_match",
    "metric_delta",
    "improved",
    "regressed",
    "baseline_missing",
    "not_applicable",
}
FALSE_SAFETY_FLAGS = {
    "graph_import_allowed": False,
    "trusted_kg_import_allowed": False,
    "production_import_attempted": False,
    "production_ladybugdb_write_allowed": False,
    "ladybugdb_written": False,
}


class FinalReplayError(RuntimeError):
    """Raised when final replay cannot safely run from local artifacts."""


@dataclass(frozen=True)
class ArticleSelection:
    article_ref: str
    source_code: str
    selection_role: str


def _load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise FinalReplayError(f"required local input is missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise FinalReplayError(f"required local input is not valid JSON: {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise FinalReplayError(f"required local input must be a JSON object: {path}")
    return payload


def _article_slug(article_ref: str) -> str:
    return article_ref.replace("/", "-").replace(":", "-")


def _catalog_by_ref(index_payload: dict[str, Any]) -> dict[str, dict[str, Any]]:
    articles = index_payload.get("articles")
    if not isinstance(articles, list):
        raise FinalReplayError("catalog index does not contain an articles list")
    result: dict[str, dict[str, Any]] = {}
    for article in articles:
        if isinstance(article, dict) and isinstance(article.get("article_ref"), str):
            result[str(article["article_ref"])] = article
    return result


def _selection_articles(selection_payload: dict[str, Any]) -> list[ArticleSelection]:
    articles = selection_payload.get("articles")
    if not isinstance(articles, list) or not articles:
        raise FinalReplayError("selection does not contain a non-empty articles list")
    selections: list[ArticleSelection] = []
    for idx, article in enumerate(articles):
        if not isinstance(article, dict):
            raise FinalReplayError(f"selection article at index {idx} is not an object")
        article_ref = article.get("article_ref")
        source_code = article.get("source_code")
        if not isinstance(article_ref, str) or not article_ref:
            raise FinalReplayError(f"selection article at index {idx} is missing article_ref")
        if not isinstance(source_code, str) or not source_code:
            raise FinalReplayError(f"selection article {article_ref} is missing source_code")
        selections.append(
            ArticleSelection(
                article_ref=article_ref,
                source_code=source_code,
                selection_role=str(article.get("selection_role") or "selected"),
            )
        )
    return selections


def _event(event_type: str, **fields: Any) -> dict[str, Any]:
    return {"schema_version": EVENT_SCHEMA_VERSION, "event_type": event_type, **fields}


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _count_chunks(path: Path) -> tuple[int, list[dict[str, Any]]]:
    payload = _load_json(path)
    chunks = payload.get("chunks") or payload.get("items") or []
    if not isinstance(chunks, list):
        raise FinalReplayError(f"chunk artifact has non-list chunks/items: {path}")
    diagnostics = payload.get("diagnostics") if isinstance(payload.get("diagnostics"), list) else []
    return len([chunk for chunk in chunks if isinstance(chunk, dict)]), [d for d in diagnostics if isinstance(d, dict)]


def _evidence_metrics(paths: dict[str, Path]) -> tuple[dict[str, int], dict[str, int]]:
    counts: dict[str, int] = {}
    diagnostics: dict[str, int] = {}
    for evidence_type, path in paths.items():
        payload = _load_json(path)
        summary = payload.get("summary") if isinstance(payload.get("summary"), dict) else {}
        counts[evidence_type] = int(summary.get("item_count") or 0)
        diagnostics[evidence_type] = int(summary.get("diagnostic_count") or 0)
    return counts, diagnostics


def _candidate_baseline_paths(baseline: Path, slug: str) -> list[Path]:
    return [baseline / slug / "final.json", baseline / slug / "preprocessing-final.json", baseline / f"{slug}.json"]


def _baseline_comparison(baseline: Path, slug: str, metrics: dict[str, Any]) -> dict[str, Any]:
    if not baseline.exists():
        return {
            "category": "baseline_missing",
            "baseline_path": str(baseline),
            "message": "Baseline directory is absent; final replay is blocked from claiming parity.",
            "metric_deltas": {},
        }
    baseline_path = next((path for path in _candidate_baseline_paths(baseline, slug) if path.exists()), None)
    if baseline_path is None:
        return {
            "category": "baseline_missing",
            "baseline_path": str(baseline),
            "message": "No per-article baseline artifact was found for this article.",
            "metric_deltas": {},
        }
    baseline_payload = _load_json(baseline_path)
    baseline_metrics = baseline_payload.get("metrics") if isinstance(baseline_payload.get("metrics"), dict) else {}
    current_flat = {
        "chunk_count": int(metrics.get("chunk_count") or 0),
        **{f"evidence_{key}_count": int(value) for key, value in dict(metrics.get("evidence_counts") or {}).items()},
    }
    baseline_flat = {
        "chunk_count": int(baseline_metrics.get("chunk_count") or 0),
        **{f"evidence_{key}_count": int(value) for key, value in dict(baseline_metrics.get("evidence_counts") or {}).items()},
    }
    deltas = {key: current_flat.get(key, 0) - baseline_flat.get(key, 0) for key in sorted(set(current_flat) | set(baseline_flat))}
    category = "exact_match" if all(delta == 0 for delta in deltas.values()) else "metric_delta"
    return {
        "category": category,
        "baseline_path": str(baseline_path),
        "message": "Compared final replay metrics with the local baseline artifact.",
        "metric_deltas": deltas,
    }


def _resolve_local_inputs(corpus_root: Path, article_ref: str) -> tuple[Path, dict[str, Path]]:
    slug = _article_slug(article_ref)
    chunk_path = corpus_root / "chunking" / slug / "chunks.json"
    if not chunk_path.exists():
        raise FinalReplayError(f"missing local chunking artifact for {article_ref}: {chunk_path}")
    evidence_paths = {evidence_type: corpus_root / "evidence" / slug / f"{evidence_type}.json" for evidence_type in EVIDENCE_TYPES}
    missing = [str(path) for path in evidence_paths.values() if not path.exists()]
    if missing:
        raise FinalReplayError(f"missing local evidence artifact(s) for {article_ref}: {', '.join(missing)}")
    return chunk_path, evidence_paths


def run_replay(args: argparse.Namespace) -> list[dict[str, Any]]:
    if not getattr(args, "no_network", False):
        raise FinalReplayError("final replay requires --no-network so missing local artifacts fail closed")
    catalog = _load_json(args.catalog)
    index = _load_json(args.index)
    selection = _load_json(args.selection)
    catalog_refs = _catalog_by_ref(index)
    selected = _selection_articles(selection)
    corpus_root = args.selection.parent
    args.final.mkdir(parents=True, exist_ok=True)

    events: list[dict[str, Any]] = [
        _event(
            "final_replay.started",
            catalog_schema_version=catalog.get("schema_version"),
            index_schema_version=index.get("schema_version"),
            selection_id=selection.get("selection_id"),
            article_count=len(selected),
            no_network=True,
            network_fetch_attempted=False,
            baseline_path=str(args.baseline),
        )
    ]
    for article in selected:
        catalog_entry = catalog_refs.get(article.article_ref)
        if catalog_entry is None:
            raise FinalReplayError(f"selection article {article.article_ref} is absent from catalog index")
        slug = _article_slug(article.article_ref)
        chunk_path, evidence_paths = _resolve_local_inputs(corpus_root, article.article_ref)
        chunk_count, chunk_diagnostics = _count_chunks(chunk_path)
        evidence_counts, evidence_diagnostics = _evidence_metrics(evidence_paths)
        metrics = {
            "chunk_count": chunk_count,
            "chunk_diagnostic_count": len(chunk_diagnostics),
            "evidence_counts": evidence_counts,
            "evidence_diagnostic_counts": evidence_diagnostics,
            "artifact_reference_count": 1 + len(evidence_paths),
        }
        comparison = _baseline_comparison(args.baseline, slug, metrics)
        if comparison["category"] not in BASELINE_CATEGORIES:
            raise FinalReplayError(f"unsupported baseline comparison category: {comparison['category']}")
        diagnostics = list(chunk_diagnostics)
        if comparison["category"] == "baseline_missing":
            diagnostics.append(
                {
                    "code": "BASELINE_ARTIFACT_MISSING",
                    "severity": "warning",
                    "json_path": "$.baseline_comparison",
                    "message": comparison["message"],
                }
            )
        artifact = {
            "schema_version": ARTIFACT_SCHEMA_VERSION,
            "article_ref": article.article_ref,
            "source_code": article.source_code,
            "selection_role": article.selection_role,
            "catalog_ref": {
                "article_path": catalog_entry.get("article_path"),
                "article_key": catalog_entry.get("article_key"),
                "primary_source_role": catalog_entry.get("primary_source_role"),
                "title": catalog_entry.get("title"),
            },
            "local_inputs": {
                "catalog": str(args.catalog),
                "index": str(args.index),
                "selection": str(args.selection),
                "chunking": str(chunk_path),
                "evidence": {key: str(path) for key, path in evidence_paths.items()},
            },
            "final_artifact_refs": {
                "chunking": str(chunk_path),
                "assets": str(evidence_paths["assets"]),
                "tables": str(evidence_paths["tables"]),
                "links": str(evidence_paths["links"]),
                "identity": str(evidence_paths["identity"]),
            },
            "metrics": metrics,
            "baseline_comparison": comparison,
            "diagnostics": diagnostics,
            "network": {"no_network_required": True, "network_fetch_attempted": False},
            "safety_state": {"metadata_only": True, "review_only": True, **FALSE_SAFETY_FLAGS},
            "readiness": {
                "preprocessing_replay_completed": True,
                "larger_validation_ready": comparison["category"] != "baseline_missing",
                "blocked_reason": "baseline_missing" if comparison["category"] == "baseline_missing" else None,
            },
        }
        output_path = args.final / slug / "final.json"
        _write_json(output_path, artifact)
        events.append(
            _event(
                "final_replay.article_completed",
                article_ref=article.article_ref,
                path=str(output_path),
                chunk_count=chunk_count,
                evidence_counts=evidence_counts,
                baseline_comparison_category=comparison["category"],
                no_network=True,
                network_fetch_attempted=False,
                graph_import_allowed=False,
                production_import_attempted=False,
                ladybugdb_written=False,
            )
        )
    events.append(
        _event(
            "final_replay.completed",
            article_count=len(selected),
            final_path=str(args.final),
            no_network=True,
            network_fetch_attempted=False,
            graph_import_allowed=False,
            production_import_attempted=False,
            ladybugdb_written=False,
        )
    )
    return events


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog", required=True, type=Path)
    parser.add_argument("--index", required=True, type=Path)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--final", required=True, type=Path)
    parser.add_argument("--write-events", required=True, type=Path)
    parser.add_argument("--no-network", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        events = run_replay(args)
        args.write_events.parent.mkdir(parents=True, exist_ok=True)
        args.write_events.write_text("".join(json.dumps(event, sort_keys=True) + "\n" for event in events), encoding="utf-8")
        completed = sum(1 for event in events if event["event_type"] == "final_replay.article_completed")
        sys.stdout.write(
            f"wrote final preprocessing replay for {completed} articles to {args.final}; events={args.write_events}\n"
        )
        return 0
    except FinalReplayError as exc:
        sys.stderr.write(f"ERROR: {exc}\n")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
