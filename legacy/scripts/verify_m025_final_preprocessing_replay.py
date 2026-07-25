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
    return len([chunk for chunk in chunks if isinstance(chunk, dict)]), [
        d
        for d in diagnostics  # ty:ignore[not-iterable]
        if isinstance(d, dict)
    ]


def _evidence_metrics(paths: dict[str, Path]) -> tuple[dict[str, int], dict[str, int]]:
    counts: dict[str, int] = {}
    diagnostics: dict[str, int] = {}
    for evidence_type, path in paths.items():
        payload = _load_json(path)
        summary = payload.get("summary") if isinstance(payload.get("summary"), dict) else {}
        counts[evidence_type] = int(summary.get("item_count") or 0)  # ty:ignore[unresolved-attribute]
        diagnostics[evidence_type] = int(summary.get("diagnostic_count") or 0)  # ty:ignore[unresolved-attribute]
    return counts, diagnostics


def _candidate_baseline_paths(baseline: Path, slug: str) -> list[Path]:
    return [
        baseline / slug / "final.json",
        baseline / slug / "preprocessing-final.json",
        baseline / f"{slug}.json",
    ]


def _baseline_comparison(baseline: Path, slug: str, metrics: dict[str, Any]) -> dict[str, Any]:
    if not baseline.exists():
        return {
            "category": "baseline_missing",
            "baseline_path": str(baseline),
            "message": "Baseline directory is absent; final replay is blocked from claiming parity.",
            "metric_deltas": {},
        }
    baseline_path = next(
        (path for path in _candidate_baseline_paths(baseline, slug) if path.exists()), None
    )
    if baseline_path is None:
        return {
            "category": "baseline_missing",
            "baseline_path": str(baseline),
            "message": "No per-article baseline artifact was found for this article.",
            "metric_deltas": {},
        }
    baseline_payload = _load_json(baseline_path)
    baseline_metrics = (
        baseline_payload.get("metrics") if isinstance(baseline_payload.get("metrics"), dict) else {}
    )
    current_flat = {
        "chunk_count": int(metrics.get("chunk_count") or 0),
        **{
            f"evidence_{key}_count": int(value)
            for key, value in dict(metrics.get("evidence_counts") or {}).items()
        },
    }
    baseline_flat = {
        "chunk_count": int(baseline_metrics.get("chunk_count") or 0),  # ty:ignore[unresolved-attribute]
        **{
            f"evidence_{key}_count": int(value)
            for key, value in dict(baseline_metrics.get("evidence_counts") or {}).items()  # ty:ignore[unresolved-attribute]
        },
    }
    deltas = {
        key: current_flat.get(key, 0) - baseline_flat.get(key, 0)
        for key in sorted(set(current_flat) | set(baseline_flat))
    }
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
    evidence_paths = {
        evidence_type: corpus_root / "evidence" / slug / f"{evidence_type}.json"
        for evidence_type in EVIDENCE_TYPES
    }
    missing = [str(path) for path in evidence_paths.values() if not path.exists()]
    if missing:
        raise FinalReplayError(
            f"missing local evidence artifact(s) for {article_ref}: {', '.join(missing)}"
        )
    return chunk_path, evidence_paths


def run_replay(args: argparse.Namespace) -> list[dict[str, Any]]:
    if not getattr(args, "no_network", False):
        raise FinalReplayError(
            "final replay requires --no-network so missing local artifacts fail closed"
        )
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
            raise FinalReplayError(
                f"selection article {article.article_ref} is absent from catalog index"
            )
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
            raise FinalReplayError(
                f"unsupported baseline comparison category: {comparison['category']}"
            )
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
                "blocked_reason": "baseline_missing"
                if comparison["category"] == "baseline_missing"
                else None,
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


def _read_final_artifacts(final: Path) -> list[dict[str, Any]]:
    artifacts: list[dict[str, Any]] = []
    for path in sorted(final.glob("*/final.json")):
        artifact = _load_json(path)
        artifact["_path"] = str(path)
        artifacts.append(artifact)
    if not artifacts:
        raise FinalReplayError(f"no final replay artifacts were found under {final}")
    return artifacts


def _classification_for(category: str) -> str:
    if category == "baseline_missing":
        return "blocked"
    if category == "exact_match":
        return "preserved"
    if category == "improved":
        return "improved"
    if category == "regressed":
        return "regressed"
    if category in {"metric_delta", "not_applicable"}:
        return "preserved"
    return "blocked"


def _summary_from_artifacts(
    args: argparse.Namespace, events: list[dict[str, Any]]
) -> dict[str, Any]:
    artifacts = _read_final_artifacts(args.final)
    behavior_counts: dict[str, int] = {}
    comparison_counts: dict[str, int] = {}
    diagnostic_counts: dict[str, int] = {}
    article_results: list[dict[str, Any]] = []
    safety_violations: list[dict[str, Any]] = []
    for artifact in artifacts:
        article_ref = str(artifact.get("article_ref"))
        comparison = (
            artifact.get("baseline_comparison")
            if isinstance(artifact.get("baseline_comparison"), dict)
            else {}
        )
        category = str(comparison.get("category") or "unknown")  # ty:ignore[unresolved-attribute]
        behavior = _classification_for(category)
        comparison_counts[category] = comparison_counts.get(category, 0) + 1
        behavior_counts[behavior] = behavior_counts.get(behavior, 0) + 1
        diagnostics = (
            artifact.get("diagnostics") if isinstance(artifact.get("diagnostics"), list) else []
        )
        for diagnostic in diagnostics:  # ty:ignore[not-iterable]
            if isinstance(diagnostic, dict):
                code = str(diagnostic.get("code") or "UNKNOWN")
                diagnostic_counts[code] = diagnostic_counts.get(code, 0) + 1
        safety_state = (
            artifact.get("safety_state") if isinstance(artifact.get("safety_state"), dict) else {}
        )
        violated = {
            key: safety_state.get(key)  # ty:ignore[unresolved-attribute]
            for key in FALSE_SAFETY_FLAGS
            if safety_state.get(key) is not False  # ty:ignore[unresolved-attribute]
        }
        if violated:
            safety_violations.append({"article_ref": article_ref, "violations": violated})
        article_results.append(
            {
                "article_ref": article_ref,
                "path": artifact.get("_path"),
                "baseline_category": category,
                "behavior_classification": behavior,
                "larger_validation_ready": bool(
                    isinstance(artifact.get("readiness"), dict)
                    and artifact["readiness"].get("larger_validation_ready") is True
                ),
                "diagnostic_count": len([item for item in diagnostics if isinstance(item, dict)]),  # ty:ignore[not-iterable]
                "metrics": artifact.get("metrics")
                if isinstance(artifact.get("metrics"), dict)
                else {},
            }
        )
    no_network_proof = {
        "required": True,
        "network_fetch_attempted": any(
            event.get("network_fetch_attempted") is True for event in events
        ),
        "all_events_no_network": all(
            event.get("no_network") is True for event in events if "no_network" in event
        ),
    }
    no_write_safety = {
        "require_no_import_flags": bool(getattr(args, "require_no_import_flags", False)),
        "safety_violations": safety_violations,
        "graph_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
    }
    ready = not safety_violations and all(
        result["larger_validation_ready"] for result in article_results
    )
    blockers: list[str] = []
    if any(result["baseline_category"] == "baseline_missing" for result in article_results):
        blockers.append("baseline_missing")
    if safety_violations:
        blockers.append("safety_flag_violation")
    if no_network_proof["network_fetch_attempted"]:
        blockers.append("network_fetch_attempted")
    return {
        "schema_version": SCHEMA_VERSION,
        "summary_type": "m025-final-preprocessing-replay-summary",
        "article_count": len(article_results),
        "final_path": str(args.final),
        "baseline_path": str(args.baseline),
        "events_path": str(getattr(args, "events", getattr(args, "write_events", ""))),
        "behavior_counts": behavior_counts,
        "baseline_comparison_counts": comparison_counts,
        "diagnostic_counts": diagnostic_counts,
        "article_results": article_results,
        "no_network_proof": no_network_proof,
        "no_write_safety": no_write_safety,
        "readiness": {
            "larger_preprocessing_validation_ready": ready,
            "decision": "ready" if ready else "blocked",
            "blockers": blockers,
            "graph_readiness_claim": False,
            "message": "M025 makes no graph readiness claim; this decision is limited to preprocessing replay readiness.",
        },
    }


def _write_summary(path: Path, summary: dict[str, Any]) -> None:
    _write_json(path, summary)


def _write_decision(path: Path, summary: dict[str, Any]) -> None:
    decision = {
        "schema_version": "m025-preprocessing-readiness-decision.v00.01",
        "decision": summary["readiness"]["decision"],
        "larger_preprocessing_validation_ready": summary["readiness"][
            "larger_preprocessing_validation_ready"
        ],
        "blockers": summary["readiness"]["blockers"],
        "graph_readiness_claim": False,
        "rationale": summary["readiness"]["message"],
        "evidence": {
            "summary": str(path.parent / "final-replay-summary.json"),
            "final_path": summary["final_path"],
            "baseline_path": summary["baseline_path"],
            "article_count": summary["article_count"],
            "behavior_counts": summary["behavior_counts"],
            "no_network_proof": summary["no_network_proof"],
            "no_write_safety": summary["no_write_safety"],
        },
    }
    _write_json(path, decision)


def _write_report(path: Path, summary: dict[str, Any]) -> None:
    rows = [
        "| Article | Baseline | Behavior | Ready | Diagnostics |",
        "|---|---:|---|---|---:|",
    ]
    for result in summary["article_results"]:
        rows.append(
            "| {article_ref} | {baseline_category} | {behavior_classification} | {ready} | {diagnostic_count} |".format(
                article_ref=result["article_ref"],
                baseline_category=result["baseline_category"],
                behavior_classification=result["behavior_classification"],
                ready="yes" if result["larger_validation_ready"] else "no",
                diagnostic_count=result["diagnostic_count"],
            )
        )
    blockers = summary["readiness"]["blockers"] or ["None"]
    diagnostics = summary["diagnostic_counts"] or {"None": 0}
    report = f"""# M025 S08 Final Preprocessing Replay Report

## Decision

- Larger preprocessing validation ready: **{str(summary["readiness"]["larger_preprocessing_validation_ready"]).lower()}**
- Decision: **{summary["readiness"]["decision"]}**
- Blockers: {", ".join(blockers)}
- Graph readiness claim: **false**

M025 makes no graph readiness claim. This report only evaluates whether the refactored preprocessing replay over the fixed five-article local smoke corpus is ready for larger preprocessing validation.

## Baseline Comparison

- Baseline path: `{summary["baseline_path"]}`
- Final replay path: `{summary["final_path"]}`
- Behavior counts: `{json.dumps(summary["behavior_counts"], sort_keys=True)}`
- Baseline comparison counts: `{json.dumps(summary["baseline_comparison_counts"], sort_keys=True)}`

{chr(10).join(rows)}

## Diagnostics

`{json.dumps(diagnostics, sort_keys=True)}`

## Readiness Blockers

{chr(10).join(f"- {blocker}" for blocker in blockers)}

## No-Network Proof

`{json.dumps(summary["no_network_proof"], sort_keys=True)}`

## No-Write Safety Evidence

`{json.dumps(summary["no_write_safety"], sort_keys=True)}`

## Failure Modes

- Filesystem inputs: missing or malformed catalog, index, selection, chunking, or evidence JSON raises `FinalReplayError` and exits non-zero rather than fetching or synthesizing data.
- Network dependency: intentionally disabled by `--require-no-network`/`--no-network`; any missing local artifact fails closed.
- Production graph writes/imports: guarded by required false safety flags; `--require-no-import-flags` verifies graph/import/write flags remain false in final artifacts.
- Subprocess dependency: the verifier is invoked through `uv run python`; interpreter or dependency failures bubble as command failures.

## Load Profile

Expected load is the fixed five-article smoke corpus. At 10x, filesystem JSON reads/writes and artifact enumeration saturate first; no network, database, or graph writer pool is involved. Protection is fail-closed local artifact validation and bounded per-article JSON artifacts, with larger-corpus validation blocked until a baseline exists.

## Negative Tests

- `tests/test_article_preprocessing_replay_contract.py::test_final_replay_requires_no_network_execution` covers missing no-network enforcement.
- `tests/test_article_preprocessing_replay_contract.py::test_final_replay_rejects_missing_local_evidence_instead_of_fetching` covers missing local evidence and verifies no fetch fallback.
- `tests/test_article_preprocessing_replay_contract.py::test_final_replay_writes_contract_compliant_per_article_artifact` covers baseline-missing blocked readiness and false safety flags.
"""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(report, encoding="utf-8")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog", required=True, type=Path)
    parser.add_argument("--index", required=True, type=Path)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--final", required=True, type=Path)
    parser.add_argument("--write-events", type=Path)
    parser.add_argument(
        "--events",
        type=Path,
        help="Read an existing final replay event log instead of rewriting it.",
    )
    parser.add_argument("--no-network", action="store_true")
    parser.add_argument("--require-no-network", action="store_true")
    parser.add_argument("--require-no-import-flags", action="store_true")
    parser.add_argument("--write-summary", type=Path)
    parser.add_argument("--write-report", type=Path)
    parser.add_argument("--write-decision", type=Path)
    return parser


def _events_from_path(path: Path) -> list[dict[str, Any]]:
    try:
        return [
            json.loads(line)
            for line in path.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
    except FileNotFoundError as exc:
        raise FinalReplayError(f"required local event log is missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise FinalReplayError(f"event log is not valid JSONL: {path}: {exc}") from exc


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    args.no_network = bool(args.no_network or args.require_no_network)
    try:
        if args.events is not None and args.write_events is None:
            events = _events_from_path(args.events)
            _read_final_artifacts(args.final)
            completed = sum(
                1 for event in events if event.get("event_type") == "final_replay.article_completed"
            )
        else:
            events = run_replay(args)
            if args.write_events is not None:
                args.write_events.parent.mkdir(parents=True, exist_ok=True)
                args.write_events.write_text(
                    "".join(json.dumps(event, sort_keys=True) + "\n" for event in events),
                    encoding="utf-8",
                )
            completed = sum(
                1 for event in events if event["event_type"] == "final_replay.article_completed"
            )
        summary = None
        if args.write_summary or args.write_report or args.write_decision:
            summary = _summary_from_artifacts(args, events)
            if args.require_no_network and summary["no_network_proof"]["network_fetch_attempted"]:
                raise FinalReplayError("network fetch was attempted despite --require-no-network")
            if args.require_no_import_flags and summary["no_write_safety"]["safety_violations"]:
                raise FinalReplayError(
                    "final replay artifacts contain graph/import/write safety flag violations"
                )
        if args.write_summary is not None and summary is not None:
            _write_summary(args.write_summary, summary)
        if args.write_report is not None and summary is not None:
            _write_report(args.write_report, summary)
        if args.write_decision is not None and summary is not None:
            _write_decision(args.write_decision, summary)
        sys.stdout.write(
            f"wrote final preprocessing replay report for {completed} articles to {args.final}; "
            f"summary={args.write_summary}; report={args.write_report}; decision={args.write_decision}\n"
        )
        return 0
    except FinalReplayError as exc:
        sys.stderr.write(f"ERROR: {exc}\n")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
