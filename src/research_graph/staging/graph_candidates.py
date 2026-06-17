# Formerly: src/arxiv_archive/staging/graph_candidates.py

"""Deterministic candidate locator generation for Scientific KG review packets.

This module implements the M020 candidate locator protocol as local-only,
review-only helpers. It reads source files to compute hashes and coordinates but
never serializes raw paper text, chunk text, embeddings, vectors, secrets, model
payloads, or import/write state.
"""

from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from research_graph.identity.canonicalization import artifact_record_hash, canonical_locator_id, canonical_source_id, stable_span_hash
from research_graph.identity.dedup import annotate_overlapping_signal_windows

CANDIDATE_LOCATOR_PROTOCOL_VERSION = "candidate_locator_protocol.v1"

ALLOWED_USES = ("candidate_locator_review", "provenance_diagnostics")
EXCLUDED_USES = (
    "trusted_kg_import",
    "production_ladybugdb_write",
    "embedding_generation",
    "source_of_truth_claim",
)

FORBIDDEN_PAYLOAD_KEYS = frozenset(
    {
        "text",
        "raw_text",
        "chunk_text",
        "paper_text",
        "claim_text",
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
        "raw_model_payload",
        "raw_minimax_response",
    }
)

ALLOWED_CANDIDATE_TYPES = frozenset(
    {
        "claim_candidate",
        "entity_candidate",
        "relation_candidate",
        "method_candidate",
        "dataset_candidate",
        "metric_candidate",
        "limitation_candidate",
        "citation_candidate",
        "retrieval_only_context",
        "repair_required_context",
    }
)
ALLOWED_ROUTES = frozenset(
    {
        "claim_location",
        "entity_location",
        "relation_location",
        "method_location",
        "dataset_location",
        "metric_location",
        "limitation_location",
        "citation_location",
        "retrieval_context",
        "repair_context",
    }
)
ALLOWED_STATES = frozenset(
    {
        "located_unreviewed",
        "review_required",
        "ambiguous_span",
        "missing_span",
        "conflicting_evidence",
        "unsupported",
        "retrieval_only",
        "repair_required",
        "rejected",
    }
)
ALLOWED_SUPPORT_LEVELS = frozenset(
    {"direct_span", "nearby_context", "multi_span", "insufficient", "contradicted", "not_evaluated"}
)
ALLOWED_UNCERTAINTY_LABELS = frozenset({"low", "medium", "high", "unknown"})
ALLOWED_REVIEW_QUEUE_REASONS = frozenset(
    {
        "needs_semantic_review",
        "span_missing",
        "span_ambiguous",
        "evidence_conflict",
        "conversion_quality_blocker",
        "locator_schema_error",
        "source_hash_missing",
        "candidate_type_uncertain",
        "retrieval_only",
        "repair_required",
        "not_reviewed",
    }
)
ALLOWED_COORDINATE_SPACES = frozenset(
    {"normalized_markdown_char", "semantic_chunk_char", "page_index_node", "artifact_record"}
)


@dataclass(frozen=True)
class LocatorSource:
    """One source artifact used to build review-only candidate locators."""

    source_id: str
    paper_id: str
    source_path: Path | str
    expected_sha256: str | None = None
    source_type: str = "markdown"
    conversion_method: str = "existing_markdown"


@dataclass(frozen=True)
class LocatorRouteSpec:
    """Route-specific deterministic signal configuration."""

    route_name: str
    candidate_type: str
    route: str
    signal_patterns: tuple[str, ...]


@dataclass(frozen=True)
class SourceReadResult:
    """Internal redaction-preserving source read result."""

    source: LocatorSource
    source_path: Path
    source_hash: str
    source_exists: bool
    hash_matches: bool
    content: str = ""


DEFAULT_ROUTE_SPECS = (
    LocatorRouteSpec(
        route_name="claim",
        candidate_type="claim_candidate",
        route="claim_location",
        signal_patterns=("claim", "result", "prove", "show", "demonstrat"),
    ),
    LocatorRouteSpec(
        route_name="method",
        candidate_type="method_candidate",
        route="method_location",
        signal_patterns=("method", "approach", "algorithm", "procedure", "model"),
    ),
    LocatorRouteSpec(
        route_name="retrieval",
        candidate_type="retrieval_only_context",
        route="retrieval_context",
        signal_patterns=("abstract", "introduction", "background", "related work"),
    ),
)


def default_safety_flags() -> dict[str, bool]:
    """Return required M020/M021 false safety flags."""
    return {
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "raw_text_included": False,
        "chunk_text_included": False,
        "raw_binary_included": False,
        "base64_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "secrets_included": False,
        "optimizer_traces_included": False,
        "model_payloads_included": False,
        "minimax_source_of_truth": False,
    }


def build_candidate_locator_artifact(
    *,
    run_id: str,
    paper_id: str,
    sources: list[LocatorSource] | tuple[LocatorSource, ...],
    route_specs: list[LocatorRouteSpec] | tuple[LocatorRouteSpec, ...] = DEFAULT_ROUTE_SPECS,
    broad_match_threshold: int = 8,
    window_before: int = 120,
    window_after: int = 240,
) -> dict[str, Any]:
    """Build a protocol-conformant candidate locator artifact.

    Source files are read only to compute hashes and coordinate offsets. Raw
    content is not included in the returned artifact.
    """
    read_results = [_read_source(source) for source in sources]
    source_ledger = [_source_ledger_item(result) for result in read_results]
    locators: list[dict[str, Any]] = []

    for result in read_results:
        line_starts = _line_starts(result.content)
        source_locators: list[dict[str, Any]] = []
        for spec in route_specs:
            source_locators.append(
                _build_locator(
                    paper_id=paper_id,
                    result=result,
                    spec=spec,
                    line_starts=line_starts,
                    broad_match_threshold=broad_match_threshold,
                    window_before=window_before,
                    window_after=window_after,
                )
            )
        annotate_overlapping_signal_windows(source_locators)
        locators.extend(source_locators)

    summary = _summary(source_ledger, locators)
    return {
        "schema_version": CANDIDATE_LOCATOR_PROTOCOL_VERSION,
        "run_id": run_id,
        "paper_id": paper_id,
        "source_ledger": source_ledger,
        "locators": locators,
        "summary": summary,
        "safety_flags": default_safety_flags(),
        "recommendation": "candidate_locator_review_only__positive_import_blocked",
    }


def build_candidate_locator_batch_from_targets(
    *,
    run_id: str,
    targets: list[dict[str, Any]] | tuple[dict[str, Any], ...],
    route_specs: list[LocatorRouteSpec] | tuple[LocatorRouteSpec, ...] = DEFAULT_ROUTE_SPECS,
    broad_match_threshold: int = 8,
    batch_paper_id: str = "bounded-target-batch",
) -> dict[str, Any]:
    """Build a deterministic locator batch from M011-style target records.

    Only target metadata, source paths, hashes, coordinates, and diagnostics are
    serialized. Source text is read transiently by ``build_candidate_locator_artifact``
    and is not embedded in the returned batch.
    """
    route_specs_by_m011_name = _route_specs_by_m011_name(route_specs)
    source_ledger: list[dict[str, Any]] = []
    locators: list[dict[str, Any]] = []
    per_paper_summary: list[dict[str, Any]] = []

    for target in targets:
        paper_id = str(target["paper_id"])
        target_source = target.get("source") or {}
        source = LocatorSource(
            source_id=canonical_source_id(paper_id),
            paper_id=paper_id,
            source_path=Path(str(target_source.get("path", "missing"))),
            expected_sha256=target_source.get("sha256"),
        )
        selected_specs = _route_specs_for_target(target, route_specs_by_m011_name)
        if not selected_specs:
            selected_specs = tuple(route_specs)
        paper_artifact = build_candidate_locator_artifact(
            run_id=f"{run_id}:{paper_id}",
            paper_id=paper_id,
            sources=(source,),
            route_specs=selected_specs,
            broad_match_threshold=broad_match_threshold,
        )
        source_ledger.extend(paper_artifact["source_ledger"])
        locators.extend(paper_artifact["locators"])
        per_paper_summary.append(
            {
                "paper_id": paper_id,
                "target_id": target.get("target_id"),
                "locator_count": paper_artifact["summary"]["locator_count"],
                "missing_span_count": paper_artifact["summary"]["missing_span_count"],
                "ambiguous_span_count": paper_artifact["summary"]["ambiguous_span_count"],
                "review_required_count": paper_artifact["summary"]["review_required_count"],
                "retrieval_only_count": paper_artifact["summary"]["retrieval_only_count"],
                "import_eligible_count": paper_artifact["summary"]["import_eligible_count"],
                "promoted_to_fact_count": paper_artifact["summary"]["promoted_to_fact_count"],
            }
        )

    summary = _summary(source_ledger, locators)
    summary["paper_count"] = len(targets)
    return {
        "schema_version": CANDIDATE_LOCATOR_PROTOCOL_VERSION,
        "run_id": run_id,
        "paper_id": batch_paper_id,
        "source_ledger": source_ledger,
        "locators": locators,
        "per_paper_summary": per_paper_summary,
        "summary": summary,
        "safety_flags": default_safety_flags(),
        "recommendation": "candidate_locator_batch_review_only__positive_import_blocked",
    }


def validate_candidate_locator_artifact(artifact: dict[str, Any]) -> list[str]:
    """Return redacted diagnostics for protocol/safety violations."""
    diagnostics: list[str] = []
    if artifact.get("schema_version") != CANDIDATE_LOCATOR_PROTOCOL_VERSION:
        diagnostics.append("invalid_schema_version")

    for hit in find_forbidden_payload_keys(artifact):
        diagnostics.append(hit)

    safety_flags = artifact.get("safety_flags")
    if not isinstance(safety_flags, dict):
        diagnostics.append("missing_safety_flags")
    else:
        for key, expected in default_safety_flags().items():
            if safety_flags.get(key) is not expected:
                diagnostics.append(f"safety_flag_true:{key}" if safety_flags.get(key) is True else f"safety_flag_invalid:{key}")

    source_ledger = artifact.get("source_ledger")
    if not isinstance(source_ledger, list) or not source_ledger:
        diagnostics.append("missing_source_ledger")
    else:
        for source in source_ledger:
            diagnostics.extend(_validate_source(source))

    locators = artifact.get("locators")
    if not isinstance(locators, list) or not locators:
        diagnostics.append("missing_locators")
    else:
        for locator in locators:
            diagnostics.extend(_validate_locator(locator))

    summary = artifact.get("summary")
    if not isinstance(summary, dict):
        diagnostics.append("missing_summary")
    else:
        if summary.get("import_eligible_count") != 0:
            diagnostics.append("summary_import_eligible_count_nonzero")
        if summary.get("promoted_to_fact_count") != 0:
            diagnostics.append("summary_promoted_to_fact_count_nonzero")

    return diagnostics


def write_candidate_locator_artifact(artifact: dict[str, Any], path: str | Path) -> Path:
    """Validate and write a candidate locator artifact as sorted JSON."""
    diagnostics = validate_candidate_locator_artifact(artifact)
    if diagnostics:
        raise ValueError(f"candidate locator artifact failed validation: {diagnostics}")
    output_path = Path(path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(artifact, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return output_path


def find_forbidden_payload_keys(value: Any, path: str = "") -> list[str]:
    """Return JSON paths containing exact forbidden payload keys."""
    hits: list[str] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = f"{path}/{key}" if path else f"/{key}"
            if key in FORBIDDEN_PAYLOAD_KEYS:
                hits.append(child_path)
            hits.extend(find_forbidden_payload_keys(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            hits.extend(find_forbidden_payload_keys(child, f"{path}[{index}]"))
    return hits


def _read_source(source: LocatorSource) -> SourceReadResult:
    source_path = Path(source.source_path)
    if not source_path.exists():
        return SourceReadResult(
            source=source,
            source_path=source_path,
            source_hash="missing",
            source_exists=False,
            hash_matches=False,
        )
    raw_bytes = source_path.read_bytes()
    source_hash = hashlib.sha256(raw_bytes).hexdigest()
    hash_matches = source.expected_sha256 is None or source_hash == source.expected_sha256
    content = raw_bytes.decode("utf-8", errors="replace") if hash_matches else ""
    return SourceReadResult(
        source=source,
        source_path=source_path,
        source_hash=source_hash,
        source_exists=True,
        hash_matches=hash_matches,
        content=content,
    )


def _source_ledger_item(result: SourceReadResult) -> dict[str, Any]:
    if not result.source_exists or not result.hash_matches:
        conversion_status = "blocked"
    else:
        conversion_status = "review_required"
    return {
        "source_id": result.source.source_id,
        "paper_id": result.source.paper_id,
        "source_type": result.source.source_type,
        "source_path": str(result.source_path),
        "source_hash": result.source_hash,
        "source_hash_algorithm": "sha256",
        "conversion_method": result.source.conversion_method,
        "conversion_status": conversion_status,
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
    }


def _build_locator(
    *,
    paper_id: str,
    result: SourceReadResult,
    spec: LocatorRouteSpec,
    line_starts: list[int],
    broad_match_threshold: int,
    window_before: int,
    window_after: int,
) -> dict[str, Any]:
    locator_id = canonical_locator_id(paper_id=paper_id, route_name=spec.route_name)
    span, diagnostics = _build_span(
        locator_id=locator_id,
        result=result,
        spec=spec,
        line_starts=line_starts,
        broad_match_threshold=broad_match_threshold,
        window_before=window_before,
        window_after=window_after,
    )
    state, support_level, uncertainty_label, review_reason = _classify_locator(spec, diagnostics)
    return {
        "locator_id": locator_id,
        "paper_id": paper_id,
        "candidate_type": spec.candidate_type,
        "route": spec.route,
        "state": state,
        "source_spans": [span],
        "support_level": support_level,
        "uncertainty_label": uncertainty_label,
        "review_queue_reason": review_reason,
        "diagnostic_codes": diagnostics,
        "allowed_uses": list(ALLOWED_USES),
        "excluded_uses": list(EXCLUDED_USES),
        "import_eligible": False,
        "promoted_to_fact": False,
        "minimax_source_of_truth": False,
    }


def _build_span(
    *,
    locator_id: str,
    result: SourceReadResult,
    spec: LocatorRouteSpec,
    line_starts: list[int],
    broad_match_threshold: int,
    window_before: int,
    window_after: int,
) -> tuple[dict[str, Any], list[str]]:
    diagnostics: list[str] = []
    if not result.source_exists:
        diagnostics.append("source_missing")
        return _artifact_record_span(locator_id, result, spec, diagnostics), diagnostics
    if not result.hash_matches:
        diagnostics.append("source_hash_mismatch")
        return _artifact_record_span(locator_id, result, spec, diagnostics), diagnostics

    pattern = _compile_route_pattern(spec)
    matches = list(pattern.finditer(result.content))
    if not matches:
        diagnostics.append("signal_missing")
        return _artifact_record_span(locator_id, result, spec, diagnostics), diagnostics

    if len(matches) > broad_match_threshold:
        diagnostics.append("broad_signal_many_matches")
    else:
        diagnostics.append("review_required")

    match = matches[0]
    char_start = max(0, match.start() - window_before)
    char_end = min(len(result.content), match.end() + window_after)
    span = {
        "span_id": f"{locator_id}-span-001",
        "source_id": result.source.source_id,
        "coordinate_space": "normalized_markdown_char",
        "char_start": char_start,
        "char_end": char_end,
        "line_start": _line_for(line_starts, char_start),
        "line_end": _line_for(line_starts, char_end),
        "span_hash": _span_hash(result.source.source_id, result.source_hash, char_start, char_end, spec.route_name),
        "raw_text_embedded": False,
        "ambiguity_diagnostics": diagnostics,
        "match_count_class": "many" if len(matches) > broad_match_threshold else "one_to_several",
    }
    return span, diagnostics


def _artifact_record_span(
    locator_id: str, result: SourceReadResult, spec: LocatorRouteSpec, diagnostics: list[str]) -> dict[str, Any]:
    return {
        "span_id": f"{locator_id}-artifact-record",
        "source_id": result.source.source_id,
        "coordinate_space": "artifact_record",
        "span_hash": artifact_record_hash(source_path=result.source_path, route_name=spec.route_name),
        "raw_text_embedded": False,
        "ambiguity_diagnostics": list(diagnostics),
    }


def _classify_locator(spec: LocatorRouteSpec, diagnostics: list[str]) -> tuple[str, str, str, str]:
    if "source_missing" in diagnostics or "source_hash_mismatch" in diagnostics or "signal_missing" in diagnostics:
        return "missing_span", "insufficient", "high", "span_missing"
    if "broad_signal_many_matches" in diagnostics:
        return "ambiguous_span", "nearby_context", "high", "span_ambiguous"
    if spec.candidate_type == "retrieval_only_context":
        return "retrieval_only", "nearby_context", "medium", "retrieval_only"
    if spec.candidate_type == "repair_required_context":
        return "repair_required", "insufficient", "high", "repair_required"
    return "review_required", "not_evaluated", "medium", "needs_semantic_review"


def _summary(source_ledger: list[dict[str, Any]], locators: list[dict[str, Any]]) -> dict[str, int]:
    return {
        "locator_count": len(locators),
        "source_count": len(source_ledger),
        "located_count": sum(1 for locator in locators if locator["state"] != "missing_span"),
        "review_required_count": sum(1 for locator in locators if locator["state"] == "review_required"),
        "missing_span_count": sum(1 for locator in locators if locator["state"] == "missing_span"),
        "ambiguous_span_count": sum(1 for locator in locators if locator["state"] == "ambiguous_span"),
        "conflicting_evidence_count": sum(1 for locator in locators if locator["state"] == "conflicting_evidence"),
        "retrieval_only_count": sum(1 for locator in locators if locator["state"] == "retrieval_only"),
        "repair_required_count": sum(1 for locator in locators if locator["state"] == "repair_required"),
        "import_eligible_count": sum(1 for locator in locators if locator["import_eligible"]),
        "promoted_to_fact_count": sum(1 for locator in locators if locator["promoted_to_fact"]),
    }


def _validate_source(source: dict[str, Any]) -> list[str]:
    diagnostics: list[str] = []
    required = {
        "source_id",
        "paper_id",
        "source_type",
        "source_path",
        "source_hash",
        "source_hash_algorithm",
        "conversion_method",
        "conversion_status",
        "raw_text_embedded",
        "raw_binary_embedded",
    }
    missing = sorted(required.difference(source))
    diagnostics.extend(f"missing_source_field:{field}" for field in missing)
    if source.get("raw_text_embedded") is not False:
        diagnostics.append(f"source_raw_text_embedded:{source.get('source_id', 'unknown')}")
    if source.get("raw_binary_embedded") is not False:
        diagnostics.append(f"source_raw_binary_embedded:{source.get('source_id', 'unknown')}")
    return diagnostics


def _validate_locator(locator: dict[str, Any]) -> list[str]:
    diagnostics: list[str] = []
    locator_id = str(locator.get("locator_id", "unknown"))
    required = {
        "locator_id",
        "paper_id",
        "candidate_type",
        "route",
        "state",
        "source_spans",
        "support_level",
        "uncertainty_label",
        "review_queue_reason",
        "diagnostic_codes",
        "allowed_uses",
        "excluded_uses",
        "import_eligible",
        "promoted_to_fact",
        "minimax_source_of_truth",
    }
    missing = sorted(required.difference(locator))
    diagnostics.extend(f"missing_locator_field:{locator_id}:{field}" for field in missing)
    if locator.get("candidate_type") not in ALLOWED_CANDIDATE_TYPES:
        diagnostics.append(f"invalid_candidate_type:{locator_id}")
    if locator.get("route") not in ALLOWED_ROUTES:
        diagnostics.append(f"invalid_route:{locator_id}")
    if locator.get("state") not in ALLOWED_STATES:
        diagnostics.append(f"invalid_state:{locator_id}")
    if locator.get("support_level") not in ALLOWED_SUPPORT_LEVELS:
        diagnostics.append(f"invalid_support_level:{locator_id}")
    if locator.get("uncertainty_label") not in ALLOWED_UNCERTAINTY_LABELS:
        diagnostics.append(f"invalid_uncertainty_label:{locator_id}")
    if locator.get("review_queue_reason") not in ALLOWED_REVIEW_QUEUE_REASONS:
        diagnostics.append(f"invalid_review_queue_reason:{locator_id}")
    if locator.get("import_eligible") is True:
        diagnostics.append(f"locator_import_eligible_true:{locator_id}")
    if locator.get("promoted_to_fact") is True:
        diagnostics.append(f"locator_promoted_to_fact_true:{locator_id}")
    if locator.get("minimax_source_of_truth") is True:
        diagnostics.append(f"locator_minimax_source_of_truth_true:{locator_id}")
    if not set(ALLOWED_USES).issubset(set(locator.get("allowed_uses") or ())):
        diagnostics.append(f"missing_allowed_uses:{locator_id}")
    if not set(EXCLUDED_USES).issubset(set(locator.get("excluded_uses") or ())):
        diagnostics.append(f"missing_excluded_uses:{locator_id}")
    source_spans = locator.get("source_spans")
    if not isinstance(source_spans, list) or not source_spans:
        diagnostics.append(f"missing_source_spans:{locator_id}")
    else:
        for span in source_spans:
            diagnostics.extend(_validate_span(locator_id, span))
    return diagnostics


def _validate_span(locator_id: str, span: dict[str, Any]) -> list[str]:
    diagnostics: list[str] = []
    required = {"span_id", "source_id", "coordinate_space", "span_hash", "raw_text_embedded"}
    missing = sorted(required.difference(span))
    diagnostics.extend(f"missing_span_field:{locator_id}:{field}" for field in missing)
    coordinate_space = span.get("coordinate_space")
    if coordinate_space not in ALLOWED_COORDINATE_SPACES:
        diagnostics.append(f"invalid_coordinate_space:{locator_id}")
    if span.get("raw_text_embedded") is not False:
        diagnostics.append(f"span_raw_text_embedded:{locator_id}")
    if coordinate_space != "artifact_record":
        char_start = span.get("char_start")
        char_end = span.get("char_end")
        if not isinstance(char_start, int) or not isinstance(char_end, int) or char_end <= char_start or char_start < 0:
            diagnostics.append(f"invalid_span_coordinates:{locator_id}")
    return diagnostics



def _route_specs_by_m011_name(route_specs: list[LocatorRouteSpec] | tuple[LocatorRouteSpec, ...]) -> dict[str, LocatorRouteSpec]:
    by_route_name = {spec.route_name: spec for spec in route_specs}
    mapping: dict[str, LocatorRouteSpec] = {}
    if "claim" in by_route_name:
        mapping["claim_extraction"] = by_route_name["claim"]
    if "method" in by_route_name:
        mapping["method_extraction"] = by_route_name["method"]
    if "retrieval" in by_route_name:
        mapping["retrieval_only"] = by_route_name["retrieval"]
    return mapping


def _route_specs_for_target(target: dict[str, Any], route_specs_by_m011_name: dict[str, LocatorRouteSpec]) -> tuple[LocatorRouteSpec, ...]:
    route_counts = ((target.get("review_metadata") or {}).get("counts_by_route") or {})
    selected: list[LocatorRouteSpec] = []
    for m011_route_name, spec in route_specs_by_m011_name.items():
        if route_counts.get(m011_route_name, 0) > 0:
            selected.append(spec)
    return tuple(selected)


def _compile_route_pattern(spec: LocatorRouteSpec) -> re.Pattern[str]:
    escaped = [re.escape(pattern) for pattern in spec.signal_patterns]
    return re.compile("|".join(escaped), re.IGNORECASE)


def _line_starts(content: str) -> list[int]:
    starts: list[int] = []
    position = 0
    for line in content.splitlines(keepends=True):
        starts.append(position)
        position += len(line)
    return starts or [0]


def _line_for(line_starts: list[int], offset: int) -> int:
    current = 1
    for index, start in enumerate(line_starts, start=1):
        if start <= offset:
            current = index
        else:
            break
    return current


def _span_hash(source_id: str, source_hash: str, char_start: int, char_end: int, route_name: str) -> str:
    packet = {
        "source_id": source_id,
        "source_hash": source_hash,
        "coordinate_space": "normalized_markdown_char",
        "char_start": char_start,
        "char_end": char_end,
        "route_name": route_name,
    }
    return stable_span_hash(
        source_id=source_id,
        source_hash=source_hash,
        char_start=char_start,
        char_end=char_end,
        route_name=route_name,
    )


__all__ = [
    "CANDIDATE_LOCATOR_PROTOCOL_VERSION",
    "DEFAULT_ROUTE_SPECS",
    "FORBIDDEN_PAYLOAD_KEYS",
    "LocatorRouteSpec",
    "LocatorSource",
    "build_candidate_locator_artifact",
    "build_candidate_locator_batch_from_targets",
    "default_safety_flags",
    "find_forbidden_payload_keys",
    "validate_candidate_locator_artifact",
    "write_candidate_locator_artifact",
]
