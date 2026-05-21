from __future__ import annotations

import json

import pytest

from arxiv_archive.minimax_structured import (
    build_minimax_structured_request,
    validate_minimax_tool_response,
)


def _schema() -> dict:
    return {
        "type": "object",
        "properties": {
            "verdict": {"type": "string", "enum": ["pass", "flag"]},
            "confidence": {"type": "number"},
            "reasons": {"type": "array", "items": {"type": "string"}},
        },
        "required": ["verdict", "confidence", "reasons"],
    }


def test_builds_forced_anthropic_tool_request_and_validates_tool_input() -> None:
    request = build_minimax_structured_request(
        prompt="Review this redacted candidate packet.",
        tool_name="record_review",
        tool_description="Record a bounded review verdict.",
        input_schema=_schema(),
        payload_class="redacted",
    )

    assert request.endpoint == "https://api.minimax.io/anthropic/v1/messages"
    assert request.auth_header == "X-Api-Key"
    assert request.body["tool_choice"] == {"type": "tool", "name": "record_review"}
    assert request.body["tools"][0]["input_schema"] == _schema()
    assert request.body["temperature"] > 0
    assert request.raw_corpus_payload_allowed is False
    assert request.minimax_source_of_truth is False

    result = validate_minimax_tool_response(
        [
            {"type": "thinking", "thinking": "internal reasoning omitted from artifacts"},
            {
                "type": "tool_use",
                "name": "record_review",
                "input": {"verdict": "pass", "confidence": 0.8, "reasons": ["schema-valid"]},
            },
        ],
        tool_name="record_review",
        input_schema=_schema(),
    )

    dumped = json.dumps(result.to_sanitized_dict())
    assert result.valid is True
    assert result.helper_evidence_only is True
    assert result.minimax_source_of_truth is False
    assert result.tool_name == "record_review"
    assert "internal reasoning" not in dumped
    assert result.raw_model_content_persisted is False


def test_rejects_prompt_only_json_and_schema_invalid_tool_input() -> None:
    prompt_only = validate_minimax_tool_response(
        [{"type": "text", "text": '{"verdict":"pass","confidence":0.8,"reasons":[]}'}],
        tool_name="record_review",
        input_schema=_schema(),
    )

    assert prompt_only.valid is False
    assert prompt_only.diagnostic_codes == ("missing_tool_use",)
    assert "verdict" not in json.dumps(prompt_only.to_sanitized_dict())

    invalid_tool_input = validate_minimax_tool_response(
        [
            {
                "type": "tool_use",
                "name": "record_review",
                "input": {"verdict": "maybe", "confidence": "high"},
            }
        ],
        tool_name="record_review",
        input_schema=_schema(),
    )

    assert invalid_tool_input.valid is False
    assert "schema_enum_mismatch:$.verdict" in invalid_tool_input.diagnostic_codes
    assert "schema_type_mismatch:$.confidence:number" in invalid_tool_input.diagnostic_codes
    assert "schema_missing_required:$.reasons" in invalid_tool_input.diagnostic_codes
    assert invalid_tool_input.raw_model_content_persisted is False


def test_rejects_raw_corpus_payloads_and_invalid_temperature() -> None:
    with pytest.raises(ValueError, match="synthetic or redacted"):
        build_minimax_structured_request(
            prompt="RAW PAPER TEXT: full article body",
            tool_name="record_review",
            tool_description="Record a bounded review verdict.",
            input_schema=_schema(),
            payload_class="raw_corpus",
        )

    with pytest.raises(ValueError, match="temperature"):
        build_minimax_structured_request(
            prompt="Review this redacted candidate packet.",
            tool_name="record_review",
            tool_description="Record a bounded review verdict.",
            input_schema=_schema(),
            payload_class="redacted",
            temperature=0,
        )
