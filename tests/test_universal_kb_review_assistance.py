from __future__ import annotations

from pathlib import Path

import pytest

from research_graph.workflows.universal_kb.contracts import CandidatePacket
from research_graph.workflows.universal_kb.review_assistance import (
    REVIEW_ASSISTANCE_PROMPT_VERSION,
    REVIEW_ASSISTANCE_SCHEMA_VERSION,
    ReviewAssistancePacket,
    build_review_assistance_packet,
    build_review_tool_invocation_record,
    review_assistance_prompt_hash,
    validate_review_assistance_tool_input,
)


def _candidate() -> CandidatePacket:
    return CandidatePacket(
        candidate_id="candidate-1",
        candidate_type="section_summary",
        evidence_refs=("artifact:fixture-paper:section:1",),
    )


def test_rejects_secret_shaped_review_diagnostics() -> None:
    with pytest.raises(ValueError, match="diagnostic must be metadata-only"):
        build_review_assistance_packet(
            candidate=_candidate(),
            diagnostics=("sk-live-abc1234567890",),
            confidence=0.6,
        )


def test_prompt_file_is_versioned_and_hashable() -> None:
    prompt_path = Path("prompts/universal_kb_review_assistance_v1.md")

    assert prompt_path.exists()
    assert REVIEW_ASSISTANCE_PROMPT_VERSION == "universal_kb_review_assistance_v1"
    assert review_assistance_prompt_hash().startswith("sha256:")
    assert "diagnostic-only" in prompt_path.read_text(encoding="utf-8")


def test_build_review_assistance_packet_is_diagnostic_only() -> None:
    packet = build_review_assistance_packet(
        candidate=_candidate(),
        diagnostics=("missing_supporting_locator",),
        confidence=0.42,
        flags=("needs_human_review",),
    )

    assert isinstance(packet, ReviewAssistancePacket)
    assert packet.candidate_id == "candidate-1"
    assert packet.schema_version == REVIEW_ASSISTANCE_SCHEMA_VERSION
    assert packet.diagnostics == ("missing_supporting_locator",)
    assert packet.flags == ("needs_human_review",)
    assert packet.review_state == "diagnostic_only"
    assert packet.safety_flags.import_eligible is False
    assert packet.safety_flags.graphdb_written is False


@pytest.mark.parametrize("review_state", ["approved", "ready", "import_eligible"])
def test_review_assistance_packet_rejects_authoritative_states(review_state: str) -> None:
    with pytest.raises(ValueError, match="diagnostic-only"):
        ReviewAssistancePacket(
            candidate_id="candidate-1",
            schema_version=REVIEW_ASSISTANCE_SCHEMA_VERSION,
            diagnostics=("needs_review",),
            confidence=0.5,
            flags=("review",),
            review_state=review_state,
        )


@pytest.mark.parametrize(
    "payload",
    [
        {"diagnostics": ["needs_review"], "confidence": 0.5, "flags": [], "approved": True},
        {"diagnostics": ["needs_review"], "confidence": 0.5, "flags": [], "import_eligible": True},
        {"diagnostics": ["raw_text: secret payload"], "confidence": 0.5, "flags": []},
        {"diagnostics": ["needs_review"], "confidence": 1.5, "flags": []},
    ],
)
def test_validate_review_assistance_tool_input_rejects_unsafe_payloads(payload: dict) -> None:
    with pytest.raises(ValueError):
        validate_review_assistance_tool_input(payload, candidate=_candidate())


def test_validate_review_assistance_tool_input_accepts_safe_payload() -> None:
    packet = validate_review_assistance_tool_input(
        {
            "diagnostics": ["missing_supporting_locator"],
            "confidence": 0.61,
            "flags": ["needs_human_review"],
        },
        candidate=_candidate(),
    )

    assert packet.to_dict()["candidate_id"] == "candidate-1"
    assert packet.to_dict()["safety_flags"]["production_import_attempted"] is False


def test_build_review_tool_invocation_record_is_sanitized() -> None:
    packet = build_review_assistance_packet(
        candidate=_candidate(),
        diagnostics=("missing_supporting_locator",),
        confidence=0.42,
        flags=("needs_human_review",),
    )

    record = build_review_tool_invocation_record(
        invocation_id="invoke-1",
        model="MiniMax-M3-512k",
        input_hash="sha256:redacted-candidate",
        review_packet=packet,
        latency_ms=123,
        cost_units=0.25,
    )
    sanitized = record.to_sanitized_dict()

    assert sanitized["tool_name"] == "universal_kb_review_assistance"
    assert sanitized["prompt_version"] == REVIEW_ASSISTANCE_PROMPT_VERSION
    assert sanitized["schema_version"] == REVIEW_ASSISTANCE_SCHEMA_VERSION
    assert sanitized["redaction_state"] == "redacted"
    assert sanitized["diagnostic_refs"] == ["candidate-1:missing_supporting_locator"]
    assert sanitized["latency_ms"] == 123
    assert sanitized["cost_units"] == 0.25
    assert sanitized["helper_evidence_only"] is True
    assert sanitized["minimax_source_of_truth"] is False
    assert "raw prompt" not in str(sanitized).lower()
    assert "raw_text" not in str(sanitized).lower()
    assert "secret" not in str(sanitized).lower()


def test_review_tool_invocation_record_rejects_unsafe_diagnostic_refs() -> None:
    packet = ReviewAssistancePacket(
        candidate_id="candidate-1",
        schema_version=REVIEW_ASSISTANCE_SCHEMA_VERSION,
        diagnostics=("safe_diagnostic",),
        confidence=0.5,
        flags=("needs_human_review",),
    )

    with pytest.raises(ValueError):
        build_review_tool_invocation_record(
            invocation_id="invoke-1",
            model="MiniMax-M3-512k",
            input_hash="sha256:redacted-candidate",
            review_packet=packet,
            diagnostic_refs=("raw_prompt: hidden instruction",),
        ).to_sanitized_dict()
