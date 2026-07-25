from __future__ import annotations

from pathlib import Path

import pytest

from research_graph.infrastructure.llm.minimax_structured import DEFAULT_MINIMAX_MODEL
from research_graph.workflows.universal_kb.contracts import CandidatePacket
from research_graph.workflows.universal_kb.queue import UniversalKBQueue
from research_graph.workflows.universal_kb.review_assistance import (
    build_review_assistance_packet,
    build_review_tool_invocation_record,
)
from research_graph.workflows.universal_kb.substrate_rehearsal import (
    NoWriteSubstrateRehearsal,
    ReadinessHandoff,
)


def _candidate() -> CandidatePacket:
    return CandidatePacket(
        candidate_id="candidate-1",
        candidate_type="section_summary",
        evidence_refs=("artifact:fixture-paper:section:1",),
    )


def _trace(candidate: CandidatePacket):
    packet = build_review_assistance_packet(
        candidate=candidate,
        diagnostics=("needs_locator",),
        confidence=0.7,
        flags=("needs_human_review",),
    )
    return build_review_tool_invocation_record(
        invocation_id="invoke-1",
        model=DEFAULT_MINIMAX_MODEL,
        input_hash="sha256:redacted-candidate",
        review_packet=packet,
    )


def test_rehearsal_builds_metadata_only_readiness_handoff(tmp_path: Path) -> None:
    candidate = _candidate()
    trace = _trace(candidate)
    queue = UniversalKBQueue(tmp_path / "queue.sqlite").initialize()
    queue.enqueue(
        job_id="candidate-1",
        stage="review_assistance",
        input_refs=candidate.evidence_refs,
        input_hash="sha256:redacted-candidate",
        tool_version="MiniMax-M3-512k",
        contract_version="m035-review-assistance.v1",
    )
    queue.unblock_ready_jobs()

    handoff = NoWriteSubstrateRehearsal(queue).build_handoff(
        candidate=candidate,
        review_trace=trace,
        queue_job_id="candidate-1",
    )

    assert isinstance(handoff, ReadinessHandoff)
    dumped = handoff.to_dict()
    assert dumped["candidate_id"] == "candidate-1"
    assert dumped["dry_run_only"] is True
    assert dumped["readiness_state"] == "diagnostics_only"
    assert dumped["queue_status"] == "ready"
    assert dumped["model"] == "MiniMax-M3-512k"
    assert dumped["safety_flags"]["graphdb_written"] is False
    assert dumped["safety_flags"]["production_import_attempted"] is False
    assert dumped["graph_write_allowed"] is False
    assert dumped["promotion_allowed"] is False
    assert "raw_text" not in str(dumped).lower()
    assert "secret" not in str(dumped).lower()


@pytest.mark.parametrize(
    "unsafe_field",
    ["graph_write_allowed", "promotion_allowed", "production_import_attempted"],
)
def test_handoff_rejects_any_write_or_promotion_authority(unsafe_field: str) -> None:
    kwargs = {
        "candidate_id": "candidate-1",
        "candidate_type": "section_summary",
        "evidence_refs": ("artifact:a",),
        "review_trace_ref": "invoke-1",
        "queue_job_id": "candidate-1",
        "queue_status": "ready",
        "model": "MiniMax-M3-512k",
        "prompt_version": "universal_kb_review_assistance_v1",
        "readiness_state": "diagnostics_only",
        unsafe_field: True,
    }

    with pytest.raises(ValueError, match="no-write rehearsal"):
        # pyrefly: ignore [bad-argument-type]
        ReadinessHandoff(**kwargs)  # ty:ignore[invalid-argument-type]


def test_rehearsal_rejects_authoritative_candidate_state(tmp_path: Path) -> None:
    candidate = CandidatePacket(
        candidate_id="candidate-1",
        candidate_type="section_summary",
        evidence_refs=("artifact:a",),
        review_state="diagnostic_only",
    )
    trace = _trace(candidate)
    queue = UniversalKBQueue(tmp_path / "queue.sqlite").initialize()
    queue.enqueue(
        job_id="candidate-1",
        stage="review_assistance",
        input_refs=candidate.evidence_refs,
        input_hash="sha256:redacted-candidate",
        tool_version="MiniMax-M3-512k",
        contract_version="m035-review-assistance.v1",
    )

    handoff = NoWriteSubstrateRehearsal(queue).build_handoff(
        candidate=candidate,
        review_trace=trace,
        queue_job_id="candidate-1",
    )

    assert handoff.readiness_state == "diagnostics_only"
    assert handoff.safety_flags.import_eligible is False
