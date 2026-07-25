"""R024 NetworkX probe wrapper I/O helpers.

This module keeps filesystem and legacy artifact-shape concerns out of the
application graph probe use case while allowing scripts to remain thin wrappers.
"""

from __future__ import annotations

import json
from collections.abc import Sequence
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from research_graph.application.graph.probe import (
    GraphProbeArticleEvidence,
    GraphProbeArtifactRef,
    GraphProbeExcludedRecord,
    GraphProbeRequest,
    GraphProbeResult,
)

R024_ENTITY_TYPES = (
    "metadata",
    "table_context",
    "figure_caption_context",
    "citation_context",
    "retrieval_context",
)


@dataclass(frozen=True)
class R024NetworkXProbeConfig:
    """Filesystem configuration for one R024 NetworkX probe wrapper."""

    corpus_id: str
    corpus_dir: Path
    parser_events_path: Path
    probe_dir: Path
    graphml_path: Path
    summary_path: Path
    events_path: Path
    summary_schema_version: str
    selection_path: Path | None = None
    memory_profile_path: Path | None = None
    memory_schema_version: str | None = None
    entity_types: Sequence[str] = R024_ENTITY_TYPES
    include_citation_relations: bool = True
    repo_root: Path | None = None


def build_request(config: R024NetworkXProbeConfig) -> GraphProbeRequest:
    """Load parser/selection artifacts and build an application probe request."""

    completed_events, skipped_events = _load_parser_events(config.parser_events_path)
    completed_by_ref = {str(event["article_ref"]): event for event in completed_events}
    if config.selection_path is not None:
        completed_articles = _articles_from_selection(config.selection_path, completed_by_ref)
        excluded_records: tuple[GraphProbeExcludedRecord, ...] = ()
    else:
        completed_articles = tuple(_article_from_event(event) for event in completed_events)
        excluded_records = tuple(_excluded_from_event(event) for event in skipped_events)

    return GraphProbeRequest(
        corpus_id=config.corpus_id,
        completed_articles=completed_articles,
        excluded_records=excluded_records,
        input_artifacts=(
            GraphProbeArtifactRef(
                path=_display_path(config.parser_events_path, repo_root=_repo_root(config)),
                artifact_type="parser-events",
            ),
        ),
        entity_types=tuple(config.entity_types),
    )


def write_legacy_artifacts(
    result: GraphProbeResult,
    config: R024NetworkXProbeConfig,
    *,
    generated_at: str | None = None,
) -> None:
    """Write R024-style summary, memory profile, and events artifacts."""

    timestamp = generated_at or datetime.now(UTC).isoformat()
    config.probe_dir.mkdir(parents=True, exist_ok=True)
    config.summary_path.write_text(
        json.dumps(_summary_payload(result, config, generated_at=timestamp), indent=2),
        encoding="utf-8",
    )
    if config.memory_profile_path is not None and result.memory_profile is not None:
        config.memory_profile_path.write_text(
            json.dumps(_memory_payload(result, config, generated_at=timestamp), indent=2),
            encoding="utf-8",
        )
    with config.events_path.open("w", encoding="utf-8") as handle:
        for event in _events_payload(result, generated_at=timestamp):
            handle.write(json.dumps(event) + "\n")


def _summary_payload(
    result: GraphProbeResult,
    config: R024NetworkXProbeConfig,
    *,
    generated_at: str,
) -> dict[str, Any]:
    metrics = result.metrics
    payload: dict[str, Any] = {
        "schema_version": config.summary_schema_version,
        "generated_at": generated_at,
        "total_catalog_records_seen": result.total_catalog_records_seen,
        "corpus_size": result.corpus_size,
        "skipped_metadata_only": result.skipped_metadata_only,
        "excluded_records": [
            {
                "article_ref": record.article_ref,
                "article_key": record.article_key,
                "skip_reason": record.skip_reason,
            }
            for record in result.excluded_records
        ],
        "chunk_count_total": result.chunk_count_total,
        "source_kind_counts": result.source_kind_counts,
        "n_nodes": metrics.n_nodes if metrics is not None else 0,
        "n_edges": metrics.n_edges if metrics is not None else 0,
        "node_types": dict(metrics.node_types) if metrics is not None else {},
        "edge_types": dict(metrics.edge_types) if metrics is not None else {},
        "citation_relations_count": (
            metrics.citation_relations_count if metrics is not None else 0
        ),
        "entity_types": list(config.entity_types),
        "fail_closed_invariants": result.fail_closed_invariants,
        "implementation": result.implementation,
        "input_artifacts": [_artifact_payload(artifact) for artifact in result.input_artifacts],
        "output_artifacts": [_artifact_payload(artifact) for artifact in result.output_artifacts],
    }
    if result.diagnostics:
        payload["diagnostics"] = [
            {
                "code": diagnostic.code,
                "phase": diagnostic.phase,
                "severity": diagnostic.severity,
                "count": diagnostic.count,
                "artifact_ref": diagnostic.artifact_ref,
                "exception_class": diagnostic.exception_class,
                "notes": diagnostic.notes,
            }
            for diagnostic in result.diagnostics
        ]
        payload["failure_phase"] = result.failure_phase
        payload["first_failure_code"] = result.first_failure_code
    return payload


def _memory_payload(
    result: GraphProbeResult,
    config: R024NetworkXProbeConfig,
    *,
    generated_at: str,
) -> dict[str, Any]:
    profile = result.memory_profile
    if profile is None:
        return {}
    return {
        "schema_version": config.memory_schema_version,
        "generated_at": generated_at,
        "tracemalloc_current_bytes": profile.tracemalloc_current_bytes or 0,
        "tracemalloc_peak_bytes": profile.tracemalloc_peak_bytes or 0,
        "peak_mb": profile.peak_mb,
        "current_mb": profile.current_mb or 0,
        "n_nodes": profile.n_nodes or (result.metrics.n_nodes if result.metrics else 0),
        "n_edges": profile.n_edges or (result.metrics.n_edges if result.metrics else 0),
        "approx_bytes_per_node": profile.approx_bytes_per_node or 0,
        "top_5_allocations": list(profile.top_allocations),
        "method": profile.method,
    }


def _events_payload(result: GraphProbeResult, *, generated_at: str) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    for article in result.completed_articles:
        events.append(
            {
                "event": "article_added",
                "timestamp": generated_at,
                "article_ref": article.article_ref,
                "article_key": article.article_key,
                "chunks_added": article.chunk_count,
                "entities_added": len(_entity_types_from_result(result)),
                "network_fetch_attempted": False,
                "production_import_attempted": False,
                "graph_import_allowed": False,
                "ladybugdb_written": False,
            }
        )
    for record in result.excluded_records:
        events.append(
            {
                "event": "metadata_only_excluded",
                "timestamp": generated_at,
                "article_ref": record.article_ref,
                "article_key": record.article_key,
                "skip_reason": record.skip_reason,
                "network_fetch_attempted": False,
                "production_import_attempted": False,
                "graph_import_allowed": False,
                "ladybugdb_written": False,
            }
        )
    return events


def _entity_types_from_result(result: GraphProbeResult) -> tuple[str, ...]:
    metrics = result.metrics
    if metrics is None:
        return ()
    entity_count = int(metrics.node_types.get("entity", 0))
    if result.corpus_size <= 0 or entity_count <= 0:
        return ()
    per_article = entity_count // result.corpus_size
    return tuple("entity" for _ in range(per_article))


def _load_parser_events(path: Path) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    completed: list[dict[str, Any]] = []
    skipped: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        event = json.loads(line)
        if event.get("event") == "parser_chunking_complete":
            completed.append(event)
        elif event.get("event") == "parser_chunking_skipped_metadata_only":
            skipped.append(event)
    return completed, skipped


def _articles_from_selection(
    selection_path: Path,
    completed_by_ref: dict[str, dict[str, Any]],
) -> tuple[GraphProbeArticleEvidence, ...]:
    selection = json.loads(selection_path.read_text(encoding="utf-8"))
    articles = selection.get("articles", []) if isinstance(selection, dict) else []
    evidence: list[GraphProbeArticleEvidence] = []
    for article in articles:
        if not isinstance(article, dict):
            continue
        ref = str(article["article_ref"])
        event = completed_by_ref.get(ref, {})
        evidence.append(
            GraphProbeArticleEvidence(
                article_ref=ref,
                article_key=str(article.get("article_key", event.get("article_key", ref))),
                chunk_count=int(event.get("chunk_count", 0)),
                source_kind=str(article.get("source_kind", event.get("source_kind", "unknown"))),
                text_source=str(event.get("text_source", "")),
            )
        )
    return tuple(evidence)


def _article_from_event(event: dict[str, Any]) -> GraphProbeArticleEvidence:
    return GraphProbeArticleEvidence(
        article_ref=str(event["article_ref"]),
        article_key=str(event.get("article_key", event["article_ref"])),
        chunk_count=int(event.get("chunk_count", 0)),
        source_kind=str(event.get("source_kind", "unknown")),
        text_source=str(event.get("text_source", "")),
    )


def _excluded_from_event(event: dict[str, Any]) -> GraphProbeExcludedRecord:
    return GraphProbeExcludedRecord(
        article_ref=str(event["article_ref"]),
        article_key=str(event.get("article_key", event["article_ref"])),
        skip_reason=str(event.get("skip_reason", "metadata_only_no_local_source_artifact")),
    )


def _artifact_payload(artifact: GraphProbeArtifactRef) -> dict[str, str | None]:
    return {
        "path": artifact.path,
        "artifact_type": artifact.artifact_type,
        "schema_version": artifact.schema_version,
    }


def _repo_root(config: R024NetworkXProbeConfig) -> Path:
    if config.repo_root is not None:
        return config.repo_root
    if config.corpus_dir.parent.name == "data":
        return config.corpus_dir.parent.parent
    return config.corpus_dir.parent


def _display_path(path: Path, *, repo_root: Path) -> str:
    try:
        return path.relative_to(repo_root).as_posix()
    except ValueError:
        return path.as_posix()
