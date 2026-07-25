"""MiniMax structured helper adapter for article artifact hints.

This module is intentionally pure: it prepares Anthropic-compatible MiniMax
forced-tool requests and validates already-received tool-use responses, but it
never performs network I/O. MiniMax output is helper evidence only and is never
trusted for KG import or promoted to fact by this adapter.

M050 (Bounded LLM Helper v2 Worker Pool) adds a work requester layer on top
of the existing request/response functions:

- request_article_artifact_classification(structure, ...) returns an
  ArticleArtifactWorkRequest with deterministic work_id (per M048 patterns-review
  01 §4.2 and M049 compute_work_id).
- The actual MiniMax HTTP call lives in article_artifact_worker.py
  (separated, bounded ProcessPoolExecutor, can run as parallel workers).
- Reducer in `research_graph.infrastructure.papers.artifacts.reducer` merges work.completed events
  idempotently sorted by work_id.

This module (`research_graph.infrastructure.papers.artifacts.minimax_boundary`) is the **request and validation
boundary**. The actual call to MiniMax lives elsewhere.

Formerly: src/arxiv_archive/article_artifact_minimax.py

Formerly: src/arxiv_archive/artifacts/minimax_boundary.py
"""

from __future__ import annotations

import datetime
import hashlib
import json
from dataclasses import dataclass
from typing import Any

from research_graph.infrastructure.llm.minimax_structured import (
    MiniMaxStructuredRequest,
    build_minimax_structured_request,
    validate_minimax_tool_response,
)
from research_graph.infrastructure.llm.models_registry import (
    ModelsRegistry,
    compute_work_id,
    get_model_for_binding,
    load_models_registry,
)
from research_graph.infrastructure.papers.artifacts.models import (
    ALLOWED_ARTIFACT_TYPES,
    ALLOWED_CANDIDATE_LINK_TYPES,
    ARTICLE_ARTIFACT_SCHEMA_VERSION,
    EXCLUDED_USES,
    FORBIDDEN_PAYLOAD_KEYS,
    FORBIDDEN_SOURCE_OF_TRUTH_KEYS,
    REDACTED_ARTICLE_STRUCTURE_SCHEMA_VERSION,
    TRUSTED_IMPORT_USE,
    build_article_artifact_manifest_from_structure,
)

MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION = "minimax-artifact-helper.v1"
MINIMAX_ARTIFACT_HELPER_TOOL_NAME = "record_article_artifact_hints"
MINIMAX_ARTIFACT_HELPER_DETECTOR = "minimax_artifact_helper_review_only"
REQUEST_MODE = "forced_tool_redacted_article_structure"

# Default binding_id for article artifact classification (M049 bindings)
DEFAULT_ARTICLE_ARTIFACT_BINDING = "article-artifact-classify"


@dataclass(frozen=True, repr=False)
class MiniMaxArtifactHelperRequest:
    """Prepared MiniMax helper request plus sanitized diagnostics."""

    structured_request: MiniMaxStructuredRequest
    diagnostics: dict[str, Any]

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "schema_version": MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION,
            "request": self.structured_request.to_sanitized_dict(),
            "diagnostics": dict(self.diagnostics),
        }


@dataclass(frozen=True)
class MiniMaxArtifactHelperResult:
    """Validated, review-only MiniMax artifact hints and sanitized diagnostics."""

    candidates: tuple[dict[str, Any], ...]
    diagnostics: dict[str, Any]

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "schema_version": MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION,
            "candidate_count": len(self.candidates),
            "candidates": [dict(candidate) for candidate in self.candidates],
            "diagnostics": dict(self.diagnostics),
        }


@dataclass(frozen=True, repr=False)
class ArticleArtifactWorkRequest:
    """M050 work request: deterministic work_id + helper request + binding + run_id.

    Per M048 patterns-review 01 §4.2 and ActiveGraph pattern 3.1 (serial audit
    + parallel workers). The work_id is the deterministic cache key; the
    helper request is the payload a bounded ProcessPoolExecutor worker would
    dispatch to MiniMax.

    Diagnostic-only output (ADR-006): no graph writes, no promotion authority.
    """

    work_id: str
    binding_id: str
    model_id: str
    paper_id: str
    input_sha256: str
    max_candidates: int
    helper_request: MiniMaxArtifactHelperRequest
    created_at: str  # ISO 8601, but NOT included in work_id (deterministic)

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "work_id": self.work_id,
            "binding_id": self.binding_id,
            "model_id": self.model_id,
            "paper_id": self.paper_id,
            "input_sha256": self.input_sha256,
            "max_candidates": self.max_candidates,
            "created_at": self.created_at,
            "diagnostic_only": True,
            "import_eligible": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
            "graph_import_allowed": False,
        }


def request_article_artifact_classification(
    structure: dict[str, Any],
    *,
    max_candidates: int = 24,
    binding_id: str = DEFAULT_ARTICLE_ARTIFACT_BINDING,
    run_id: str | None = None,
    registry: ModelsRegistry | None = None,
) -> ArticleArtifactWorkRequest:
    """Build a work request for article artifact classification.

    Per M050 (Bounded LLM Helper v2) and M048 patterns-review 01 §4.2:
    - Computes work_id = sha256(model_id || binding_id || paper_id || max_candidates || input_sha256)
      via M049 compute_work_id.
    - Wraps the existing build_article_artifact_minimax_request as the helper payload.
    - Emits a serial audit event (work_id + binding + helper_request).

    The actual MiniMax HTTP call happens in article_artifact_worker.py
    (bounded ProcessPoolExecutor). This function does NOT perform network I/O.
    """
    if max_candidates < 1:
        raise ValueError("max_candidates must be positive")

    if registry is None:
        registry = load_models_registry()
    model = get_model_for_binding(registry, binding_id)

    # Validate structure up front (same checks as build_article_artifact_minimax_request).
    build_article_artifact_manifest_from_structure(structure)
    summary = _summarize_redacted_structure(structure, max_candidates=max_candidates)
    inputsha256 = _stable_hash(structure)
    summarysha256 = _stable_hash(summary)
    prompt = _build_prompt(summary, input_sha256=inputsha256, summary_sha256=summarysha256)
    structured_request = build_minimax_structured_request(
        prompt=prompt,
        tool_name=MINIMAX_ARTIFACT_HELPER_TOOL_NAME,
        tool_description=(
            "Record bounded, review-required article artifact hint candidates "
            "from a redacted structure summary. Never assert facts or import eligibility."
        ),
        input_schema=article_artifact_minimax_hint_schema(max_candidates=max_candidates),
        payload_class="redacted",
    )
    diagnostics = _base_diagnostics(
        inputsha256=inputsha256,
        summarysha256=summarysha256,
        max_candidates=max_candidates,
        response_validation_status="not_evaluated",
        diagnostic_codes=(),
    )
    diagnostics.update(
        {
            "paper_id": str(structure.get("paper_id")),
            "redacted_summary_counts": summary["counts"],
        }
    )
    helper_request = MiniMaxArtifactHelperRequest(
        structured_request=structured_request,
        diagnostics=diagnostics,
    )

    # Compute work_id via M049 deterministic formula.
    work_id = compute_work_id(
        model_id=model.id,
        binding_id=binding_id,
        input_data={
            "paper_id": str(structure.get("paper_id")),
            "max_candidates": max_candidates,
            "input_sha256": inputsha256,
        },
        prompt_data={
            "task": "classify_article_artifact",
            "tool_name": MINIMAX_ARTIFACT_HELPER_TOOL_NAME,
        },
        run_id=run_id,
    )

    return ArticleArtifactWorkRequest(
        work_id=work_id,
        binding_id=binding_id,
        model_id=model.id,
        paper_id=str(structure.get("paper_id")),
        input_sha256=inputsha256,
        max_candidates=max_candidates,
        helper_request=helper_request,
        created_at=datetime.datetime.now(tz=datetime.UTC).isoformat(),
    )


def build_article_artifact_minimax_request(
    structure: dict[str, Any], *, max_candidates: int = 24
) -> MiniMaxArtifactHelperRequest:
    """Build a forced-tool MiniMax request for redacted article artifact hints.

    The input structure is validated through the deterministic fixture boundary
    first. The prompt carries only IDs, types, coordinates, hashes, and counts;
    diagnostics carry hashes rather than raw structure or model responses.
    """

    if max_candidates < 1:
        raise ValueError("max_candidates must be positive")
    # Reuse the existing public boundary validator without trusting its output
    # as MiniMax input. It rejects raw payload keys and source-of-truth markers.
    build_article_artifact_manifest_from_structure(structure)
    summary = _summarize_redacted_structure(structure, max_candidates=max_candidates)
    inputsha256 = _stable_hash(structure)
    summarysha256 = _stable_hash(summary)
    prompt = _build_prompt(summary, input_sha256=inputsha256, summary_sha256=summarysha256)
    request = build_minimax_structured_request(
        prompt=prompt,
        tool_name=MINIMAX_ARTIFACT_HELPER_TOOL_NAME,
        tool_description=(
            "Record bounded, review-required article artifact hint candidates "
            "from a redacted structure summary. Never assert facts or import eligibility."
        ),
        input_schema=article_artifact_minimax_hint_schema(max_candidates=max_candidates),
        payload_class="redacted",
    )
    diagnostics = _base_diagnostics(
        inputsha256=inputsha256,
        summarysha256=summarysha256,
        max_candidates=max_candidates,
        response_validation_status="not_evaluated",
        diagnostic_codes=(),
    )
    diagnostics.update(
        {
            "paper_id": str(structure.get("paper_id")),
            "redacted_summary_counts": summary["counts"],
        }
    )
    return MiniMaxArtifactHelperRequest(structured_request=request, diagnostics=diagnostics)


def validate_article_artifact_minimax_response(
    content_blocks: list[dict[str, Any]], *, structure: dict[str, Any], max_candidates: int = 24
) -> MiniMaxArtifactHelperResult:
    """Validate MiniMax tool-use response and return review-required hints only."""

    if max_candidates < 1:
        raise ValueError("max_candidates must be positive")
    build_article_artifact_manifest_from_structure(structure)
    inputsha256 = _stable_hash(structure)
    summary = _summarize_redacted_structure(structure, max_candidates=max_candidates)
    summarysha256 = _stable_hash(summary)
    schema = article_artifact_minimax_hint_schema(max_candidates=max_candidates)
    validation = validate_minimax_tool_response(
        content_blocks,
        tool_name=MINIMAX_ARTIFACT_HELPER_TOOL_NAME,
        input_schema=schema,
    )
    diagnostic_codes = list(validation.diagnostic_codes)
    refusal_codes = _refusal_codes(content_blocks)
    diagnostic_codes.extend(refusal_codes)
    if not validation.valid:
        return MiniMaxArtifactHelperResult(
            candidates=(),
            diagnostics=_base_diagnostics(
                inputsha256=inputsha256,
                summarysha256=summarysha256,
                max_candidates=max_candidates,
                response_validation_status="invalid",
                diagnostic_codes=tuple(diagnostic_codes),
                refusal_codes=tuple(refusal_codes),
            ),
        )

    tool_input = _first_tool_input(content_blocks)
    semantic_diagnostics = _validate_tool_input_semantics(
        tool_input,
        structure=structure,
        input_sha256=inputsha256,
        max_candidates=max_candidates,
    )
    diagnostic_codes.extend(semantic_diagnostics)
    if semantic_diagnostics:
        return MiniMaxArtifactHelperResult(
            candidates=(),
            diagnostics=_base_diagnostics(
                inputsha256=inputsha256,
                summarysha256=summarysha256,
                max_candidates=max_candidates,
                response_validation_status="invalid",
                diagnostic_codes=tuple(diagnostic_codes),
                refusal_codes=tuple(refusal_codes),
            ),
        )

    candidates = tuple(
        _sanitize_candidate(candidate) for candidate in tool_input.get("artifact_hints", [])
    )
    diagnostics = _base_diagnostics(
        inputsha256=inputsha256,
        summarysha256=summarysha256,
        max_candidates=max_candidates,
        response_validation_status="valid",
        diagnostic_codes=tuple(diagnostic_codes),
        refusal_codes=tuple(refusal_codes),
    )
    diagnostics.update(
        {
            "provider_candidate_count": len(tool_input.get("artifact_hints", [])),
            "merged_candidate_count": len(candidates),
            "raw_model_content_persisted": False,
        }
    )
    return MiniMaxArtifactHelperResult(candidates=candidates, diagnostics=diagnostics)


def article_artifact_minimax_hint_schema(*, max_candidates: int = 24) -> dict[str, Any]:
    """Return the local JSON schema expected from the MiniMax forced tool."""

    return {
        "type": "object",
        "properties": {
            "schema_version": {"type": "string", "enum": [MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION]},
            "source_schema_version": {
                "type": "string",
                "enum": [REDACTED_ARTICLE_STRUCTURE_SCHEMA_VERSION],
            },
            "manifest_schema_version": {
                "type": "string",
                "enum": [ARTICLE_ARTIFACT_SCHEMA_VERSION],
            },
            "input_sha256": {"type": "string"},
            "artifact_hints": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "artifact_id": {"type": "string"},
                        "artifact_type": {"type": "string", "enum": sorted(ALLOWED_ARTIFACT_TYPES)},
                        "review_state": {"type": "string", "enum": ["review_required"]},
                        "confidence_label": {
                            "type": "string",
                            "enum": ["low", "medium", "high", "unknown", "needs_review"],
                        },
                        "evidence_span_ids": {"type": "array", "items": {"type": "string"}},
                        "diagnostic_codes": {"type": "array", "items": {"type": "string"}},
                        "candidate_links": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "link_id": {"type": "string"},
                                    "source_artifact_id": {"type": "string"},
                                    "target_ref": {"type": "string"},
                                    "link_type": {
                                        "type": "string",
                                        "enum": sorted(ALLOWED_CANDIDATE_LINK_TYPES),
                                    },
                                    "review_state": {"type": "string", "enum": ["review_required"]},
                                    "evidence_span_ids": {
                                        "type": "array",
                                        "items": {"type": "string"},
                                    },
                                    "diagnostic_codes": {
                                        "type": "array",
                                        "items": {"type": "string"},
                                    },
                                    "promoted_to_fact": {"type": "boolean", "enum": [False]},
                                    "import_eligible": {"type": "boolean", "enum": [False]},
                                },
                                "required": [
                                    "link_id",
                                    "source_artifact_id",
                                    "target_ref",
                                    "link_type",
                                    "review_state",
                                    "evidence_span_ids",
                                    "diagnostic_codes",
                                    "promoted_to_fact",
                                    "import_eligible",
                                ],
                            },
                        },
                    },
                    "required": [
                        "artifact_id",
                        "artifact_type",
                        "review_state",
                        "confidence_label",
                        "evidence_span_ids",
                        "diagnostic_codes",
                    ],
                },
            },
            "helper_limit": {"type": "integer"},
            "minimax_source_of_truth": {"type": "boolean", "enum": [False]},
            "promoted_to_fact": {"type": "boolean", "enum": [False]},
            "import_eligible": {"type": "boolean", "enum": [False]},
        },
        "required": [
            "schema_version",
            "source_schema_version",
            "manifest_schema_version",
            "input_sha256",
            "artifact_hints",
            "helper_limit",
            "minimax_source_of_truth",
            "promoted_to_fact",
            "import_eligible",
        ],
    }


def _build_prompt(summary: dict[str, Any], *, input_sha256: str, summary_sha256: str) -> str:
    payload = {
        "task": "Suggest bounded article artifact hints that require human/local review.",
        "rules": [
            "Use the forced tool only; do not answer with free text JSON.",
            "Return only review_required candidates.",
            "Do not claim source-of-truth status, fact promotion, or import eligibility.",
            "Use only redacted IDs, marker types, safe span IDs, coordinates, and hashes in this packet.",
        ],
        "input_sha256": input_sha256,
        "summary_sha256": summary_sha256,
        "redacted_structure_summary": summary,
    }
    return json.dumps(payload, sort_keys=True, separators=(",", ":"))


def _summarize_redacted_structure(
    structure: dict[str, Any], *, max_candidates: int
) -> dict[str, Any]:
    sections = _list_of_dicts(structure.get("sections"))
    placeholders = _list_of_dicts(structure.get("artifact_placeholders"))
    structured_markers = _list_of_dicts(structure.get("structured_markers"))
    scientific_markers = _list_of_dicts(structure.get("scientific_markers"))
    safe_spans = _list_of_dicts(structure.get("safe_spans"))
    marker_records = (placeholders + structured_markers + scientific_markers)[:max_candidates]
    return {
        "schema_version": str(structure.get("schema_version")),
        "paper_id": str(structure.get("paper_id")),
        "counts": {
            "sections": len(sections),
            "artifact_placeholders": len(placeholders),
            "structured_markers": len(structured_markers),
            "scientific_markers": len(scientific_markers),
            "safe_spans": len(safe_spans),
            "source_refs": len(_list_of_dicts(structure.get("source_refs"))),
            "max_candidates": max_candidates,
        },
        "sections": [
            {
                "section_id": section.get("section_id"),
                "parent_section_id": section.get("parent_section_id"),
                "section_type": section.get("section_type"),
                "ordinal_path": section.get("ordinal_path"),
                "span_id": section.get("span_id"),
            }
            for section in sections[:max_candidates]
        ],
        "marker_records": [
            {
                "artifact_id": marker.get("artifact_id"),
                "artifact_type": marker.get("artifact_type"),
                "section_id": marker.get("section_id"),
                "span_id": marker.get("span_id"),
                "caption_span_id": marker.get("caption_span_id"),
                "candidate_link_target_count": len(marker.get("candidate_link_targets", []))
                if isinstance(marker.get("candidate_link_targets"), list)
                else 0,
                "target_ref_hash": _hash_or_none(marker.get("target_ref")),
            }
            for marker in marker_records
        ],
        "safe_span_refs": [
            {
                "span_id": span.get("span_id"),
                "source_id": span.get("source_id"),
                "coordinate_space": span.get("coordinate_space"),
                "char_start": span.get("char_start"),
                "char_end": span.get("char_end"),
                "page_start": span.get("page_start"),
                "page_end": span.get("page_end"),
                "span_hash": span.get("span_hash"),
            }
            for span in safe_spans[: max_candidates * 2]
        ],
    }


def _sanitize_candidate(candidate: dict[str, Any]) -> dict[str, Any]:
    sanitized = {
        "artifact_id": str(candidate["artifact_id"]),
        "artifact_type": str(candidate["artifact_type"]),
        "review_state": "review_required",
        "confidence_label": str(candidate.get("confidence_label") or "unknown"),
        "evidence_span_ids": [str(value) for value in candidate.get("evidence_span_ids", [])],
        "diagnostic_codes": [
            "minimax_helper_review_required",
            *[str(value) for value in candidate.get("diagnostic_codes", [])],
        ],
        "detector": MINIMAX_ARTIFACT_HELPER_DETECTOR,
        "allowed_uses": ["artifact_review", "candidate_link_review", "provenance_diagnostics"],
        "excluded_uses": list(EXCLUDED_USES),
        "helper_evidence_only": True,
        "minimax_source_of_truth": False,
        "promoted_to_fact": False,
        "import_eligible": False,
        "raw_model_content_persisted": False,
    }
    candidate_links = candidate.get("candidate_links")
    if isinstance(candidate_links, list):
        sanitized["candidate_links"] = [  # pyrefly: ignore[bad-assignment]
            {
                "link_id": str(link["link_id"]),
                "source_artifact_id": str(link["source_artifact_id"]),
                "target_ref_hash": _stable_hash(str(link["target_ref"])),
                "link_type": str(link["link_type"]),
                "review_state": "review_required",
                "evidence_span_ids": [str(value) for value in link.get("evidence_span_ids", [])],
                "diagnostic_codes": [
                    "minimax_helper_link_review_required",
                    *[str(value) for value in link.get("diagnostic_codes", [])],
                ],
                "helper_evidence_only": True,
                "promoted_to_fact": False,
                "import_eligible": False,
            }
            for link in candidate_links
        ]
    return sanitized


def _validate_tool_input_semantics(
    tool_input: dict[str, Any], *, structure: dict[str, Any], input_sha256: str, max_candidates: int
) -> tuple[str, ...]:
    diagnostics: list[str] = []
    diagnostics.extend(_find_forbidden_tool_payload(tool_input))
    if tool_input.get("input_sha256") != input_sha256:
        diagnostics.append("input_sha256_mismatch:$.input_sha256")
    if tool_input.get("helper_limit") != max_candidates:
        diagnostics.append("helper_limit_mismatch:$.helper_limit")
    if tool_input.get("kg_import_allowed") is True:
        diagnostics.append("kg_import_allowed_true:$.kg_import_allowed")
    if tool_input.get("trusted_kg_import_allowed") is True:
        diagnostics.append("trusted_kg_import_allowed_true:$.trusted_kg_import_allowed")
    if TRUSTED_IMPORT_USE in _string_list(tool_input.get("allowed_uses")):
        diagnostics.append("trusted_import_allowed:$.allowed_uses")
    for key in ("production_import_attempted", "ladybugdb_written", "production_write_attempted"):
        if tool_input.get(key) is True:
            diagnostics.append(f"production_write_flag_true:$.{key}")

    artifact_hints = _list_of_dicts(tool_input.get("artifact_hints"))
    if len(artifact_hints) > max_candidates:
        diagnostics.append("too_many_artifact_hints:$.artifact_hints")
    existing_artifact_ids = _structure_artifact_ids(structure)
    seen_artifact_ids: set[str] = set()
    safe_span_ids = _structure_safe_span_ids(structure)
    helper_artifact_ids = {str(candidate.get("artifact_id")) for candidate in artifact_hints}
    for index, candidate in enumerate(artifact_hints):
        path = f"$.artifact_hints[{index}]"
        artifact_id = _string_or_none(candidate.get("artifact_id"))
        if artifact_id is None:
            continue
        if artifact_id in seen_artifact_ids:
            diagnostics.append(f"duplicate_helper_artifact_id:{path}.artifact_id")
        seen_artifact_ids.add(artifact_id)
        if artifact_id in existing_artifact_ids:
            diagnostics.append(f"unsafe_artifact_id_collision:{path}.artifact_id")
        if TRUSTED_IMPORT_USE in _string_list(candidate.get("allowed_uses")):
            diagnostics.append(f"trusted_import_allowed:{path}.allowed_uses")
        for key in (
            "kg_import_allowed",
            "trusted_kg_import_allowed",
            "production_import_attempted",
            "ladybugdb_written",
        ):
            if candidate.get(key) is True:
                diagnostics.append(f"unsafe_helper_flag_true:{path}.{key}")
        for span_index, span_id in enumerate(_string_list(candidate.get("evidence_span_ids"))):
            if span_id not in safe_span_ids:
                diagnostics.append(
                    f"unknown_evidence_span_id:{path}.evidence_span_ids[{span_index}]"
                )
        for link_index, link in enumerate(_list_of_dicts(candidate.get("candidate_links"))):
            link_path = f"{path}.candidate_links[{link_index}]"
            source_artifact_id = _string_or_none(link.get("source_artifact_id"))
            if source_artifact_id != artifact_id:
                diagnostics.append(f"invalid_candidate_link_source:{link_path}.source_artifact_id")
            target_ref = _string_or_none(link.get("target_ref"))
            if target_ref is None or (
                target_ref not in helper_artifact_ids and target_ref not in existing_artifact_ids
            ):
                diagnostics.append(f"invalid_candidate_link_target:{link_path}.target_ref")
            if link.get("link_type") not in ALLOWED_CANDIDATE_LINK_TYPES:
                diagnostics.append(f"invalid_candidate_link_type:{link_path}.link_type")
            if link.get("review_state") != "review_required":
                diagnostics.append(f"invalid_candidate_link_review_state:{link_path}.review_state")
            for key in (
                "promoted_to_fact",
                "import_eligible",
                "kg_import_allowed",
                "trusted_kg_import_allowed",
            ):
                if link.get(key) is True:
                    diagnostics.append(f"unsafe_candidate_link_flag_true:{link_path}.{key}")
            for span_index, span_id in enumerate(_string_list(link.get("evidence_span_ids"))):
                if span_id not in safe_span_ids:
                    diagnostics.append(
                        f"unknown_candidate_link_span_id:{link_path}.evidence_span_ids[{span_index}]"
                    )
    return tuple(diagnostics)


def _find_forbidden_tool_payload(value: Any, path: str = "$") -> list[str]:
    diagnostics: list[str] = []
    if isinstance(value, dict):
        for key, child in value.items():
            key_text = str(key)
            child_path = f"{path}.{key_text}" if path != "$" else f"$.{key_text}"
            normalized_key = key_text.lower()
            if normalized_key in FORBIDDEN_PAYLOAD_KEYS:
                diagnostics.append(f"forbidden_payload_key:{child_path}")
            if normalized_key in FORBIDDEN_SOURCE_OF_TRUTH_KEYS and child is not False:
                diagnostics.append(f"source_of_truth_claim:{child_path}")
            diagnostics.extend(_find_forbidden_tool_payload(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            diagnostics.extend(_find_forbidden_tool_payload(child, f"{path}[{index}]"))
    elif isinstance(value, str):
        normalized_value = value.upper()
        for marker in (
            "RAW PAPER TEXT",
            "RAW CHUNK TEXT",
            "FULL ARTICLE BODY",
            "BEGIN PDF",
            "BASE64",
        ):
            if marker in normalized_value:
                diagnostics.append(f"raw_payload_marker:{path}")
                break
    return diagnostics


def _structure_artifact_ids(structure: dict[str, Any]) -> set[str]:
    artifact_ids: set[str] = set()
    for key in ("artifact_placeholders", "structured_markers", "scientific_markers"):
        for record in _list_of_dicts(structure.get(key)):
            artifact_id = _string_or_none(record.get("artifact_id"))
            if artifact_id is not None:
                artifact_ids.add(artifact_id)
    for section in _list_of_dicts(structure.get("sections")):
        section_id = _string_or_none(section.get("section_id"))
        section_type = _string_or_none(section.get("section_type")) or "section"
        if section_id is not None:
            artifact_ids.add(
                f"{structure.get('paper_id')}:artifact:{section_type}:{section_id.rsplit(':', 1)[-1]}"
            )
    return artifact_ids


def _structure_safe_span_ids(structure: dict[str, Any]) -> set[str]:
    return {
        span_id
        for span in _list_of_dicts(structure.get("safe_spans"))
        if (span_id := _string_or_none(span.get("span_id"))) is not None
    }


def _string_list(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, str)]


def _string_or_none(value: Any) -> str | None:
    return value if isinstance(value, str) and value else None


def _base_diagnostics(
    *,
    inputsha256: str,
    summarysha256: str,
    max_candidates: int,
    response_validation_status: str,
    diagnostic_codes: tuple[str, ...],
    refusal_codes: tuple[str, ...] = (),
) -> dict[str, Any]:
    return {
        "schema_version": MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION,
        "request_mode": REQUEST_MODE,
        "source_schema_version": REDACTED_ARTICLE_STRUCTURE_SCHEMA_VERSION,
        "manifest_schema_version": ARTICLE_ARTIFACT_SCHEMA_VERSION,
        "tool_name": MINIMAX_ARTIFACT_HELPER_TOOL_NAME,
        "input_sha256": inputsha256,
        "redacted_summary_sha256": summarysha256,
        "max_candidates": max_candidates,
        "response_validation_status": response_validation_status,
        "diagnostic_codes": list(diagnostic_codes),
        "refusal_codes": list(refusal_codes),
        "payload_class": "redacted",
        "helper_evidence_only": True,
        "minimax_source_of_truth": False,
        "promoted_to_fact": False,
        "import_eligible": False,
        "raw_prompt_persisted": False,
        "raw_response_persisted": False,
        "credential_value_logged": False,
    }


def _first_tool_input(content_blocks: list[dict[str, Any]]) -> dict[str, Any]:
    for block in content_blocks:
        if (
            block.get("type") == "tool_use"
            and block.get("name") == MINIMAX_ARTIFACT_HELPER_TOOL_NAME
        ):
            tool_input = block.get("input")
            return tool_input if isinstance(tool_input, dict) else {}
    return {}


def _refusal_codes(content_blocks: list[dict[str, Any]]) -> tuple[str, ...]:
    for block in content_blocks:
        if block.get("type") != "text" or not isinstance(block.get("text"), str):
            continue
        if "refus" in block["text"].lower() or "cannot" in block["text"].lower():
            return ("provider_refusal_text_block",)
    return ()


def _stable_hash(value: Any) -> str:
    serialized = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    return hashlib.sha256(serialized.encode("utf-8")).hexdigest()


def _hash_or_none(value: Any) -> str | None:
    if not isinstance(value, str):
        return None
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _list_of_dicts(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


__all__ = [
    "DEFAULT_ARTICLE_ARTIFACT_BINDING",
    "MINIMAX_ARTIFACT_HELPER_DETECTOR",
    "MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION",
    "MINIMAX_ARTIFACT_HELPER_TOOL_NAME",
    "ArticleArtifactWorkRequest",
    "MiniMaxArtifactHelperRequest",
    "MiniMaxArtifactHelperResult",
    "article_artifact_minimax_hint_schema",
    "build_article_artifact_minimax_request",
    "request_article_artifact_classification",
    "validate_article_artifact_minimax_response",
]
