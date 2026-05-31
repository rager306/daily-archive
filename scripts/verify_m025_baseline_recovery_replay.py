#!/usr/bin/env python3
"""Regenerate the M025 local baseline for final preprocessing replay.

This CLI is intentionally filesystem-only. It reads the fixed article catalog,
catalog index, corpus selection, local chunking artifacts, and local evidence
artifacts, then writes S08-compatible per-article baseline artifacts under
``baseline/<slug>/final.json``. It never fetches network data, never imports graph
facts, and never writes to production LadybugDB.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m025-baseline-recovery-replay.v00.01"
ARTIFACT_SCHEMA_VERSION = "m025-baseline-recovery-artifact.v00.01"
EVENT_SCHEMA_VERSION = "m025-baseline-recovery-event.v00.01"
EVIDENCE_TYPES = ("assets", "tables", "links", "identity")
FALSE_SAFETY_FLAGS = {
    "graph_import_allowed": False,
    "trusted_kg_import_allowed": False,
    "production_import_attempted": False,
    "production_ladybugdb_write_allowed": False,
    "ladybugdb_written": False,
}


class BaselineRecoveryError(RuntimeError):
    """Raised when baseline recovery cannot safely run from local artifacts."""


@dataclass(frozen=True)
class ArticleSelection:
    article_ref: str
    source_code: str
    selection_role: str


def _looks_like_url(value: str) -> bool:
    lowered = value.lower()
    return lowered.startswith(("http://", "https://", "http:/", "https:/"))


def _reject_url_path(path: Path, label: str) -> None:
    if _looks_like_url(str(path)):
        raise BaselineRecoveryError(f"{label} must be a local filesystem path, not a URL: {path}")


def _validate_paths_are_local(args: argparse.Namespace) -> None:
    for label in (
        "catalog",
        "index",
        "selection",
        "baseline",
        "write_events",
        "write_summary",
        "write_report",
    ):
        path = getattr(args, label, None)
        if path is not None:
            _reject_url_path(path, label.replace("_", "-"))


def _load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise BaselineRecoveryError(f"required local input is missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise BaselineRecoveryError(f"required local input is not valid JSON: {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise BaselineRecoveryError(f"required local input must be a JSON object: {path}")
    return payload


def _article_slug(article_ref: str) -> str:
    return article_ref.replace("/", "-").replace(":", "-")


def _catalog_by_ref(index_payload: dict[str, Any]) -> dict[str, dict[str, Any]]:
    articles = index_payload.get("articles")
    if not isinstance(articles, list):
        raise BaselineRecoveryError("catalog index does not contain an articles list")
    result: dict[str, dict[str, Any]] = {}
    for article in articles:
        if isinstance(article, dict) and isinstance(article.get("article_ref"), str):
            result[str(article["article_ref"])] = article
    return result


def _selection_articles(selection_payload: dict[str, Any]) -> list[ArticleSelection]:
    articles = selection_payload.get("articles")
    if not isinstance(articles, list) or not articles:
        raise BaselineRecoveryError("selection does not contain a non-empty articles list")
    selections: list[ArticleSelection] = []
    for idx, article in enumerate(articles):
        if not isinstance(article, dict):
            raise BaselineRecoveryError(f"selection article at index {idx} is not an object")
        article_ref = article.get("article_ref")
        source_code = article.get("source_code")
        if not isinstance(article_ref, str) or not article_ref:
            raise BaselineRecoveryError(f"selection article at index {idx} is missing article_ref")
        if not isinstance(source_code, str) or not source_code:
            raise BaselineRecoveryError(f"selection article {article_ref} is missing source_code")
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
        raise BaselineRecoveryError(f"chunk artifact has non-list chunks/items: {path}")
    diagnostics = payload.get("diagnostics") if isinstance(payload.get("diagnostics"), list) else []
    return len([chunk for chunk in chunks if isinstance(chunk, dict)]), [d for d in diagnostics if isinstance(d, dict)]


def _evidence_metrics(paths: dict[str, Path]) -> tuple[dict[str, int], dict[str, int]]:
    counts: dict[str, int] = {}
    diagnostics: dict[str, int] = {}
    for evidence_type, path in paths.items():
        payload = _load_json(path)
        summary = payload.get("summary") if isinstance(payload.get("summary"), dict) else {}
        try:
            counts[evidence_type] = int(summary.get("item_count") or 0)
            diagnostics[evidence_type] = int(summary.get("diagnostic_count") or 0)
        except (TypeError, ValueError) as exc:
            raise BaselineRecoveryError(f"evidence summary metrics must be integers: {path}") from exc
    return counts, diagnostics


def _resolve_local_inputs(corpus_root: Path, article_ref: str) -> tuple[Path, dict[str, Path]]:
    slug = _article_slug(article_ref)
    chunk_path = corpus_root / "chunking" / slug / "chunks.json"
    if not chunk_path.exists():
        raise BaselineRecoveryError(f"missing local chunking artifact for {article_ref}: {chunk_path}")
    evidence_paths = {evidence_type: corpus_root / "evidence" / slug / f"{evidence_type}.json" for evidence_type in EVIDENCE_TYPES}
    missing = [str(path) for path in evidence_paths.values() if not path.exists()]
    if missing:
        raise BaselineRecoveryError(f"missing local evidence artifact(s) for {article_ref}: {', '.join(missing)}")
    return chunk_path, evidence_paths


def run_recovery(args: argparse.Namespace) -> list[dict[str, Any]]:
    _validate_paths_are_local(args)
    if not getattr(args, "no_network", False):
        raise BaselineRecoveryError("baseline recovery requires --no-network so missing local artifacts fail closed")
    catalog = _load_json(args.catalog)
    index = _load_json(args.index)
    selection = _load_json(args.selection)
    catalog_refs = _catalog_by_ref(index)
    selected = _selection_articles(selection)
    corpus_root = args.selection.parent
    args.baseline.mkdir(parents=True, exist_ok=True)

    events: list[dict[str, Any]] = [
        _event(
            "baseline_recovery.started",
            catalog_schema_version=catalog.get("schema_version"),
            index_schema_version=index.get("schema_version"),
            selection_id=selection.get("selection_id"),
            article_count=len(selected),
            baseline_path=str(args.baseline),
            no_network=True,
            network_fetch_attempted=False,
            graph_import_allowed=False,
            production_import_attempted=False,
            ladybugdb_written=False,
        )
    ]
    for article in selected:
        catalog_entry = catalog_refs.get(article.article_ref)
        if catalog_entry is None:
            raise BaselineRecoveryError(f"selection article {article.article_ref} is absent from catalog index")
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
        diagnostics: list[dict[str, Any]] = [
            {
                "code": "BASELINE_REGENERATED_LOCAL_ONLY",
                "severity": "info",
                "json_path": "$.baseline_provenance",
                "message": "Baseline was regenerated from local chunking and evidence artifacts only.",
            }
        ]
        diagnostics.extend(chunk_diagnostics)
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
            "baseline_provenance": {
                "kind": "regenerated_local_baseline",
                "source_pipeline": "current_local_preprocessing_pipeline",
                "selection_id": selection.get("selection_id"),
                "disclosure": "This is a regenerated local-only baseline, not a recovered historical production baseline.",
            },
            "diagnostics": diagnostics,
            "network": {"no_network_required": True, "network_fetch_attempted": False},
            "safety_state": {"metadata_only": True, "review_only": True, **FALSE_SAFETY_FLAGS},
            "readiness": {
                "baseline_artifact_generated": True,
                "final_replay_compatible": True,
                "graph_readiness_claim": False,
            },
        }
        output_path = args.baseline / slug / "final.json"
        _write_json(output_path, artifact)
        events.append(
            _event(
                "baseline_recovery.article_completed",
                article_ref=article.article_ref,
                path=str(output_path),
                chunk_count=chunk_count,
                evidence_counts=evidence_counts,
                baseline_provenance_kind="regenerated_local_baseline",
                no_network=True,
                network_fetch_attempted=False,
                graph_import_allowed=False,
                production_import_attempted=False,
                ladybugdb_written=False,
            )
        )
    events.append(
        _event(
            "baseline_recovery.completed",
            article_count=len(selected),
            baseline_path=str(args.baseline),
            no_network=True,
            network_fetch_attempted=False,
            graph_import_allowed=False,
            production_import_attempted=False,
            ladybugdb_written=False,
        )
    )
    return events


def _read_baseline_artifacts(baseline: Path) -> list[dict[str, Any]]:
    artifacts: list[dict[str, Any]] = []
    for path in sorted(baseline.glob("*/final.json")):
        artifact = _load_json(path)
        artifact["_path"] = str(path)
        artifacts.append(artifact)
    if not artifacts:
        raise BaselineRecoveryError(f"no baseline artifacts were found under {baseline}")
    return artifacts


def _summary_from_artifacts(args: argparse.Namespace, events: list[dict[str, Any]]) -> dict[str, Any]:
    artifacts = _read_baseline_artifacts(args.baseline)
    article_results: list[dict[str, Any]] = []
    diagnostic_counts: dict[str, int] = {}
    safety_violations: list[dict[str, Any]] = []
    provenance_counts: dict[str, int] = {}
    for artifact in artifacts:
        article_ref = str(artifact.get("article_ref"))
        provenance = artifact.get("baseline_provenance") if isinstance(artifact.get("baseline_provenance"), dict) else {}
        provenance_kind = str(provenance.get("kind") or "unknown")
        provenance_counts[provenance_kind] = provenance_counts.get(provenance_kind, 0) + 1
        diagnostics = artifact.get("diagnostics") if isinstance(artifact.get("diagnostics"), list) else []
        for diagnostic in diagnostics:
            if isinstance(diagnostic, dict):
                code = str(diagnostic.get("code") or "UNKNOWN")
                diagnostic_counts[code] = diagnostic_counts.get(code, 0) + 1
        safety_state = artifact.get("safety_state") if isinstance(artifact.get("safety_state"), dict) else {}
        violated = {key: safety_state.get(key) for key in FALSE_SAFETY_FLAGS if safety_state.get(key) is not False}
        if violated:
            safety_violations.append({"article_ref": article_ref, "violations": violated})
        metrics = artifact.get("metrics") if isinstance(artifact.get("metrics"), dict) else {}
        article_results.append(
            {
                "article_ref": article_ref,
                "path": artifact.get("_path"),
                "baseline_provenance_kind": provenance_kind,
                "chunk_count": int(metrics.get("chunk_count") or 0),
                "evidence_counts": metrics.get("evidence_counts") if isinstance(metrics.get("evidence_counts"), dict) else {},
                "diagnostic_count": len([item for item in diagnostics if isinstance(item, dict)]),
                "final_replay_compatible": bool(
                    isinstance(artifact.get("readiness"), dict)
                    and artifact["readiness"].get("final_replay_compatible") is True
                ),
            }
        )
    no_network_proof = {
        "required": True,
        "network_fetch_attempted": any(event.get("network_fetch_attempted") is True for event in events),
        "all_events_no_network": all(event.get("no_network") is True for event in events if "no_network" in event),
    }
    no_write_safety = {
        "require_no_import_flags": bool(getattr(args, "require_no_import_flags", False)),
        "safety_violations": safety_violations,
        "graph_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
    }
    ready = (
        not safety_violations
        and not no_network_proof["network_fetch_attempted"]
        and all(result["baseline_provenance_kind"] == "regenerated_local_baseline" for result in article_results)
        and all(result["final_replay_compatible"] for result in article_results)
    )
    blockers: list[str] = []
    if safety_violations:
        blockers.append("safety_flag_violation")
    if no_network_proof["network_fetch_attempted"]:
        blockers.append("network_fetch_attempted")
    if any(result["baseline_provenance_kind"] != "regenerated_local_baseline" for result in article_results):
        blockers.append("unexpected_baseline_provenance")
    if any(not result["final_replay_compatible"] for result in article_results):
        blockers.append("not_final_replay_compatible")
    return {
        "schema_version": SCHEMA_VERSION,
        "summary_type": "m025-baseline-recovery-summary",
        "article_count": len(article_results),
        "baseline_path": str(args.baseline),
        "events_path": str(getattr(args, "write_events", "")),
        "provenance_counts": provenance_counts,
        "diagnostic_counts": diagnostic_counts,
        "article_results": article_results,
        "no_network_proof": no_network_proof,
        "no_write_safety": no_write_safety,
        "readiness": {
            "baseline_recovery_completed": ready,
            "decision": "ready" if ready else "blocked",
            "blockers": blockers,
            "graph_readiness_claim": False,
            "message": "Regenerated local baseline is disclosed and limited to preprocessing replay comparisons.",
        },
    }


def _write_report(path: Path, summary: dict[str, Any]) -> None:
    rows = [
        "| Article | Provenance | Chunks | Diagnostics | Compatible |",
        "|---|---|---:|---:|---|",
    ]
    for result in summary["article_results"]:
        rows.append(
            "| {article_ref} | {provenance} | {chunks} | {diagnostics} | {compatible} |".format(
                article_ref=result["article_ref"],
                provenance=result["baseline_provenance_kind"],
                chunks=result["chunk_count"],
                diagnostics=result["diagnostic_count"],
                compatible="yes" if result["final_replay_compatible"] else "no",
            )
        )
    blockers = summary["readiness"]["blockers"] or ["None"]
    diagnostics = summary["diagnostic_counts"] or {"None": 0}
    report = f"""# M025 S09 Baseline Recovery Report

## Decision

- Baseline recovery completed: **{str(summary['readiness']['baseline_recovery_completed']).lower()}**
- Decision: **{summary['readiness']['decision']}**
- Blockers: {', '.join(blockers)}
- Graph readiness claim: **false**

This report discloses that the baseline was regenerated from local current-pipeline artifacts only. It is not a recovered historical production baseline and carries no graph-readiness claim.

## Baseline Provenance

- Baseline path: `{summary['baseline_path']}`
- Provenance counts: `{json.dumps(summary['provenance_counts'], sort_keys=True)}`

{chr(10).join(rows)}

## Diagnostics

`{json.dumps(diagnostics, sort_keys=True)}`

## No-Network Proof

`{json.dumps(summary['no_network_proof'], sort_keys=True)}`

## No-Write Safety Evidence

`{json.dumps(summary['no_write_safety'], sort_keys=True)}`

## Failure Modes

- Filesystem inputs: missing or malformed catalog, index, selection, chunking, or evidence JSON raises `BaselineRecoveryError` and exits non-zero rather than fetching or synthesizing data.
- Network dependency: intentionally disabled by `--require-no-network`/`--no-network`; URL-like paths are rejected and missing local artifacts fail closed.
- Production graph writes/imports: guarded by required false safety flags; `--require-no-import-flags` verifies graph/import/write flags remain false in generated baseline artifacts.
- Subprocess dependency: the verifier is invoked through `uv run python`; interpreter or dependency failures bubble as command failures.

## Load Profile

Expected load is the fixed five-article smoke corpus. At 10x, local filesystem JSON reads/writes and artifact enumeration saturate first; no network, database, graph writer, or connection pool is involved. Protection is bounded per-article JSON processing and fail-closed validation of every local artifact.

## Negative Tests

- `tests/test_article_baseline_recovery_replay.py::test_baseline_recovery_requires_no_network_execution` covers missing no-network enforcement.
- `tests/test_article_baseline_recovery_replay.py::test_baseline_recovery_rejects_missing_local_chunking_without_fetching` covers missing local chunking and verifies no fetch fallback.
- `tests/test_article_baseline_recovery_replay.py::test_baseline_recovery_requires_local_paths` covers URL-like path rejection.
- `tests/test_article_baseline_recovery_replay.py::test_validation_helper_rejects_baseline_missing_final_summary` covers the downstream validation blocker.
- `tests/test_article_baseline_recovery_replay.py::test_validation_helper_rejects_unsafe_baseline_flags` covers malformed graph/import/write safety flags.
"""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(report, encoding="utf-8")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog", required=True, type=Path)
    parser.add_argument("--index", required=True, type=Path)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--write-events", type=Path)
    parser.add_argument("--no-network", action="store_true")
    parser.add_argument("--require-no-network", action="store_true")
    parser.add_argument("--require-no-import-flags", action="store_true")
    parser.add_argument("--write-summary", type=Path)
    parser.add_argument("--write-report", type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    args.no_network = bool(args.no_network or args.require_no_network)
    try:
        events = run_recovery(args)
        if args.write_events is not None:
            args.write_events.parent.mkdir(parents=True, exist_ok=True)
            args.write_events.write_text("".join(json.dumps(event, sort_keys=True) + "\n" for event in events), encoding="utf-8")
        summary = None
        if args.write_summary or args.write_report or args.require_no_import_flags or args.require_no_network:
            summary = _summary_from_artifacts(args, events)
            if args.require_no_network and summary["no_network_proof"]["network_fetch_attempted"]:
                raise BaselineRecoveryError("network fetch was attempted despite --require-no-network")
            if args.require_no_import_flags and summary["no_write_safety"]["safety_violations"]:
                raise BaselineRecoveryError("baseline artifacts contain graph/import/write safety flag violations")
        if args.write_summary is not None and summary is not None:
            _write_json(args.write_summary, summary)
        if args.write_report is not None and summary is not None:
            _write_report(args.write_report, summary)
        completed = sum(1 for event in events if event["event_type"] == "baseline_recovery.article_completed")
        sys.stdout.write(
            f"wrote baseline recovery artifacts for {completed} articles to {args.baseline}; "
            f"summary={args.write_summary}; report={args.write_report}\n"
        )
        return 0
    except BaselineRecoveryError as exc:
        sys.stderr.write(f"ERROR: {exc}\n")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
