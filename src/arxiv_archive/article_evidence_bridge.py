"""Versioned article evidence bridge contract.

This module bridges S01 article-loader outcomes into a redacted, deterministic
per-article evidence bundle for later PageIndex, asset, link, retrieval,
staging, and metrics work.  It consumes ``ArticleLoadResult`` metadata only and
never serializes article text, binary payloads, embeddings, vectors, API keys,
secrets, model output, or graph-write/readiness claims.
"""

from __future__ import annotations

import hashlib
import json
from collections.abc import Iterable
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Literal

from arxiv_archive.article_loader import ArticleLoadResult

ARTICLE_EVIDENCE_BUNDLE_SCHEMA_VERSION = "m024-article-evidence-bundle.v1"
ARTICLE_EVIDENCE_RUN_SCHEMA_VERSION = "m024-article-evidence-run.v1"
ARTICLE_EVIDENCE_DIAGNOSTICS_SCHEMA_VERSION = "m024-article-evidence-diagnostics.v1"

BundleSubtreeStatus = Literal["absent", "metadata_only", "review_only", "blocked", "not_attempted"]
LoadOutcome = Literal["loaded", "loaded_metadata_only", "failed"]
DiagnosticSeverity = Literal["info", "warning", "repair_required", "error"]

ALLOWED_LOAD_OUTCOMES = frozenset({"loaded", "loaded_metadata_only", "failed"})
ALLOWED_REPLAY_EVENTS = frozenset({"source.load_started", "source.load_completed", "source.load_failed"})
TERMINAL_REPLAY_EVENTS = frozenset({"source.load_completed", "source.load_failed"})
ALLOWED_SUBTREE_STATUSES = frozenset({"absent", "metadata_only", "review_only", "blocked", "not_attempted"})
ALLOWED_USES = ("source_provenance_review", "bridge_validation", "downstream_scaffolding")
EXCLUDED_USES = (
    "trusted_kg_import",
    "production_ladybugdb_write",
    "embedding_generation",
    "vector_indexing",
    "source_of_truth_claim",
)

FORBIDDEN_PAYLOAD_KEYS = frozenset(
    {
        "text",
        "raw_text",
        "chunk_text",
        "paper_text",
        "claim_text",
        "section_text",
        "caption_text",
        "table_text",
        "equation_text",
        "model_output",
        "raw_model_output",
        "raw_minimax_response",
        "base64",
        "binary",
        "bytes",
        "image_bytes",
        "payload",
        "embedding",
        "embeddings",
        "vector",
        "vectors",
        "secret",
        "secrets",
        "token",
        "tokens",
        "api_key",
        "credentials",
        "optimizer_trace",
        "optimizer_traces",
    }
)
FORBIDDEN_SOURCE_OF_TRUTH_KEYS = frozenset(
    {"source_of_truth", "source_of_truth_claim", "truth_source", "canonical_source", "minimax_source_of_truth"}
)


def default_safety_flags() -> dict[str, bool]:
    """Return the required fail-closed bridge safety flags."""
    return {
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "base64_embedded": False,
        "embedding_generation_attempted": False,
        "vector_indexing_attempted": False,
        "model_output_embedded": False,
        "credential_material_embedded": False,
    }


class ArticleEvidenceReplayError(ValueError):
    """Raised when S01 metadata-only events cannot be replayed safely."""

    def __init__(self, diagnostics: list[dict[str, Any]]) -> None:
        self.diagnostics = diagnostics
        codes = ", ".join(str(diagnostic.get("code")) for diagnostic in diagnostics[:5])
        super().__init__(f"Article evidence replay rejected by diagnostics: {codes}")


@dataclass(frozen=True)
class ArticleEvidenceDiagnostic:
    """One stable, redacted diagnostic for bundle validation."""

    code: str
    json_path: str
    severity: DiagnosticSeverity = "repair_required"
    object_id: str | None = None
    message: str = "Article evidence bridge diagnostic; inspect code and JSON path, not source content."
    blocks_import: bool = True

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "json_path": self.json_path,
            "severity": self.severity,
            "object_id": self.object_id,
            "message": self.message,
            "blocks_import": self.blocks_import,
        }


@dataclass(frozen=True)
class ArticleEvidenceSourceReference:
    """Deterministic metadata-only source reference from ``ArticleLoadResult``."""

    source_id: str
    paper_id: str
    source_path: str
    source_type: str
    media_type: str
    sha256: str | None
    byte_size: int
    parser_name: str
    loader_name: str
    load_outcome: str
    failure_reason: str | None
    warning_count: int
    duration_ms: int

    @classmethod
    def from_load_result(cls, result: ArticleLoadResult, *, paper_id: str) -> ArticleEvidenceSourceReference:
        return cls(
            source_id=result.source_id,
            paper_id=result.paper_id or paper_id,
            source_path=str(result.source_path),
            source_type=result.source_type,
            media_type=result.media_type,
            sha256=result.sha256,
            byte_size=result.byte_size,
            parser_name=result.parser_name,
            loader_name=result.loader_name,
            load_outcome=result.outcome,
            failure_reason=result.failure_reason,
            warning_count=result.warning_count,
            duration_ms=result.duration_ms,
        )

    @classmethod
    def from_load_event(cls, event: dict[str, Any], *, paper_id: str) -> ArticleEvidenceSourceReference:
        return cls(
            source_id=str(event["source_id"]),
            paper_id=str(event.get("paper_id") or paper_id),
            source_path=str(event["source_path"]),
            source_type=str(event["source_type"]),
            media_type=str(event["media_type"]),
            sha256=event.get("sha256") if event.get("sha256") is not None else None,
            byte_size=int(event["byte_size"]),
            parser_name=str(event["parser_name"]),
            loader_name=str(event["loader_name"]),
            load_outcome=str(event["outcome"]),
            failure_reason=event.get("failure_reason") if event.get("failure_reason") is not None else None,
            warning_count=int(event.get("warning_count", 0) or 0),
            duration_ms=int(event.get("duration_ms", 0) or 0),
        )

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "source_id": self.source_id,
            "paper_id": self.paper_id,
            "source_path": self.source_path,
            "source_type": self.source_type,
            "media_type": self.media_type,
            "sha256": self.sha256,
            "byte_size": self.byte_size,
            "parser_name": self.parser_name,
            "loader_name": self.loader_name,
            "load_outcome": self.load_outcome,
            "failure_reason": self.failure_reason,
            "warning_count": self.warning_count,
            "duration_ms": self.duration_ms,
            "raw_text_embedded": False,
            "raw_binary_embedded": False,
        }


@dataclass(frozen=True)
class ArticleEvidenceBundle:
    """Per-article bridge bundle containing only redacted metadata and counts."""

    paper_id: str
    run_id: str
    bundle_id: str
    source_refs: tuple[ArticleEvidenceSourceReference, ...]
    bundle_root: str | None = None
    diagnostics: tuple[ArticleEvidenceDiagnostic, ...] = ()
    subtrees: dict[str, dict[str, Any]] = field(default_factory=dict)
    summary: dict[str, Any] = field(default_factory=dict)

    def to_redacted_dict(self) -> dict[str, Any]:
        source_refs = [source.to_redacted_dict() for source in self.source_refs]
        summary = dict(self.summary) if self.summary else summarize_source_refs(source_refs)
        subtrees = dict(self.subtrees) if self.subtrees else _default_subtrees(source_refs, summary)
        return {
            "schema_version": ARTICLE_EVIDENCE_BUNDLE_SCHEMA_VERSION,
            "diagnostics_schema_version": ARTICLE_EVIDENCE_DIAGNOSTICS_SCHEMA_VERSION,
            "bundle_id": self.bundle_id,
            "run_id": self.run_id,
            "paper_id": self.paper_id,
            "bundle_root": self.bundle_root,
            "source_refs": source_refs,
            "subtrees": subtrees,
            "summary": summary,
            "diagnostics": [diagnostic.to_redacted_dict() for diagnostic in self.diagnostics],
            "allowed_uses": list(ALLOWED_USES),
            "excluded_uses": list(EXCLUDED_USES),
            "safety_flags": default_safety_flags(),
            "import_eligible_count": 0,
            "promoted_to_fact_count": 0,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        }


@dataclass(frozen=True)
class ArticleEvidenceRunSummary:
    """Run-level redacted summary across one or more evidence bundles."""

    run_id: str
    bundles: tuple[dict[str, Any], ...]
    output_paths: dict[str, Any] = field(default_factory=dict)
    input_source_ids: tuple[str, ...] = ()
    input_hashes: tuple[str, ...] = ()

    def to_redacted_dict(self) -> dict[str, Any]:
        summaries = [bundle.get("summary", {}) for bundle in self.bundles if isinstance(bundle.get("summary"), dict)]
        diagnostics = [diagnostic for bundle in self.bundles for diagnostic in _list_of_dicts(bundle.get("diagnostics"))]
        return {
            "schema_version": ARTICLE_EVIDENCE_RUN_SCHEMA_VERSION,
            "bundle_schema_version": ARTICLE_EVIDENCE_BUNDLE_SCHEMA_VERSION,
            "diagnostics_schema_version": ARTICLE_EVIDENCE_DIAGNOSTICS_SCHEMA_VERSION,
            "run_id": self.run_id,
            "paper_count": len({bundle.get("paper_id") for bundle in self.bundles if bundle.get("paper_id")}),
            "bundle_count": len(self.bundles),
            "source_count": sum(int(summary.get("source_count", 0) or 0) for summary in summaries),
            "outcome_counts": _merge_counts(summary.get("outcome_counts", {}) for summary in summaries),
            "failure_counts": _merge_counts(summary.get("failure_counts", {}) for summary in summaries),
            "diagnostic_count": len(diagnostics),
            "diagnostic_counts_by_code": _counts(diagnostic.get("code") for diagnostic in diagnostics),
            "input_source_ids": list(self.input_source_ids),
            "input_hashes": list(self.input_hashes),
            "import_eligible_count": 0,
            "promoted_to_fact_count": 0,
            "production_import_attempted": False,
            "ladybugdb_written": False,
            "safety_flags": default_safety_flags(),
            "output_paths": dict(self.output_paths),
        }


def build_article_evidence_bundle(
    load_results: Iterable[ArticleLoadResult],
    paper_id: str,
    run_id: str,
    bundle_root: str | Path | None = None,
) -> ArticleEvidenceBundle:
    """Build a deterministic metadata-only evidence bundle from loader results."""
    source_refs = tuple(
        sorted(
            (ArticleEvidenceSourceReference.from_load_result(result, paper_id=paper_id) for result in load_results),
            key=lambda source: (source.source_id, source.source_path, source.load_outcome),
        )
    )
    return _bundle_from_source_refs(source_refs, paper_id=paper_id, run_id=run_id, bundle_root=bundle_root)


def build_article_evidence_bundle_from_load_events(
    events: Iterable[dict[str, Any]],
    *,
    paper_id: str,
    run_id: str,
    bundle_root: str | Path | None = None,
) -> ArticleEvidenceBundle:
    """Build a metadata-only evidence bundle by replaying flattened S01 load events.

    ``source.load_started`` events are accepted for validation/replay provenance,
    but only terminal ``source.load_completed`` and ``source.load_failed`` events
    become source references so replayed bundles match the direct
    ``ArticleLoadResult`` path without rerunning acquisition or extraction.
    """
    event_list = [dict(event) for event in events]
    diagnostics = validate_article_load_events(event_list, paper_id=paper_id)
    if diagnostics:
        raise ArticleEvidenceReplayError(diagnostics)
    source_refs = tuple(
        sorted(
            (
                ArticleEvidenceSourceReference.from_load_event(event, paper_id=paper_id)
                for event in event_list
                if event.get("event") in TERMINAL_REPLAY_EVENTS
            ),
            key=lambda source: (source.source_id, source.source_path, source.load_outcome),
        )
    )
    return _bundle_from_source_refs(source_refs, paper_id=paper_id, run_id=run_id, bundle_root=bundle_root)


def validate_article_load_events(events: Iterable[dict[str, Any]], *, paper_id: str | None = None) -> list[dict[str, Any]]:
    """Validate S01 flattened load events for safe metadata-only replay."""
    diagnostics: list[ArticleEvidenceDiagnostic] = []
    terminal_source_ids: set[str] = set()
    required = (
        "event",
        "source_path",
        "source_id",
        "source_type",
        "media_type",
        "byte_size",
        "parser_name",
        "loader_name",
        "outcome",
        "duration_ms",
        "warning_count",
    )
    for index, event in enumerate(events):
        path = f"/events[{index}]"
        if not isinstance(event, dict):
            diagnostics.append(_diagnostic("malformed_replay_event", path))
            continue
        object_id = _string_or_none(event.get("source_id"))
        diagnostics.extend(_validate_forbidden_keys(event, path))
        event_name = event.get("event")
        if event_name not in ALLOWED_REPLAY_EVENTS:
            diagnostics.append(_diagnostic("unsupported_replay_event", f"{path}/event", object_id))
            continue
        diagnostics.extend(_required(event, required, path))
        diagnostics.extend(_validate_non_empty_ids(event, ("source_path", "source_id", "source_type", "media_type", "parser_name", "loader_name"), path, object_id))
        if paper_id is not None and event.get("paper_id") not in {None, paper_id}:
            diagnostics.append(_diagnostic("replay_paper_id_mismatch", f"{path}/paper_id", object_id))
        if not isinstance(event.get("byte_size"), int) or int(event.get("byte_size", -1)) < 0:
            diagnostics.append(_diagnostic("invalid_byte_size", f"{path}/byte_size", object_id))
        if not isinstance(event.get("warning_count"), int) or int(event.get("warning_count", -1)) < 0:
            diagnostics.append(_diagnostic("invalid_warning_count", f"{path}/warning_count", object_id))
        if not isinstance(event.get("duration_ms"), int) or int(event.get("duration_ms", -1)) < 0:
            diagnostics.append(_diagnostic("invalid_duration_ms", f"{path}/duration_ms", object_id))
        sha = event.get("sha256")
        if sha is not None and not _valid_sha256(sha):
            diagnostics.append(_diagnostic("invalid_sha256", f"{path}/sha256", object_id))
        if event_name == "source.load_started":
            if event.get("outcome") != "started":
                diagnostics.append(_diagnostic("invalid_started_outcome", f"{path}/outcome", object_id))
            if event.get("failure_reason") is not None:
                diagnostics.append(_diagnostic("unexpected_failure_reason", f"{path}/failure_reason", object_id))
            continue
        if event.get("outcome") not in ALLOWED_LOAD_OUTCOMES:
            diagnostics.append(_diagnostic("invalid_load_outcome", f"{path}/outcome", object_id))
        if event_name == "source.load_completed" and event.get("outcome") not in {"loaded", "loaded_metadata_only"}:
            diagnostics.append(_diagnostic("terminal_event_outcome_mismatch", f"{path}/outcome", object_id))
        if event_name == "source.load_failed" and event.get("outcome") != "failed":
            diagnostics.append(_diagnostic("terminal_event_outcome_mismatch", f"{path}/outcome", object_id))
        if event.get("outcome") == "failed" and not event.get("failure_reason"):
            diagnostics.append(_diagnostic("missing_failure_reason", f"{path}/failure_reason", object_id))
        if event.get("outcome") != "failed" and event.get("failure_reason") is not None:
            diagnostics.append(_diagnostic("unexpected_failure_reason", f"{path}/failure_reason", object_id))
        if event.get("outcome") in {"loaded", "loaded_metadata_only"} and not _valid_sha256(sha):
            diagnostics.append(_diagnostic("missing_checksum_for_loaded_source", f"{path}/sha256", object_id))
        source_id = event.get("source_id")
        if isinstance(source_id, str) and source_id:
            if source_id in terminal_source_ids:
                diagnostics.append(_diagnostic("duplicate_terminal_source_id", f"{path}/source_id", source_id))
            else:
                terminal_source_ids.add(source_id)
    return [diagnostic.to_redacted_dict() for diagnostic in diagnostics]


def replay_input_source_ids(events: Iterable[dict[str, Any]]) -> list[str]:
    """Return deterministic source IDs observed in replay input events."""
    return sorted({str(event.get("source_id")) for event in events if isinstance(event, dict) and event.get("source_id")})


def replay_input_hashes(events: Iterable[dict[str, Any]]) -> list[str]:
    """Return deterministic redacted hashes for replay input events."""
    return sorted(_replay_event_hash(event) for event in events if isinstance(event, dict))


def _bundle_from_source_refs(
    source_refs: tuple[ArticleEvidenceSourceReference, ...],
    *,
    paper_id: str,
    run_id: str,
    bundle_root: str | Path | None = None,
) -> ArticleEvidenceBundle:
    source_dicts = [source.to_redacted_dict() for source in source_refs]
    summary = summarize_source_refs(source_dicts)
    bundle_id = _bundle_id(paper_id=paper_id, run_id=run_id, source_refs=source_dicts)
    return ArticleEvidenceBundle(
        paper_id=paper_id,
        run_id=run_id,
        bundle_id=bundle_id,
        source_refs=source_refs,
        bundle_root=str(bundle_root) if bundle_root is not None else None,
        subtrees=_default_subtrees(source_dicts, summary),
        summary=summary,
    )


def summarize_source_refs(source_refs: list[dict[str, Any]]) -> dict[str, Any]:
    """Return deterministic bridge observability counts for source refs."""
    source_count = len(source_refs)
    checksum_count = sum(1 for source in source_refs if _valid_sha256(source.get("sha256")))
    failure_count = sum(1 for source in source_refs if source.get("load_outcome") == "failed")
    warning_count = sum(int(source.get("warning_count", 0) or 0) for source in source_refs)
    return {
        "source_count": source_count,
        "outcome_counts": _counts(source.get("load_outcome") for source in source_refs),
        "source_type_counts": _counts(source.get("source_type") for source in source_refs),
        "media_type_counts": _counts(source.get("media_type") for source in source_refs),
        "failure_counts": _counts(source.get("failure_reason") for source in source_refs if source.get("failure_reason")),
        "failure_count": failure_count,
        "warning_count": warning_count,
        "checksum_count": checksum_count,
        "checksum_coverage_rate": checksum_count / source_count if source_count else 0.0,
        "metadata_only_count": sum(1 for source in source_refs if source.get("load_outcome") == "loaded_metadata_only"),
        "loaded_count": sum(1 for source in source_refs if source.get("load_outcome") == "loaded"),
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "import_eligible_count": 0,
        "promoted_to_fact_count": 0,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "safety_flags": default_safety_flags(),
    }


def validate_article_evidence_bundle(bundle: ArticleEvidenceBundle | dict[str, Any]) -> list[dict[str, Any]]:
    """Return redacted diagnostics with stable codes and JSON paths."""
    payload = bundle.to_redacted_dict() if hasattr(bundle, "to_redacted_dict") else dict(bundle)
    diagnostics: list[ArticleEvidenceDiagnostic] = []
    diagnostics.extend(_required(payload, ("schema_version", "bundle_id", "run_id", "paper_id", "source_refs", "subtrees", "summary", "safety_flags"), ""))
    if payload.get("schema_version") != ARTICLE_EVIDENCE_BUNDLE_SCHEMA_VERSION:
        diagnostics.append(_diagnostic("invalid_schema_version", "/schema_version"))
    if payload.get("diagnostics_schema_version") != ARTICLE_EVIDENCE_DIAGNOSTICS_SCHEMA_VERSION:
        diagnostics.append(_diagnostic("invalid_diagnostics_schema_version", "/diagnostics_schema_version"))
    diagnostics.extend(_validate_forbidden_keys(payload))
    diagnostics.extend(_validate_source_of_truth_markers(payload))
    diagnostics.extend(_validate_safety_flags(payload.get("safety_flags"), "/safety_flags"))
    diagnostics.extend(_validate_uses(payload, ""))
    for field_name in ("import_eligible_count", "promoted_to_fact_count"):
        if payload.get(field_name) != 0:
            diagnostics.append(_diagnostic(f"{field_name}_nonzero", f"/{field_name}"))
    for field_name in ("production_import_attempted", "ladybugdb_written"):
        if payload.get(field_name) is not False:
            diagnostics.append(_diagnostic(f"{field_name}_true", f"/{field_name}"))

    source_refs = _list_of_dicts(payload.get("source_refs"))
    diagnostics.extend(_validate_duplicate_ids(source_refs, "source_id", "/source_refs", "duplicate_source_id"))
    for index, source in enumerate(source_refs):
        diagnostics.extend(_validate_source_ref(source, f"/source_refs[{index}]", payload.get("paper_id")))
    diagnostics.extend(_validate_subtrees(payload.get("subtrees"), source_refs))
    diagnostics.extend(_validate_summary(payload.get("summary"), source_refs))
    for index, diagnostic_record in enumerate(_list_of_dicts(payload.get("diagnostics"))):
        diagnostics.extend(_validate_diagnostic_record(diagnostic_record, f"/diagnostics[{index}]"))
    return [diagnostic.to_redacted_dict() for diagnostic in diagnostics]


def to_redacted_dict(value: ArticleEvidenceBundle | ArticleEvidenceRunSummary | dict[str, Any]) -> dict[str, Any]:
    """Convert a bridge object or mapping to a redacted dictionary."""
    if hasattr(value, "to_redacted_dict"):
        return value.to_redacted_dict()  # type: ignore[no-any-return]
    return dict(value)


def attach_page_index_summary(
    bundle: ArticleEvidenceBundle | dict[str, Any],
    page_index: dict[str, Any],
    *,
    manifest_path: str | Path | None = None,
    manifest_sha256: str | None = None,
) -> dict[str, Any]:
    """Return a redacted bundle dict with an updated ``subtrees["page_index"]`` summary.

    The bridge stores PageIndex observability metadata only: status, counts,
    diagnostics counters, manifest provenance, source provenance, and fail-closed
    flags. It does not copy PageIndex nodes, anchors, source spans, normalized
    text, graph-import claims, or payload-bearing diagnostics into the bridge.
    """
    payload = to_redacted_dict(bundle)
    source_refs = _list_of_dicts(payload.get("source_refs"))
    summary = page_index.get("summary") if isinstance(page_index.get("summary"), dict) else {}
    manifest_diagnostics = _list_of_dicts(page_index.get("diagnostics"))
    validation_diagnostics = _page_index_validation_diagnostics(page_index)
    bridge_diagnostics = _page_index_bridge_diagnostics(source_refs)
    diagnostics = manifest_diagnostics + validation_diagnostics + bridge_diagnostics
    diagnostic_counts = _counts(diagnostic.get("code") for diagnostic in diagnostics)
    blocker_count = sum(1 for diagnostic in diagnostics if diagnostic.get("blocks_import") is True)
    fallback_count = _int_from_mapping(summary, "fallback_count")
    status: BundleSubtreeStatus
    if blocker_count:
        status = "blocked"
    elif fallback_count:
        status = "review_only"
    else:
        status = "metadata_only"

    subtrees = dict(payload.get("subtrees") if isinstance(payload.get("subtrees"), dict) else {})
    subtrees["page_index"] = {
        "status": status,
        "review_only": True,
        "record_count": _int_from_mapping(summary, "node_count"),
        "node_count": _int_from_mapping(summary, "node_count"),
        "anchor_count": _int_from_mapping(summary, "anchor_count"),
        "missing_parent_count": _int_from_mapping(summary, "missing_parent_count"),
        "missing_span_count": _int_from_mapping(summary, "missing_span_count"),
        "fallback_count": fallback_count,
        "blocker_count": blocker_count,
        "diagnostic_count": len(diagnostics),
        "diagnostic_counts_by_code": diagnostic_counts,
        "manifest": {
            "path": str(manifest_path) if manifest_path is not None else None,
            "sha256": manifest_sha256,
            "schema_version": _string_or_none(page_index.get("schema_version")),
            "diagnostics_schema_version": _string_or_none(page_index.get("diagnostics_schema_version")),
            "builder": _string_or_none(page_index.get("builder")),
            "paper_id": _string_or_none(page_index.get("paper_id")),
        },
        "source_provenance": _page_index_source_provenance(source_refs),
        "graph_import_claim": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "import_eligible_count": 0,
        "promoted_to_fact_count": 0,
    }
    payload["subtrees"] = subtrees
    payload["import_eligible_count"] = 0
    payload["promoted_to_fact_count"] = 0
    payload["production_import_attempted"] = False
    payload["ladybugdb_written"] = False
    payload["safety_flags"] = default_safety_flags()
    return payload


def attach_assets_summary(
    bundle: ArticleEvidenceBundle | dict[str, Any],
    asset_manifest: dict[str, Any],
    manifest_path: str | Path | None = None,
    manifest_sha256: str | None = None,
) -> dict[str, Any]:
    """Return a redacted bundle dict with an updated ``subtrees["assets"]`` summary.

    The bridge stores aggregate asset observability only: manifest provenance,
    status, counts, coverage rates, source/PageIndex provenance counters, and
    fail-closed import flags. It never copies asset records, captions, table
    contents, image payloads, embeddings, vectors, or graph-readiness claims into
    the evidence bundle subtree.
    """
    payload = to_redacted_dict(bundle)
    bundle_source_refs = _list_of_dicts(payload.get("source_refs"))
    summary = asset_manifest.get("summary") if isinstance(asset_manifest.get("summary"), dict) else {}
    manifest_source_refs = _list_of_dicts(asset_manifest.get("source_refs"))
    assets = _list_of_dicts(asset_manifest.get("assets"))
    diagnostics = (
        _list_of_dicts(asset_manifest.get("diagnostics"))
        + _asset_manifest_validation_diagnostics(asset_manifest)
        + _asset_bridge_diagnostics(
            asset_manifest,
            bundle_source_refs,
            manifest_source_refs,
            assets,
            manifest_path=manifest_path,
            manifest_sha256=manifest_sha256,
        )
    )
    diagnostic_counts = _counts(diagnostic.get("code") for diagnostic in diagnostics)
    blocker_count = sum(1 for diagnostic in diagnostics if diagnostic.get("blocks_import") is True)
    manifest_status = _string_or_none((asset_manifest.get("subtree") or {}).get("status") if isinstance(asset_manifest.get("subtree"), dict) else None)
    status: BundleSubtreeStatus
    if blocker_count:
        status = "blocked"
    elif manifest_status and manifest_status.startswith("review_only"):
        status = "review_only"
    else:
        status = "metadata_only"

    record_count = _int_from_mapping(summary, "asset_count") if summary else len(assets)
    subtrees = dict(payload.get("subtrees") if isinstance(payload.get("subtrees"), dict) else {})
    subtrees["assets"] = {
        "status": status,
        "review_only": True,
        "record_count": record_count,
        "asset_count": record_count,
        "asset_counts_by_type": dict(summary.get("asset_counts_by_type", {})) if isinstance(summary.get("asset_counts_by_type"), dict) else _counts(asset.get("asset_type") for asset in assets),
        "preservation_state_counts": dict(summary.get("preservation_state_counts", {})) if isinstance(summary.get("preservation_state_counts"), dict) else _counts(asset.get("preservation_state") for asset in assets),
        "interpretation_status_counts": dict(summary.get("interpretation_status_counts", {})) if isinstance(summary.get("interpretation_status_counts"), dict) else _counts(asset.get("interpretation_status") for asset in assets),
        "source_ref_count": _int_from_mapping(summary, "source_ref_count") if summary else len(manifest_source_refs),
        "page_index_node_ref_count": _int_from_mapping(summary, "page_index_node_ref_count"),
        "page_index_anchor_ref_count": _int_from_mapping(summary, "page_index_anchor_ref_count"),
        "hash_coverage_rate": _float_from_mapping(summary, "hash_coverage_rate"),
        "page_index_anchor_coverage_rate": _float_from_mapping(summary, "page_index_anchor_coverage_rate"),
        "source_span_coverage_rate": _float_from_mapping(summary, "source_span_coverage_rate"),
        "blocker_count": blocker_count,
        "import_ineligible_count": record_count,
        "diagnostic_count": len(diagnostics),
        "diagnostic_counts_by_code": diagnostic_counts,
        "manifest": {
            "path": str(manifest_path) if manifest_path is not None else None,
            "sha256": manifest_sha256,
            "schema_version": _string_or_none(asset_manifest.get("schema_version")),
            "diagnostics_schema_version": _string_or_none(asset_manifest.get("diagnostics_schema_version")),
            "builder": _string_or_none(asset_manifest.get("builder")),
            "paper_id": _string_or_none(asset_manifest.get("paper_id")),
        },
        "source_provenance": _asset_source_provenance(bundle_source_refs, manifest_source_refs, assets),
        "page_index_provenance": _asset_page_index_provenance(assets, asset_manifest),
        "graph_import_claim": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "import_eligible_count": 0,
        "promoted_to_fact_count": 0,
    }
    payload["subtrees"] = subtrees
    payload["import_eligible_count"] = 0
    payload["promoted_to_fact_count"] = 0
    payload["production_import_attempted"] = False
    payload["ladybugdb_written"] = False
    payload["safety_flags"] = default_safety_flags()
    return payload


def attach_links_dedup_summary(
    bundle: ArticleEvidenceBundle | dict[str, Any],
    links_dedup_manifest: Any,
    *,
    manifest_path: str | Path | None = None,
    manifest_sha256: str | None = None,
) -> dict[str, Any]:
    """Return a redacted bundle dict with ``subtrees["links_dedup"]`` summary attached.

    The bridge stores constant-size link/dedup observability only: manifest
    provenance, source/PageIndex coverage counters, family/signal/decision
    counts, diagnostics, and fail-closed import counters. It never embeds link
    records, raw references, metadata values, model output, vectors, graph-write
    claims, or import authorization from the manifest.
    """
    payload = to_redacted_dict(bundle)
    bundle_source_refs = _list_of_dicts(payload.get("source_refs"))
    manifest = _links_dedup_redacted_manifest(links_dedup_manifest)
    summary = manifest.get("summary") if isinstance(manifest.get("summary"), dict) else {}
    family_counts = summary.get("link_family_counts") if isinstance(summary.get("link_family_counts"), dict) else {}
    metadata_signal_counts = summary.get("metadata_signal_counts") if isinstance(summary.get("metadata_signal_counts"), dict) else {}
    dedup_decision_counts = summary.get("dedup_decision_counts") if isinstance(summary.get("dedup_decision_counts"), dict) else {}
    diagnostic_counts = dict(summary.get("diagnostic_counts", {})) if isinstance(summary.get("diagnostic_counts"), dict) else {}
    manifest_diagnostics = _list_of_dicts(manifest.get("diagnostics"))
    validation_diagnostics = _links_dedup_validation_diagnostics(manifest)
    bridge_diagnostics = _links_dedup_bridge_diagnostics(
        manifest,
        bundle_source_refs,
        manifest_path=manifest_path,
        manifest_sha256=manifest_sha256,
        bundle_paper_id=_string_or_none(payload.get("paper_id")),
    )
    diagnostics = manifest_diagnostics + validation_diagnostics + bridge_diagnostics
    bridge_diagnostic_counts = _counts(diagnostic.get("code") for diagnostic in diagnostics)
    for key, count in bridge_diagnostic_counts.items():
        diagnostic_counts[key] = int(diagnostic_counts.get(key, 0) or 0) + count
    blocker_count = sum(1 for diagnostic in diagnostics if diagnostic.get("blocks_import") is not False)

    record_count = sum(_int_from_mapping(family_counts, key) for key in ("citation", "structural", "metadata_signal", "dedup_candidate"))
    status: BundleSubtreeStatus = "blocked" if blocker_count else "review_only"
    page_index_coverage = summary.get("page_index_anchor_coverage") if isinstance(summary.get("page_index_anchor_coverage"), dict) else {}
    source_span_coverage = summary.get("source_span_coverage") if isinstance(summary.get("source_span_coverage"), dict) else {}

    subtrees = dict(payload.get("subtrees") if isinstance(payload.get("subtrees"), dict) else {})
    subtrees["links_dedup"] = {
        "status": status,
        "review_only": True,
        "record_count": record_count,
        "citation_link_count": _int_from_mapping(family_counts, "citation"),
        "structural_link_count": _int_from_mapping(family_counts, "structural"),
        "metadata_signal_count": _int_from_mapping(family_counts, "metadata_signal"),
        "dedup_candidate_count": _int_from_mapping(family_counts, "dedup_candidate"),
        "link_family_counts": dict(family_counts),
        "page_index_anchor_coverage": dict(page_index_coverage),
        "source_span_coverage": dict(source_span_coverage),
        "metadata_signal_counts": dict(metadata_signal_counts),
        "dedup_decision_counts": dict(dedup_decision_counts),
        "diagnostic_count": len(diagnostics),
        "diagnostic_counts_by_code": dict(sorted(diagnostic_counts.items())),
        "blocker_count": blocker_count,
        "conflict_count": int(diagnostic_counts.get("conflict_count", 0) or 0),
        "insufficient_metadata_count": int(diagnostic_counts.get("insufficient_metadata_count", 0) or 0),
        "forbidden_payload_detection_count": int(diagnostic_counts.get("forbidden_payload_detection_count", 0) or 0),
        "unsafe_authorization_count": int(diagnostic_counts.get("unsafe_authorization_count", 0) or 0),
        "manifest": {
            "path": str(manifest_path) if manifest_path is not None else None,
            "sha256": manifest_sha256,
            "schema_version": _string_or_none(manifest.get("schema_version")),
            "paper_id": _string_or_none(manifest.get("paper_id")),
            "run_id": _string_or_none(manifest.get("run_id")),
        },
        "source_provenance": _links_dedup_source_provenance(bundle_source_refs, _list_of_dicts(manifest.get("source_refs"))),
        "page_index_provenance": _links_dedup_page_index_provenance(manifest),
        "graph_import_claim": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "import_eligible_count": 0,
        "promoted_to_fact_count": 0,
    }
    payload["subtrees"] = subtrees
    payload["import_eligible_count"] = 0
    payload["promoted_to_fact_count"] = 0
    payload["production_import_attempted"] = False
    payload["ladybugdb_written"] = False
    payload["safety_flags"] = default_safety_flags()
    return payload



def attach_retrieval_table_benchmark_summary(
    bundle: ArticleEvidenceBundle | dict[str, Any],
    benchmark_manifest: Any,
    *,
    manifest_path: str | Path | None = None,
    manifest_sha256: str | None = None,
) -> dict[str, Any]:
    """Attach S06 retrieval/table benchmark aggregates to an evidence bundle.

    The attachment is intentionally aggregate-only. It validates the untrusted
    manifest, converts it through the retrieval/table redactor, and copies only
    constant-size counters, status totals, manifest provenance, source/PageIndex
    /asset/link provenance counters, diagnostic counts, and fail-closed
    import/write indicators into ``subtrees["retrieval"]`` and ``subtrees["metrics"]``.
    Retrieval-unit arrays and table-candidate records never enter the bridge.
    """
    payload = to_redacted_dict(bundle)
    bundle_source_refs = _list_of_dicts(payload.get("source_refs"))
    manifest = _retrieval_table_redacted_manifest(benchmark_manifest)
    summary = manifest.get("summary") if isinstance(manifest.get("summary"), dict) else {}
    manifest_diagnostics = _list_of_dicts(manifest.get("diagnostics"))
    validation_diagnostics = _retrieval_table_validation_diagnostics(benchmark_manifest)
    bridge_diagnostics = _retrieval_table_bridge_diagnostics(
        manifest,
        bundle_source_refs,
        manifest_path=manifest_path,
        manifest_sha256=manifest_sha256,
        bundle_paper_id=_string_or_none(payload.get("paper_id")),
    )
    diagnostics = manifest_diagnostics + validation_diagnostics + bridge_diagnostics
    diagnostic_counts = _counts(diagnostic.get("code") for diagnostic in diagnostics)
    blocker_count = sum(1 for diagnostic in diagnostics if diagnostic.get("blocks_import") is not False)
    summary_diagnostic_counts = summary.get("diagnostic_counts") if isinstance(summary.get("diagnostic_counts"), dict) else {}

    retrieval_unit_count = _int_from_mapping(summary, "retrieval_unit_count")
    table_candidate_count = _int_from_mapping(summary, "table_candidate_count")
    record_count = retrieval_unit_count + table_candidate_count
    status: BundleSubtreeStatus = "blocked" if blocker_count else "review_only"

    subtrees = dict(payload.get("subtrees") if isinstance(payload.get("subtrees"), dict) else {})
    retrieval_subtree = {
        "status": status,
        "review_only": True,
        "record_count": record_count,
        "retrieval_unit_count": retrieval_unit_count,
        "table_candidate_count": table_candidate_count,
        "included_review_only_count": _int_from_mapping(summary, "included_review_only_count"),
        "blocked_count": _int_from_mapping(summary, "blocked_count"),
        "repair_required_count": _int_from_mapping(summary, "repair_required_count"),
        "ranking_tie_count": _int_from_mapping(summary, "ranking_tie_count"),
        "source_ref_count": _int_from_mapping(summary, "source_ref_count"),
        "page_index_node_ref_count": _int_from_mapping(summary, "page_index_node_ref_count"),
        "page_index_anchor_ref_count": _int_from_mapping(summary, "page_index_anchor_ref_count"),
        "asset_ref_count": _int_from_mapping(summary, "asset_ref_count"),
        "link_provenance_ref_count": _int_from_mapping(summary, "link_provenance_ref_count"),
        "manifest_provenance_count": _int_from_mapping(summary, "manifest_provenance_count"),
        "diagnostic_count": len(diagnostics),
        "diagnostic_counts_by_code": diagnostic_counts,
        "summary_diagnostic_counts": dict(sorted((str(key), int(value)) for key, value in summary_diagnostic_counts.items() if isinstance(value, int))),
        "blocker_count": blocker_count,
        "forbidden_payload_detection_count": _int_from_mapping(summary_diagnostic_counts, "forbidden_payload_detection_count") + int(diagnostic_counts.get("forbidden_payload_key", 0)),
        "unsafe_authorization_count": _int_from_mapping(summary_diagnostic_counts, "unsafe_authorization_count") + int(diagnostic_counts.get("unsafe_authorization", 0)),
        "unsafe_readiness_count": _int_from_mapping(summary_diagnostic_counts, "unsafe_readiness_count") + int(diagnostic_counts.get("unsafe_readiness", 0)),
        "manifest": {
            "path": str(manifest_path) if manifest_path is not None else _string_or_none(manifest.get("manifest_path")),
            "sha256": manifest_sha256 if manifest_sha256 is not None else _string_or_none(manifest.get("manifest_sha256")),
            "schema_version": _string_or_none(manifest.get("schema_version")),
            "manifest_schema": _string_or_none(manifest.get("manifest_schema")),
            "builder": _string_or_none(manifest.get("builder")),
            "paper_id": _string_or_none(manifest.get("paper_id")),
            "run_id": _string_or_none(manifest.get("run_id")),
        },
        "source_provenance": _retrieval_table_source_provenance(bundle_source_refs, _list_of_dicts(manifest.get("source_refs"))),
        "page_index_provenance": _retrieval_table_manifest_provenance(manifest.get("page_index_refs")),
        "asset_provenance": _retrieval_table_manifest_provenance(manifest.get("asset_refs")),
        "links_dedup_provenance": _retrieval_table_manifest_provenance(manifest.get("links_dedup_refs")),
        "graph_import_claim": False,
        "trusted_kg_import_allowed": False,
        "embedding_generation_attempted": False,
        "vector_indexing_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "import_eligible_count": 0,
        "promoted_to_fact_count": 0,
    }
    subtrees["retrieval"] = retrieval_subtree

    metrics = dict(subtrees.get("metrics") if isinstance(subtrees.get("metrics"), dict) else {})
    metrics.update(
        {
            "status": "blocked" if blocker_count else "review_only",
            "retrieval_table_benchmark": {
                "status": status,
                "record_count": record_count,
                "retrieval_unit_count": retrieval_unit_count,
                "table_candidate_count": table_candidate_count,
                "included_review_only_count": retrieval_subtree["included_review_only_count"],
                "blocked_count": retrieval_subtree["blocked_count"],
                "repair_required_count": retrieval_subtree["repair_required_count"],
                "ranking_tie_count": retrieval_subtree["ranking_tie_count"],
                "diagnostic_count": len(diagnostics),
                "diagnostic_counts_by_code": diagnostic_counts,
                "manifest_path": retrieval_subtree["manifest"]["path"],
                "manifest_sha256": retrieval_subtree["manifest"]["sha256"],
                "manifest_schema": retrieval_subtree["manifest"]["schema_version"],
                "import_eligible_count": 0,
                "promoted_to_fact_count": 0,
                "production_import_attempted": False,
                "ladybugdb_written": False,
            },
            "import_eligible_count": 0,
            "promoted_to_fact_count": 0,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        }
    )
    subtrees["metrics"] = metrics

    payload["subtrees"] = subtrees
    payload["import_eligible_count"] = 0
    payload["promoted_to_fact_count"] = 0
    payload["production_import_attempted"] = False
    payload["ladybugdb_written"] = False
    payload["safety_flags"] = default_safety_flags()
    return payload

def to_json(value: ArticleEvidenceBundle | ArticleEvidenceRunSummary | dict[str, Any]) -> str:
    """Serialize a bridge artifact deterministically."""
    return json.dumps(to_redacted_dict(value), indent=2, sort_keys=True) + "\n"


def build_article_evidence_run_summary(
    *,
    run_id: str,
    bundles: Iterable[ArticleEvidenceBundle | dict[str, Any]],
    output_paths: dict[str, Any] | None = None,
    input_source_ids: Iterable[str] | None = None,
    input_hashes: Iterable[str] | None = None,
) -> dict[str, Any]:
    """Build a redacted run summary artifact for evidence bridge outputs."""
    bundle_dicts = tuple(to_redacted_dict(bundle) for bundle in bundles)
    return ArticleEvidenceRunSummary(
        run_id=run_id,
        bundles=bundle_dicts,
        output_paths=dict(output_paths or {}),
        input_source_ids=tuple(sorted(str(value) for value in (input_source_ids or ()))),
        input_hashes=tuple(sorted(str(value) for value in (input_hashes or ()))),
    ).to_redacted_dict()


def build_article_evidence_run_summary_from_load_events(
    *,
    run_id: str,
    bundles: Iterable[ArticleEvidenceBundle | dict[str, Any]],
    events: Iterable[dict[str, Any]],
    output_paths: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Build a run summary that fingerprints metadata-only replay inputs."""
    event_list = [dict(event) for event in events]
    return build_article_evidence_run_summary(
        run_id=run_id,
        bundles=bundles,
        output_paths=output_paths,
        input_source_ids=replay_input_source_ids(event_list),
        input_hashes=replay_input_hashes(event_list),
    )


def _page_index_validation_diagnostics(page_index: dict[str, Any]) -> list[dict[str, Any]]:
    try:
        from arxiv_archive.article_page_index import validate_article_page_index
    except ImportError:
        return [_diagnostic("page_index_validator_unavailable", "/subtrees/page_index").to_redacted_dict()]
    return validate_article_page_index(page_index)


def _asset_manifest_validation_diagnostics(asset_manifest: dict[str, Any]) -> list[dict[str, Any]]:
    try:
        from arxiv_archive.article_assets import validate_article_asset_manifest
    except ImportError:
        return [_diagnostic("asset_manifest_validator_unavailable", "/subtrees/assets").to_redacted_dict()]
    return validate_article_asset_manifest(asset_manifest)


def _page_index_bridge_diagnostics(source_refs: list[dict[str, Any]]) -> list[dict[str, Any]]:
    diagnostics: list[ArticleEvidenceDiagnostic] = []
    if not source_refs:
        diagnostics.append(_diagnostic("page_index_missing_source_refs", "/source_refs"))
    for index, source in enumerate(source_refs):
        object_id = _string_or_none(source.get("source_id"))
        if not source.get("source_path"):
            diagnostics.append(_diagnostic("page_index_missing_source_path", f"/source_refs[{index}]/source_path", object_id))
        if source.get("load_outcome") in {"loaded", "loaded_metadata_only"} and not _valid_sha256(source.get("sha256")):
            diagnostics.append(_diagnostic("page_index_missing_source_hash", f"/source_refs[{index}]/sha256", object_id))
    return [diagnostic.to_redacted_dict() for diagnostic in diagnostics]


def _page_index_source_provenance(source_refs: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "source_count": len(source_refs),
        "source_ids": sorted(str(source.get("source_id")) for source in source_refs if source.get("source_id")),
        "source_paths": sorted(str(source.get("source_path")) for source in source_refs if source.get("source_path")),
        "source_hashes": sorted(str(source.get("sha256")) for source in source_refs if source.get("sha256")),
        "outcome_counts": _counts(source.get("load_outcome") for source in source_refs),
        "failure_counts": _counts(source.get("failure_reason") for source in source_refs if source.get("failure_reason")),
        "failed_source_present": any(source.get("load_outcome") == "failed" for source in source_refs),
    }




def _asset_bridge_diagnostics(
    asset_manifest: dict[str, Any],
    bundle_source_refs: list[dict[str, Any]],
    manifest_source_refs: list[dict[str, Any]],
    assets: list[dict[str, Any]],
    *,
    manifest_path: str | Path | None,
    manifest_sha256: str | None,
) -> list[dict[str, Any]]:
    diagnostics: list[ArticleEvidenceDiagnostic] = []
    if manifest_path is None or not str(manifest_path).strip():
        diagnostics.append(_diagnostic("assets_missing_manifest_path", "/subtrees/assets/manifest/path"))
    if not _valid_sha256(manifest_sha256):
        diagnostics.append(_diagnostic("assets_missing_manifest_sha256", "/subtrees/assets/manifest/sha256"))
    if not bundle_source_refs:
        diagnostics.append(_diagnostic("assets_missing_bundle_source_refs", "/source_refs"))
    if assets and not manifest_source_refs:
        diagnostics.append(_diagnostic("assets_missing_manifest_source_refs", "/asset_manifest/source_refs"))

    bundle_source_ids = {str(source.get("source_id")) for source in bundle_source_refs if source.get("source_id")}
    manifest_source_ids = {str(source.get("source_id")) for source in manifest_source_refs if source.get("source_id")}
    for source_id in sorted(manifest_source_ids - bundle_source_ids):
        diagnostics.append(_diagnostic("assets_source_ref_not_in_bundle", "/asset_manifest/source_refs", source_id))
    for index, asset in enumerate(assets):
        object_id = _string_or_none(asset.get("asset_id") or asset.get("source_asset_ref"))
        source_file_id = _string_or_none(asset.get("source_file_id"))
        if source_file_id and source_file_id not in manifest_source_ids:
            diagnostics.append(_diagnostic("assets_record_unknown_source_ref", f"/asset_manifest/assets[{index}]/source_file_id", object_id))
        if asset.get("import_eligible") is not False:
            diagnostics.append(_diagnostic("assets_record_import_eligible", f"/asset_manifest/assets[{index}]/import_eligible", object_id))
        if asset.get("promoted_to_fact") is not False:
            diagnostics.append(_diagnostic("assets_record_promoted_to_fact", f"/asset_manifest/assets[{index}]/promoted_to_fact", object_id))

    for field_name in ("import_eligible_count", "promoted_to_fact_count"):
        if asset_manifest.get(field_name) != 0:
            diagnostics.append(_diagnostic(f"assets_{field_name}_nonzero", f"/asset_manifest/{field_name}"))
    for field_name in ("production_import_attempted", "ladybugdb_written"):
        if asset_manifest.get(field_name) is not False:
            diagnostics.append(_diagnostic(f"assets_{field_name}_true", f"/asset_manifest/{field_name}"))
    return [diagnostic.to_redacted_dict() for diagnostic in diagnostics]


def _asset_source_provenance(
    bundle_source_refs: list[dict[str, Any]], manifest_source_refs: list[dict[str, Any]], assets: list[dict[str, Any]]
) -> dict[str, Any]:
    asset_source_ids = sorted(str(asset.get("source_file_id")) for asset in assets if asset.get("source_file_id"))
    manifest_source_ids = sorted(str(source.get("source_id")) for source in manifest_source_refs if source.get("source_id"))
    bundle_source_ids = sorted(str(source.get("source_id")) for source in bundle_source_refs if source.get("source_id"))
    hashes = sorted(str(source.get("sha256")) for source in manifest_source_refs if _valid_sha256(source.get("sha256")))
    return {
        "bundle_source_count": len(bundle_source_refs),
        "manifest_source_count": len(manifest_source_refs),
        "referenced_source_count": len(set(asset_source_ids)),
        "bundle_source_ids": bundle_source_ids,
        "manifest_source_ids": manifest_source_ids,
        "referenced_source_ids": sorted(set(asset_source_ids)),
        "source_hash_count": len(hashes),
        "source_hash_coverage_rate": len(hashes) / len(manifest_source_refs) if manifest_source_refs else 0.0,
        "source_paths": sorted(str(source.get("source_path")) for source in manifest_source_refs if source.get("source_path")),
    }


def _asset_page_index_provenance(assets: list[dict[str, Any]], asset_manifest: dict[str, Any]) -> dict[str, Any]:
    manifest = asset_manifest.get("page_index_manifest") if isinstance(asset_manifest.get("page_index_manifest"), dict) else {}
    return {
        "manifest_path": _string_or_none(manifest.get("manifest_path")),
        "manifest_sha256": _string_or_none(manifest.get("manifest_sha256")),
        "manifest_schema_version": _string_or_none(manifest.get("schema_version")),
        "node_ref_count": len({asset.get("page_index_node_id") for asset in assets if asset.get("page_index_node_id")}),
        "anchor_ref_count": len({asset.get("page_index_anchor_id") for asset in assets if asset.get("page_index_anchor_id")}),
    }


def _links_dedup_redacted_manifest(value: Any) -> dict[str, Any]:
    try:
        from arxiv_archive.article_links_dedup import (
            to_redacted_dict as links_dedup_to_redacted_dict,
        )
    except ImportError:
        return dict(value) if isinstance(value, dict) else {}
    if isinstance(value, dict) or hasattr(value, "to_redacted_dict"):
        try:
            return links_dedup_to_redacted_dict(value)
        except (TypeError, ValueError, AttributeError):
            return dict(value) if isinstance(value, dict) else {}
    return {}


def _links_dedup_validation_diagnostics(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    try:
        from arxiv_archive.article_links_dedup import validate_article_links_dedup_manifest
    except ImportError:
        return [_diagnostic("links_dedup_validator_unavailable", "/subtrees/links_dedup").to_redacted_dict()]
    return validate_article_links_dedup_manifest(manifest)


def _links_dedup_bridge_diagnostics(
    manifest: dict[str, Any],
    bundle_source_refs: list[dict[str, Any]],
    *,
    manifest_path: str | Path | None,
    manifest_sha256: str | None,
    bundle_paper_id: str | None,
) -> list[dict[str, Any]]:
    diagnostics: list[ArticleEvidenceDiagnostic] = []
    if manifest_path is None or not str(manifest_path).strip():
        diagnostics.append(_diagnostic("links_dedup_missing_manifest_path", "/subtrees/links_dedup/manifest/path"))
    if not _valid_sha256(manifest_sha256):
        diagnostics.append(_diagnostic("links_dedup_missing_manifest_sha256", "/subtrees/links_dedup/manifest/sha256"))
    if manifest.get("schema_version") != "m024-article-links-dedup.v1":
        diagnostics.append(_diagnostic("links_dedup_invalid_manifest_schema", "/links_dedup_manifest/schema_version"))
    manifest_paper_id = _string_or_none(manifest.get("paper_id"))
    if bundle_paper_id and manifest_paper_id and manifest_paper_id != bundle_paper_id:
        diagnostics.append(_diagnostic("links_dedup_paper_id_mismatch", "/links_dedup_manifest/paper_id", manifest_paper_id))
    manifest_source_refs = _list_of_dicts(manifest.get("source_refs"))
    if not bundle_source_refs:
        diagnostics.append(_diagnostic("links_dedup_missing_bundle_source_refs", "/source_refs"))
    if not manifest_source_refs:
        diagnostics.append(_diagnostic("links_dedup_missing_manifest_source_refs", "/links_dedup_manifest/source_refs"))

    bundle_source_ids = {str(source.get("source_id")) for source in bundle_source_refs if source.get("source_id")}
    bundle_source_hashes = {str(source.get("sha256")) for source in bundle_source_refs if _valid_sha256(source.get("sha256"))}
    for index, source in enumerate(manifest_source_refs):
        object_id = _string_or_none(source.get("source_id"))
        if not source.get("source_path"):
            diagnostics.append(_diagnostic("links_dedup_missing_source_path", f"/links_dedup_manifest/source_refs[{index}]/source_path", object_id))
        sha = source.get("sha256")
        if not _valid_sha256(sha):
            diagnostics.append(_diagnostic("links_dedup_missing_source_hash", f"/links_dedup_manifest/source_refs[{index}]/sha256", object_id))
        if object_id and bundle_source_ids and object_id not in bundle_source_ids:
            diagnostics.append(_diagnostic("links_dedup_source_ref_not_in_bundle", f"/links_dedup_manifest/source_refs[{index}]/source_id", object_id))
        if _valid_sha256(sha) and bundle_source_hashes and str(sha) not in bundle_source_hashes:
            diagnostics.append(_diagnostic("links_dedup_source_hash_not_in_bundle", f"/links_dedup_manifest/source_refs[{index}]/sha256", object_id))

    for field_name in ("import_eligible_count", "promoted_to_fact_count"):
        if manifest.get(field_name) != 0:
            diagnostics.append(_diagnostic(f"links_dedup_{field_name}_nonzero", f"/links_dedup_manifest/{field_name}"))
    for field_name in ("production_import_attempted", "ladybugdb_written", "trusted_kg_import_allowed"):
        if manifest.get(field_name) is True:
            diagnostics.append(_diagnostic(f"links_dedup_{field_name}_true", f"/links_dedup_manifest/{field_name}"))
    safety_flags = manifest.get("safety_flags") if isinstance(manifest.get("safety_flags"), dict) else {}
    for field_name in ("production_import_attempted", "ladybugdb_written", "trusted_kg_import_allowed", "raw_payloads_included", "model_outputs_included"):
        if safety_flags.get(field_name) is True:
            diagnostics.append(_diagnostic(f"links_dedup_safety_flag_true:{field_name}", f"/links_dedup_manifest/safety_flags/{field_name}"))
    return [diagnostic.to_redacted_dict() for diagnostic in diagnostics]


def _links_dedup_source_provenance(bundle_source_refs: list[dict[str, Any]], manifest_source_refs: list[dict[str, Any]]) -> dict[str, Any]:
    bundle_ids = sorted(str(source.get("source_id")) for source in bundle_source_refs if source.get("source_id"))
    manifest_ids = sorted(str(source.get("source_id")) for source in manifest_source_refs if source.get("source_id"))
    hashes = sorted(str(source.get("sha256")) for source in manifest_source_refs if _valid_sha256(source.get("sha256")))
    return {
        "bundle_source_count": len(bundle_source_refs),
        "manifest_source_count": len(manifest_source_refs),
        "bundle_source_ids": bundle_ids,
        "manifest_source_ids": manifest_ids,
        "source_hash_count": len(hashes),
        "source_hash_coverage_rate": len(hashes) / len(manifest_source_refs) if manifest_source_refs else 0.0,
        "source_paths": sorted(str(source.get("source_path")) for source in manifest_source_refs if source.get("source_path")),
    }


def _links_dedup_page_index_provenance(manifest: dict[str, Any]) -> dict[str, Any]:
    refs = manifest.get("page_index_refs") if isinstance(manifest.get("page_index_refs"), dict) else {}
    return {
        "manifest_path": _string_or_none(refs.get("manifest_path")),
        "manifest_sha256": _string_or_none(refs.get("manifest_sha256")),
        "manifest_schema_version": _string_or_none(refs.get("schema_version")),
        "node_ref_count": len(_string_list(refs.get("node_ids"))),
        "anchor_ref_count": len(_string_list(refs.get("anchor_ids"))),
    }



def _retrieval_table_redacted_manifest(value: Any) -> dict[str, Any]:
    try:
        from arxiv_archive.article_retrieval_tables import (
            to_redacted_dict as retrieval_tables_to_redacted_dict,
        )
    except ImportError:
        return dict(value) if isinstance(value, dict) else {}
    if isinstance(value, dict) or hasattr(value, "to_redacted_dict"):
        try:
            return retrieval_tables_to_redacted_dict(value.to_redacted_dict() if hasattr(value, "to_redacted_dict") else value)
        except (TypeError, ValueError, AttributeError):
            return dict(value) if isinstance(value, dict) else {}
    return {}


def _retrieval_table_validation_diagnostics(value: Any) -> list[dict[str, Any]]:
    try:
        from arxiv_archive.article_retrieval_tables import validate_article_retrieval_table_manifest
    except ImportError:
        return [_diagnostic("retrieval_table_validator_unavailable", "/subtrees/retrieval").to_redacted_dict()]
    raw_manifest = value.to_redacted_dict() if hasattr(value, "to_redacted_dict") else value
    if not isinstance(raw_manifest, dict):
        return [_diagnostic("retrieval_table_manifest_not_mapping", "/subtrees/retrieval").to_redacted_dict()]
    return [diagnostic.to_redacted_dict() for diagnostic in validate_article_retrieval_table_manifest(raw_manifest)]


def _retrieval_table_bridge_diagnostics(
    manifest: dict[str, Any],
    bundle_source_refs: list[dict[str, Any]],
    *,
    manifest_path: str | Path | None,
    manifest_sha256: str | None,
    bundle_paper_id: str | None,
) -> list[dict[str, Any]]:
    diagnostics: list[ArticleEvidenceDiagnostic] = []
    provided_path = str(manifest_path) if manifest_path is not None else None
    manifest_declared_path = _string_or_none(manifest.get("manifest_path"))
    provided_sha = manifest_sha256 if manifest_sha256 is not None else None
    manifest_declared_sha = _string_or_none(manifest.get("manifest_sha256"))
    if not provided_path and not manifest_declared_path:
        diagnostics.append(_diagnostic("retrieval_table_missing_manifest_path", "/subtrees/retrieval/manifest/path"))
    if provided_path is not None and not provided_path.strip():
        diagnostics.append(_diagnostic("retrieval_table_empty_manifest_path", "/subtrees/retrieval/manifest/path"))
    if provided_path and manifest_declared_path and provided_path != manifest_declared_path:
        diagnostics.append(_diagnostic("retrieval_table_manifest_path_mismatch", "/subtrees/retrieval/manifest/path"))
    if not _valid_sha256(provided_sha or manifest_declared_sha):
        diagnostics.append(_diagnostic("retrieval_table_missing_manifest_sha256", "/subtrees/retrieval/manifest/sha256"))
    if provided_sha and manifest_declared_sha and provided_sha != manifest_declared_sha:
        diagnostics.append(_diagnostic("retrieval_table_manifest_sha256_mismatch", "/subtrees/retrieval/manifest/sha256"))
    if manifest.get("schema_version") != "m024-article-retrieval-tables.v1":
        diagnostics.append(_diagnostic("retrieval_table_invalid_manifest_schema", "/retrieval_table_manifest/schema_version"))
    manifest_paper_id = _string_or_none(manifest.get("paper_id"))
    if bundle_paper_id and manifest_paper_id and manifest_paper_id != bundle_paper_id:
        diagnostics.append(_diagnostic("retrieval_table_paper_id_mismatch", "/retrieval_table_manifest/paper_id", manifest_paper_id))
    manifest_source_refs = _list_of_dicts(manifest.get("source_refs"))
    if not bundle_source_refs:
        diagnostics.append(_diagnostic("retrieval_table_missing_bundle_source_refs", "/source_refs"))
    if not manifest_source_refs:
        diagnostics.append(_diagnostic("retrieval_table_missing_manifest_source_refs", "/retrieval_table_manifest/source_refs"))

    bundle_source_ids = {str(source.get("source_id")) for source in bundle_source_refs if source.get("source_id")}
    bundle_source_hashes = {str(source.get("sha256")) for source in bundle_source_refs if _valid_sha256(source.get("sha256"))}
    for index, source in enumerate(manifest_source_refs):
        object_id = _string_or_none(source.get("source_id"))
        if not source.get("source_path"):
            diagnostics.append(_diagnostic("retrieval_table_missing_source_path", f"/retrieval_table_manifest/source_refs[{index}]/source_path", object_id))
        sha = source.get("sha256")
        if not _valid_sha256(sha):
            diagnostics.append(_diagnostic("retrieval_table_missing_source_hash", f"/retrieval_table_manifest/source_refs[{index}]/sha256", object_id))
        if object_id and bundle_source_ids and object_id not in bundle_source_ids:
            diagnostics.append(_diagnostic("retrieval_table_source_ref_not_in_bundle", f"/retrieval_table_manifest/source_refs[{index}]/source_id", object_id))
        if _valid_sha256(sha) and bundle_source_hashes and str(sha) not in bundle_source_hashes:
            diagnostics.append(_diagnostic("retrieval_table_source_hash_not_in_bundle", f"/retrieval_table_manifest/source_refs[{index}]/sha256", object_id))

    for field_name in ("import_eligible_count", "promoted_to_fact_count"):
        if manifest.get(field_name) != 0:
            diagnostics.append(_diagnostic(f"retrieval_table_{field_name}_nonzero", f"/retrieval_table_manifest/{field_name}"))
    return [diagnostic.to_redacted_dict() for diagnostic in diagnostics]


def _retrieval_table_source_provenance(bundle_source_refs: list[dict[str, Any]], manifest_source_refs: list[dict[str, Any]]) -> dict[str, Any]:
    bundle_ids = sorted(str(source.get("source_id")) for source in bundle_source_refs if source.get("source_id"))
    manifest_ids = sorted(str(source.get("source_id")) for source in manifest_source_refs if source.get("source_id"))
    hashes = sorted(str(source.get("sha256")) for source in manifest_source_refs if _valid_sha256(source.get("sha256")))
    return {
        "bundle_source_count": len(bundle_source_refs),
        "manifest_source_count": len(manifest_source_refs),
        "bundle_source_ids": bundle_ids,
        "manifest_source_ids": manifest_ids,
        "source_hash_count": len(hashes),
        "source_hash_coverage_rate": len(hashes) / len(manifest_source_refs) if manifest_source_refs else 0.0,
        "source_paths": sorted(str(source.get("source_path")) for source in manifest_source_refs if source.get("source_path")),
    }


def _retrieval_table_manifest_provenance(value: Any) -> dict[str, Any]:
    refs = value if isinstance(value, dict) else {}
    return {
        "manifest_path": _string_or_none(refs.get("manifest_path")),
        "manifest_sha256": _string_or_none(refs.get("manifest_sha256")),
        "manifest_schema_version": _string_or_none(refs.get("schema_version")),
        "node_ref_count": len(_string_list(refs.get("node_ids"))),
        "anchor_ref_count": len(_string_list(refs.get("anchor_ids"))),
        "asset_ref_count": len(_string_list(refs.get("asset_ids"))),
        "metadata_signal_ref_count": len(_string_list(refs.get("metadata_signal_ids"))),
        "dedup_candidate_ref_count": len(_string_list(refs.get("dedup_candidate_ids"))),
    }

def _float_from_mapping(value: Any, key: str) -> float:
    if not isinstance(value, dict):
        return 0.0
    item = value.get(key, 0.0)
    return float(item) if isinstance(item, int | float) else 0.0


def _int_from_mapping(value: Any, key: str) -> int:
    if not isinstance(value, dict):
        return 0
    item = value.get(key, 0)
    return item if isinstance(item, int) else 0


def _default_subtrees(source_refs: list[dict[str, Any]], summary: dict[str, Any]) -> dict[str, dict[str, Any]]:
    has_loaded = any(source.get("load_outcome") == "loaded" for source in source_refs)
    has_metadata_only = any(source.get("load_outcome") == "loaded_metadata_only" for source in source_refs)
    has_failed = any(source.get("load_outcome") == "failed" for source in source_refs)
    raw_status: BundleSubtreeStatus = "metadata_only" if source_refs else "absent"
    normalized_status: BundleSubtreeStatus = "review_only" if has_loaded else "blocked"
    downstream_status: BundleSubtreeStatus = "not_attempted"
    return {
        "raw": {
            "status": raw_status,
            "source_count": summary["source_count"],
            "checksum_count": summary["checksum_count"],
            "checksum_coverage_rate": summary["checksum_coverage_rate"],
            "raw_text_embedded": False,
            "raw_binary_embedded": False,
        },
        "normalized": {
            "status": normalized_status,
            "source_count": sum(1 for source in source_refs if source.get("load_outcome") == "loaded"),
            "review_only": True,
            "raw_text_embedded": False,
        },
        "page_index": {"status": downstream_status, "record_count": 0, "import_eligible_count": 0},
        "assets": {"status": downstream_status, "record_count": 0, "import_eligible_count": 0},
        "links_dedup": {
            "status": downstream_status,
            "record_count": 0,
            "citation_link_count": 0,
            "structural_link_count": 0,
            "metadata_signal_count": 0,
            "dedup_candidate_count": 0,
            "import_eligible_count": 0,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        },
        "retrieval": {"status": downstream_status, "record_count": 0, "embedding_generation_attempted": False},
        "staging": {"status": "blocked" if has_failed else downstream_status, "record_count": 0, "production_import_attempted": False},
        "metrics": {
            "status": "review_only" if source_refs else "absent",
            "outcome_counts": dict(summary["outcome_counts"]),
            "failure_counts": dict(summary["failure_counts"]),
            "metadata_only_present": has_metadata_only,
            "failed_source_present": has_failed,
            "promoted_to_fact_count": 0,
            "ladybugdb_written": False,
        },
    }


def _bundle_id(*, paper_id: str, run_id: str, source_refs: list[dict[str, Any]]) -> str:
    digest_input = json.dumps(
        {
            "paper_id": paper_id,
            "run_id": run_id,
            "source_refs": [
                {
                    "source_id": source.get("source_id"),
                    "sha256": source.get("sha256"),
                    "load_outcome": source.get("load_outcome"),
                    "failure_reason": source.get("failure_reason"),
                }
                for source in source_refs
            ],
        },
        sort_keys=True,
    )
    return f"article-evidence-bundle:{hashlib.sha256(digest_input.encode()).hexdigest()[:24]}"


def _validate_source_ref(source: dict[str, Any], path: str, paper_id: Any) -> list[ArticleEvidenceDiagnostic]:
    diagnostics: list[ArticleEvidenceDiagnostic] = []
    source_id = _string_or_none(source.get("source_id"))
    diagnostics.extend(
        _required(
            source,
            (
                "source_id",
                "paper_id",
                "source_path",
                "source_type",
                "media_type",
                "byte_size",
                "parser_name",
                "loader_name",
                "load_outcome",
                "warning_count",
                "duration_ms",
                "raw_text_embedded",
                "raw_binary_embedded",
            ),
            path,
        )
    )
    diagnostics.extend(_validate_non_empty_ids(source, ("source_id", "paper_id", "source_path", "source_type", "media_type", "parser_name", "loader_name"), path, source_id))
    if source.get("paper_id") != paper_id:
        diagnostics.append(_diagnostic("source_ref_paper_id_mismatch", f"{path}/paper_id", source_id))
    if source.get("load_outcome") not in ALLOWED_LOAD_OUTCOMES:
        diagnostics.append(_diagnostic("invalid_load_outcome", f"{path}/load_outcome", source_id))
    if source.get("load_outcome") == "failed" and not source.get("failure_reason"):
        diagnostics.append(_diagnostic("missing_failure_reason", f"{path}/failure_reason", source_id))
    if source.get("load_outcome") != "failed" and source.get("failure_reason") is not None:
        diagnostics.append(_diagnostic("unexpected_failure_reason", f"{path}/failure_reason", source_id))
    if not isinstance(source.get("byte_size"), int) or int(source.get("byte_size", -1)) < 0:
        diagnostics.append(_diagnostic("invalid_byte_size", f"{path}/byte_size", source_id))
    if not isinstance(source.get("warning_count"), int) or int(source.get("warning_count", -1)) < 0:
        diagnostics.append(_diagnostic("invalid_warning_count", f"{path}/warning_count", source_id))
    if not isinstance(source.get("duration_ms"), int) or int(source.get("duration_ms", -1)) < 0:
        diagnostics.append(_diagnostic("invalid_duration_ms", f"{path}/duration_ms", source_id))
    sha = source.get("sha256")
    if sha is not None and not _valid_sha256(sha):
        diagnostics.append(_diagnostic("invalid_sha256", f"{path}/sha256", source_id))
    if source.get("load_outcome") in {"loaded", "loaded_metadata_only"} and not _valid_sha256(sha):
        diagnostics.append(_diagnostic("missing_checksum_for_loaded_source", f"{path}/sha256", source_id))
    if source.get("raw_text_embedded") is not False:
        diagnostics.append(_diagnostic("source_ref_raw_text_embedded", f"{path}/raw_text_embedded", source_id))
    if source.get("raw_binary_embedded") is not False:
        diagnostics.append(_diagnostic("source_ref_raw_binary_embedded", f"{path}/raw_binary_embedded", source_id))
    return diagnostics


def _validate_subtrees(value: Any, source_refs: list[dict[str, Any]]) -> list[ArticleEvidenceDiagnostic]:
    if not isinstance(value, dict):
        return [_diagnostic("missing_subtrees", "/subtrees")]
    diagnostics: list[ArticleEvidenceDiagnostic] = []
    for name in ("raw", "normalized", "page_index", "assets", "links_dedup", "retrieval", "staging", "metrics"):
        subtree = value.get(name)
        if not isinstance(subtree, dict):
            diagnostics.append(_diagnostic("missing_subtree", f"/subtrees/{name}"))
            continue
        status = subtree.get("status")
        if status not in ALLOWED_SUBTREE_STATUSES:
            diagnostics.append(_diagnostic("invalid_subtree_status", f"/subtrees/{name}/status"))
        if subtree.get("import_eligible_count", 0) != 0:
            diagnostics.append(_diagnostic("subtree_import_eligible_count_nonzero", f"/subtrees/{name}/import_eligible_count"))
        if subtree.get("promoted_to_fact_count", 0) != 0:
            diagnostics.append(_diagnostic("subtree_promoted_to_fact_count_nonzero", f"/subtrees/{name}/promoted_to_fact_count"))
        if subtree.get("production_import_attempted", False) is not False:
            diagnostics.append(_diagnostic("subtree_production_import_attempted", f"/subtrees/{name}/production_import_attempted"))
        if subtree.get("ladybugdb_written", False) is not False:
            diagnostics.append(_diagnostic("subtree_ladybugdb_written", f"/subtrees/{name}/ladybugdb_written"))
    links_dedup = value.get("links_dedup")
    if isinstance(links_dedup, dict):
        diagnostics.extend(_validate_links_dedup_subtree(links_dedup, source_refs))
    if source_refs and value.get("raw", {}).get("status") == "absent":
        diagnostics.append(_diagnostic("raw_subtree_absent_with_sources", "/subtrees/raw/status"))
    return diagnostics


def _validate_links_dedup_subtree(subtree: dict[str, Any], source_refs: list[dict[str, Any]]) -> list[ArticleEvidenceDiagnostic]:
    diagnostics: list[ArticleEvidenceDiagnostic] = []
    for field_name in ("graph_import_claim", "trusted_kg_import_allowed"):
        if subtree.get(field_name, False) is not False:
            diagnostics.append(_diagnostic(f"links_dedup_{field_name}_true", f"/subtrees/links_dedup/{field_name}"))
    manifest = subtree.get("manifest") if isinstance(subtree.get("manifest"), dict) else {}
    if manifest.get("path") is not None and not str(manifest.get("path", "")).strip():
        diagnostics.append(_diagnostic("links_dedup_empty_manifest_path", "/subtrees/links_dedup/manifest/path"))
    if manifest.get("sha256") is not None and not _valid_sha256(manifest.get("sha256")):
        diagnostics.append(_diagnostic("links_dedup_invalid_manifest_sha256", "/subtrees/links_dedup/manifest/sha256"))
    provenance = subtree.get("source_provenance") if isinstance(subtree.get("source_provenance"), dict) else {}
    bundle_ids = {str(source.get("source_id")) for source in source_refs if source.get("source_id")}
    for source_id in _string_list(provenance.get("manifest_source_ids")):
        if bundle_ids and source_id not in bundle_ids:
            diagnostics.append(_diagnostic("links_dedup_source_ref_not_in_bundle", "/subtrees/links_dedup/source_provenance/manifest_source_ids", source_id))
    return diagnostics


def _validate_summary(value: Any, source_refs: list[dict[str, Any]]) -> list[ArticleEvidenceDiagnostic]:
    if not isinstance(value, dict):
        return [_diagnostic("missing_summary", "/summary")]
    diagnostics = _required(value, ("source_count", "outcome_counts", "failure_counts", "checksum_count", "checksum_coverage_rate", "safety_flags"), "/summary")
    if value.get("source_count") != len(source_refs):
        diagnostics.append(_diagnostic("summary_source_count_mismatch", "/summary/source_count"))
    if value.get("promoted_to_fact_count") != 0:
        diagnostics.append(_diagnostic("summary_promoted_to_fact_count_nonzero", "/summary/promoted_to_fact_count"))
    if value.get("import_eligible_count") != 0:
        diagnostics.append(_diagnostic("summary_import_eligible_count_nonzero", "/summary/import_eligible_count"))
    if value.get("production_import_attempted") is not False:
        diagnostics.append(_diagnostic("summary_production_import_attempted", "/summary/production_import_attempted"))
    if value.get("ladybugdb_written") is not False:
        diagnostics.append(_diagnostic("summary_ladybugdb_written", "/summary/ladybugdb_written"))
    diagnostics.extend(_validate_safety_flags(value.get("safety_flags"), "/summary/safety_flags"))
    return diagnostics


def _validate_diagnostic_record(record: dict[str, Any], path: str) -> list[ArticleEvidenceDiagnostic]:
    diagnostics = _required(record, ("code", "json_path", "severity", "blocks_import"), path)
    if record.get("severity") not in {"info", "warning", "repair_required", "error"}:
        diagnostics.append(_diagnostic("invalid_diagnostic_severity", f"{path}/severity", _string_or_none(record.get("object_id"))))
    if not isinstance(record.get("json_path"), str) or not str(record.get("json_path", "")).startswith("/"):
        diagnostics.append(_diagnostic("invalid_diagnostic_json_path", f"{path}/json_path", _string_or_none(record.get("object_id"))))
    return diagnostics


def _validate_uses(value: dict[str, Any], path: str) -> list[ArticleEvidenceDiagnostic]:
    diagnostics: list[ArticleEvidenceDiagnostic] = []
    allowed_uses = set(_string_list(value.get("allowed_uses")))
    excluded_uses = set(_string_list(value.get("excluded_uses")))
    if "trusted_kg_import" in allowed_uses:
        diagnostics.append(_diagnostic("trusted_import_allowed", f"{path}/allowed_uses" if path else "/allowed_uses"))
    for use in EXCLUDED_USES:
        if use not in excluded_uses:
            diagnostics.append(_diagnostic("missing_excluded_use", f"{path}/excluded_uses" if path else "/excluded_uses"))
    return diagnostics


def _validate_safety_flags(value: Any, path: str) -> list[ArticleEvidenceDiagnostic]:
    if not isinstance(value, dict):
        return [_diagnostic("missing_safety_flags", path)]
    diagnostics: list[ArticleEvidenceDiagnostic] = []
    for key, expected in default_safety_flags().items():
        if value.get(key) is not expected:
            code = f"safety_flag_true:{key}" if value.get(key) is True else f"safety_flag_invalid:{key}"
            diagnostics.append(_diagnostic(code, f"{path}/{key}"))
    return diagnostics


def _validate_forbidden_keys(value: Any, path: str = "") -> list[ArticleEvidenceDiagnostic]:
    diagnostics: list[ArticleEvidenceDiagnostic] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = f"{path}/{key}" if path else f"/{key}"
            if key in FORBIDDEN_PAYLOAD_KEYS:
                diagnostics.append(_diagnostic("forbidden_payload_key", child_path))
            diagnostics.extend(_validate_forbidden_keys(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            diagnostics.extend(_validate_forbidden_keys(child, f"{path}[{index}]"))
    return diagnostics


def _validate_source_of_truth_markers(value: Any, path: str = "") -> list[ArticleEvidenceDiagnostic]:
    diagnostics: list[ArticleEvidenceDiagnostic] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = f"{path}/{key}" if path else f"/{key}"
            if key.lower() in FORBIDDEN_SOURCE_OF_TRUTH_KEYS:
                diagnostics.append(_diagnostic("source_of_truth_claim", child_path))
            diagnostics.extend(_validate_source_of_truth_markers(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            diagnostics.extend(_validate_source_of_truth_markers(child, f"{path}[{index}]"))
    return diagnostics


def _validate_non_empty_ids(value: dict[str, Any], fields: tuple[str, ...], path: str, object_id: str | None) -> list[ArticleEvidenceDiagnostic]:
    diagnostics: list[ArticleEvidenceDiagnostic] = []
    for field_name in fields:
        if field_name in value and isinstance(value.get(field_name), str) and not value[field_name].strip():
            diagnostics.append(_diagnostic(f"empty_{field_name}", f"{path}/{field_name}", object_id))
    return diagnostics


def _validate_duplicate_ids(values: list[dict[str, Any]], field_name: str, path: str, code: str) -> list[ArticleEvidenceDiagnostic]:
    diagnostics: list[ArticleEvidenceDiagnostic] = []
    seen: set[str] = set()
    for index, value in enumerate(values):
        identifier = value.get(field_name)
        if not isinstance(identifier, str) or not identifier:
            continue
        if identifier in seen:
            diagnostics.append(_diagnostic(code, f"{path}[{index}]/{field_name}", identifier))
        else:
            seen.add(identifier)
    return diagnostics


def _required(value: dict[str, Any], fields: tuple[str, ...], path: str) -> list[ArticleEvidenceDiagnostic]:
    diagnostics: list[ArticleEvidenceDiagnostic] = []
    for field_name in fields:
        if field_name not in value or value.get(field_name) is None:
            diagnostics.append(_diagnostic(f"missing_{field_name}", f"{path}/{field_name}" if path else f"/{field_name}"))
    return diagnostics


def _diagnostic(code: str, json_path: str, object_id: str | None = None) -> ArticleEvidenceDiagnostic:
    return ArticleEvidenceDiagnostic(code=code, json_path=json_path, object_id=object_id)


def _valid_sha256(value: Any) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(character in "0123456789abcdef" for character in value.lower())


def _list_of_dicts(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def _string_list(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    return [str(item) for item in value if item is not None]


def _string_or_none(value: Any) -> str | None:
    return str(value) if value is not None else None


def _replay_event_hash(event: dict[str, Any]) -> str:
    redacted_fingerprint = {
        key: event.get(key)
        for key in (
            "event",
            "source_id",
            "source_path",
            "source_type",
            "media_type",
            "sha256",
            "byte_size",
            "parser_name",
            "loader_name",
            "outcome",
            "failure_reason",
            "duration_ms",
            "warning_count",
        )
    }
    if event.get("paper_id") is not None:
        redacted_fingerprint["paper_id"] = event.get("paper_id")
    digest = hashlib.sha256(json.dumps(redacted_fingerprint, sort_keys=True, default=str).encode()).hexdigest()
    return f"source-load-event:{digest}"


def _counts(values: Any) -> dict[str, int]:
    counts: dict[str, int] = {}
    for value in values:
        if value is None:
            continue
        key = str(value)
        counts[key] = counts.get(key, 0) + 1
    return dict(sorted(counts.items()))


def _merge_counts(values: Any) -> dict[str, int]:
    counts: dict[str, int] = {}
    for value in values:
        if not isinstance(value, dict):
            continue
        for key, count in value.items():
            if isinstance(count, int):
                counts[str(key)] = counts.get(str(key), 0) + count
    return dict(sorted(counts.items()))


__all__ = [
    "ALLOWED_LOAD_OUTCOMES",
    "ALLOWED_REPLAY_EVENTS",
    "ALLOWED_SUBTREE_STATUSES",
    "ALLOWED_USES",
    "ARTICLE_EVIDENCE_BUNDLE_SCHEMA_VERSION",
    "ARTICLE_EVIDENCE_DIAGNOSTICS_SCHEMA_VERSION",
    "ARTICLE_EVIDENCE_RUN_SCHEMA_VERSION",
    "ArticleEvidenceBundle",
    "ArticleEvidenceDiagnostic",
    "ArticleEvidenceReplayError",
    "ArticleEvidenceRunSummary",
    "ArticleEvidenceSourceReference",
    "attach_assets_summary",
    "attach_links_dedup_summary",
    "attach_page_index_summary",
    "attach_retrieval_table_benchmark_summary",
    "build_article_evidence_bundle",
    "build_article_evidence_bundle_from_load_events",
    "build_article_evidence_run_summary",
    "build_article_evidence_run_summary_from_load_events",
    "default_safety_flags",
    "summarize_source_refs",
    "replay_input_hashes",
    "replay_input_source_ids",
    "to_json",
    "to_redacted_dict",
    "validate_article_evidence_bundle",
    "validate_article_load_events",
]
