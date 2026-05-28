"""MiniMax structured helper adapter for article artifact hints.

This module is intentionally pure: it prepares Anthropic-compatible MiniMax
forced-tool requests and validates already-received tool-use responses, but it
never performs network I/O. MiniMax output is helper evidence only and is never
trusted for KG import or promoted to fact by this adapter.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from typing import Any

from arxiv_archive.article_artifacts import (
    ALLOWED_ARTIFACT_TYPES,
    ARTICLE_ARTIFACT_SCHEMA_VERSION,
    EXCLUDED_USES,
    REDACTED_ARTICLE_STRUCTURE_SCHEMA_VERSION,
    build_article_artifact_manifest_from_structure,
)
from arxiv_archive.minimax_structured import (
    MiniMaxStructuredRequest,
    build_minimax_structured_request,
    validate_minimax_tool_response,
)

MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION = "m023-minimax-artifact-helper.v1"
MINIMAX_ARTIFACT_HELPER_TOOL_NAME = "record_article_artifact_hints"
MINIMAX_ARTIFACT_HELPER_DETECTOR = "minimax_artifact_helper_review_only"
REQUEST_MODE = "forced_tool_redacted_article_structure"


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
    input_sha256 = _stable_hash(structure)
    summary_sha256 = _stable_hash(summary)
    prompt = _build_prompt(summary, input_sha256=input_sha256, summary_sha256=summary_sha256)
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
        input_sha256=input_sha256,
        summary_sha256=summary_sha256,
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
    input_sha256 = _stable_hash(structure)
    summary = _summarize_redacted_structure(structure, max_candidates=max_candidates)
    summary_sha256 = _stable_hash(summary)
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
                input_sha256=input_sha256,
                summary_sha256=summary_sha256,
                max_candidates=max_candidates,
                response_validation_status="invalid",
                diagnostic_codes=tuple(diagnostic_codes),
                refusal_codes=tuple(refusal_codes),
            ),
        )

    tool_input = _first_tool_input(content_blocks)
    candidates = tuple(_sanitize_candidate(candidate) for candidate in tool_input.get("artifact_hints", []))
    diagnostics = _base_diagnostics(
        input_sha256=input_sha256,
        summary_sha256=summary_sha256,
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
            "source_schema_version": {"type": "string", "enum": [REDACTED_ARTICLE_STRUCTURE_SCHEMA_VERSION]},
            "manifest_schema_version": {"type": "string", "enum": [ARTICLE_ARTIFACT_SCHEMA_VERSION]},
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


def _summarize_redacted_structure(structure: dict[str, Any], *, max_candidates: int) -> dict[str, Any]:
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
    return {
        "artifact_id": str(candidate["artifact_id"]),
        "artifact_type": str(candidate["artifact_type"]),
        "review_state": "review_required",
        "confidence_label": str(candidate.get("confidence_label") or "unknown"),
        "evidence_span_ids": [str(value) for value in candidate.get("evidence_span_ids", [])],
        "diagnostic_codes": ["minimax_helper_review_required", *[str(value) for value in candidate.get("diagnostic_codes", [])]],
        "detector": MINIMAX_ARTIFACT_HELPER_DETECTOR,
        "allowed_uses": ["artifact_review", "candidate_link_review", "provenance_diagnostics"],
        "excluded_uses": list(EXCLUDED_USES),
        "helper_evidence_only": True,
        "minimax_source_of_truth": False,
        "promoted_to_fact": False,
        "import_eligible": False,
        "raw_model_content_persisted": False,
    }


def _base_diagnostics(
    *,
    input_sha256: str,
    summary_sha256: str,
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
        "input_sha256": input_sha256,
        "redacted_summary_sha256": summary_sha256,
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
        if block.get("type") == "tool_use" and block.get("name") == MINIMAX_ARTIFACT_HELPER_TOOL_NAME:
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
    "MINIMAX_ARTIFACT_HELPER_DETECTOR",
    "MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION",
    "MINIMAX_ARTIFACT_HELPER_TOOL_NAME",
    "MiniMaxArtifactHelperRequest",
    "MiniMaxArtifactHelperResult",
    "article_artifact_minimax_hint_schema",
    "build_article_artifact_minimax_request",
    "validate_article_artifact_minimax_response",
]
