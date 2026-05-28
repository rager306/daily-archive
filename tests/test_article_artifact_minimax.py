from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path

import pytest

from arxiv_archive.article_artifact_minimax import (
    MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION,
    MINIMAX_ARTIFACT_HELPER_TOOL_NAME,
    build_article_artifact_minimax_request,
    validate_article_artifact_minimax_response,
)

FIXTURE_DIR = Path(__file__).parent / "fixtures" / "article_artifacts"


def _structure() -> dict:
    return json.loads((FIXTURE_DIR / "basic_article_structure.json").read_text(encoding="utf-8"))


def _valid_tool_input(input_sha256: str) -> dict:
    return {
        "schema_version": MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION,
        "source_schema_version": "m023-redacted-article-structure.v1",
        "manifest_schema_version": "m023-article-artifacts.v1",
        "input_sha256": input_sha256,
        "helper_limit": 24,
        "artifact_hints": [
            {
                "artifact_id": "fixture-paper-0001:artifact:dataset:helper-0001",
                "artifact_type": "dataset",
                "review_state": "review_required",
                "confidence_label": "needs_review",
                "evidence_span_ids": ["fixture-paper-0001:span:section-methods"],
                "diagnostic_codes": ["suggested_by_redacted_structure_summary"],
            }
        ],
        "minimax_source_of_truth": False,
        "promoted_to_fact": False,
        "import_eligible": False,
    }


def test_builds_forced_tool_request_from_redacted_article_structure() -> None:
    structure = _structure()

    helper_request = build_article_artifact_minimax_request(structure)

    request = helper_request.structured_request
    safe = helper_request.to_sanitized_dict()
    dumped = json.dumps(safe)
    assert request.auth_header == "X-Api-Key"
    assert request.body["tool_choice"] == {"type": "tool", "name": MINIMAX_ARTIFACT_HELPER_TOOL_NAME}
    assert request.body["tools"][0]["input_schema"]["required"] == [
        "schema_version",
        "source_schema_version",
        "manifest_schema_version",
        "input_sha256",
        "artifact_hints",
        "helper_limit",
        "minimax_source_of_truth",
        "promoted_to_fact",
        "import_eligible",
    ]
    assert safe["diagnostics"]["request_mode"] == "forced_tool_redacted_article_structure"
    assert safe["diagnostics"]["response_validation_status"] == "not_evaluated"
    assert safe["diagnostics"]["payload_class"] == "redacted"
    assert safe["diagnostics"]["helper_evidence_only"] is True
    assert safe["diagnostics"]["minimax_source_of_truth"] is False
    assert safe["diagnostics"]["raw_prompt_persisted"] is False
    assert safe["diagnostics"]["raw_response_persisted"] is False
    assert len(safe["diagnostics"]["input_sha256"]) == 64
    assert len(safe["diagnostics"]["redacted_summary_sha256"]) == 64
    assert "external-reference:fixture-ref-0001" not in request.body["messages"][0]["content"]
    assert "external-reference:fixture-ref-0001" not in dumped
    assert "raw paper text" not in dumped.lower()


def test_validates_tool_response_and_merges_only_review_required_helper_candidates() -> None:
    structure = _structure()
    input_sha256 = build_article_artifact_minimax_request(structure).diagnostics["input_sha256"]

    result = validate_article_artifact_minimax_response(
        [
            {"type": "thinking", "thinking": "do not persist this chain of thought"},
            {
                "type": "tool_use",
                "name": MINIMAX_ARTIFACT_HELPER_TOOL_NAME,
                "input": _valid_tool_input(input_sha256),
            },
        ],
        structure=structure,
    )

    dumped = json.dumps(result.to_sanitized_dict())
    assert result.diagnostics["response_validation_status"] == "valid"
    assert result.diagnostics["provider_candidate_count"] == 1
    assert result.diagnostics["merged_candidate_count"] == 1
    assert result.candidates[0]["review_state"] == "review_required"
    assert result.candidates[0]["helper_evidence_only"] is True
    assert result.candidates[0]["minimax_source_of_truth"] is False
    assert result.candidates[0]["promoted_to_fact"] is False
    assert result.candidates[0]["import_eligible"] is False
    assert result.candidates[0]["raw_model_content_persisted"] is False
    assert "do not persist" not in dumped
    assert "source_of_truth" in dumped
    assert '"minimax_source_of_truth": true' not in dumped


def test_rejects_invalid_review_state_and_source_of_truth_flags_without_merging() -> None:
    structure = _structure()
    input_sha256 = build_article_artifact_minimax_request(structure).diagnostics["input_sha256"]
    tool_input = _valid_tool_input(input_sha256)
    tool_input["artifact_hints"][0]["review_state"] = "accepted"
    tool_input["minimax_source_of_truth"] = True

    result = validate_article_artifact_minimax_response(
        [{"type": "tool_use", "name": MINIMAX_ARTIFACT_HELPER_TOOL_NAME, "input": tool_input}],
        structure=structure,
    )

    assert result.candidates == ()
    assert result.diagnostics["response_validation_status"] == "invalid"
    assert "schema_enum_mismatch:$.artifact_hints[0].review_state" in result.diagnostics["diagnostic_codes"]
    assert "schema_enum_mismatch:$.minimax_source_of_truth" in result.diagnostics["diagnostic_codes"]
    assert result.diagnostics["raw_response_persisted"] is False


def test_rejects_prompt_only_or_refusal_responses_without_raw_response_persistence() -> None:
    structure = _structure()

    prompt_only = validate_article_artifact_minimax_response(
        [{"type": "text", "text": '{"artifact_hints": []}'}],
        structure=structure,
    )
    refusal = validate_article_artifact_minimax_response(
        [{"type": "text", "text": "I cannot provide that."}],
        structure=structure,
    )

    assert prompt_only.candidates == ()
    assert prompt_only.diagnostics["response_validation_status"] == "invalid"
    assert prompt_only.diagnostics["diagnostic_codes"] == ["missing_tool_use"]
    assert refusal.candidates == ()
    assert refusal.diagnostics["refusal_codes"] == ["provider_refusal_text_block"]
    assert "provider_refusal_text_block" in refusal.diagnostics["diagnostic_codes"]
    assert refusal.diagnostics["raw_response_persisted"] is False


def test_rejects_raw_payload_keys_before_building_request() -> None:
    structure = deepcopy(_structure())
    structure["artifact_placeholders"][0]["caption_text"] = "forbidden raw caption"

    with pytest.raises(ValueError, match="forbidden raw payload keys"):
        build_article_artifact_minimax_request(structure)


def test_rejects_invalid_candidate_limit() -> None:
    with pytest.raises(ValueError, match="positive"):
        build_article_artifact_minimax_request(_structure(), max_candidates=0)
