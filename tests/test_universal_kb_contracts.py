from __future__ import annotations

from dataclasses import FrozenInstanceError

import pytest

from research_graph.infrastructure.papers.artifacts.models import (
    default_safety_flags as article_default_safety_flags,
)
from research_graph.infrastructure.repair.chunk_import_contract import (
    ContractValidationResult,
    validation_to_dict,
)
from research_graph.workflows.universal_kb.contracts import (
    PIPELINE_FAILURE_CLASSES,
    PIPELINE_FAILURE_CODES,
    RETRYABLE_FAILURE_CODES,
    CandidatePacket,
    DependencyRecord,
    EvidenceArtifactRecord,
    FailureRecord,
    ProcessingJob,
    ReviewPacket,
    SafetyFlags,
    ToolInvocationRecord,
)


def test_safety_flags_default_to_no_write_and_are_frozen() -> None:
    flags = SafetyFlags()

    assert flags.graph_import_allowed is False
    assert flags.graphdb_written is False
    assert flags.ladybugdb_written is False
    assert flags.production_import_attempted is False
    assert flags.import_eligible is False
    assert flags.to_dict() == {
        "graph_import_allowed": False,
        "graphdb_written": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }
    flags.assert_no_write()

    with pytest.raises(FrozenInstanceError):
        flags.import_eligible = True  # type: ignore[misc]  # ty:ignore[invalid-assignment]


@pytest.mark.parametrize(
    "unsafe_flags",
    [
        {"graph_import_allowed": True},
        {"graphdb_written": True},
        {"ladybugdb_written": True},
        {"production_import_attempted": True},
        {"import_eligible": True},
    ],
)
def test_safety_flags_reject_write_authority_by_default(unsafe_flags: dict[str, bool]) -> None:
    with pytest.raises(ValueError, match="M034 forbids"):
        SafetyFlags(**unsafe_flags)


def test_candidate_packet_preserves_candidate_only_boundary() -> None:
    packet = CandidatePacket(
        candidate_id="candidate-1",
        evidence_refs=("artifact:1",),
        candidate_type="sidecar_layout",
    )

    assert packet.review_state == "pending"
    assert packet.safety_flags == SafetyFlags()
    assert packet.to_dict()["safety_flags"]["import_eligible"] is False
    packet.assert_no_write()


def test_candidate_packet_serializes_graph_projection_metadata() -> None:
    packet = CandidatePacket(
        candidate_id="candidate-1",
        evidence_refs=("artifact:1",),
        candidate_type="sidecar_layout",
        schema_version="universal-kb-candidate.v1",
        graph_node_refs=("node:paper:1",),
        graph_edge_refs=("edge:paper:1->claim:1",),
        provenance_refs=("source:arxiv:abc",),
        diagnostics=("missing_review_packet",),
    )

    dumped = packet.to_dict()

    assert dumped["schema_version"] == "universal-kb-candidate.v1"
    assert dumped["graph_node_refs"] == ["node:paper:1"]
    assert dumped["graph_edge_refs"] == ["edge:paper:1->claim:1"]
    assert dumped["provenance_refs"] == ["source:arxiv:abc"]
    assert dumped["diagnostics"] == ["missing_review_packet"]
    assert dumped["safety_flags"]["import_eligible"] is False


def test_candidate_packet_rejects_invalid_graph_projection_metadata() -> None:
    with pytest.raises(ValueError, match="schema_version must be non-empty"):
        CandidatePacket(
            candidate_id="candidate-1",
            evidence_refs=("artifact:1",),
            candidate_type="sidecar_layout",
            schema_version="",
        )

    with pytest.raises(ValueError, match="diagnostic refs contain forbidden keys"):
        CandidatePacket(
            candidate_id="candidate-1",
            evidence_refs=("artifact:1",),
            candidate_type="sidecar_layout",
            diagnostics=("raw_text",),
        )


@pytest.mark.parametrize("review_state", ["approved", "ready", "import_eligible"])
def test_candidate_packet_rejects_authoritative_review_states(review_state: str) -> None:
    with pytest.raises(
        ValueError, match="candidate packet cannot carry authoritative review state"
    ):
        CandidatePacket(
            candidate_id="candidate-1",
            evidence_refs=("artifact:1",),
            candidate_type="sidecar_layout",
            review_state=review_state,
        )


def test_review_packet_cannot_approve_or_import() -> None:
    packet = ReviewPacket(
        packet_id="review-1",
        candidate_refs=("candidate-1",),
        diagnostics=("missing_human_review",),
        reviewer_refs=("validator:deterministic",),
    )

    assert packet.review_required is True
    assert packet.review_state == "pending"
    assert packet.to_dict()["review_state"] == "pending"
    packet.assert_no_write()

    with pytest.raises(ValueError, match="review packet cannot approve readiness"):
        ReviewPacket(
            packet_id="review-2",
            candidate_refs=("candidate-1",),
            diagnostics=(),
            review_required=False,
            review_state="approved",
            reviewer_refs=("llm:helper",),
        )


def test_evidence_artifact_record_is_metadata_only() -> None:
    record = EvidenceArtifactRecord(
        artifact_id="artifact-1",
        artifact_type="opendataloader_layout",
        producer="opendataloader-adapter",
        input_hash="sha256:abc",
        tool_version="tool.v1",
        output_path="artifacts/candidate.json",
        diagnostic_refs=("diag:1",),
    )

    dumped = record.to_dict()
    assert dumped["artifact_id"] == "artifact-1"
    assert dumped["diagnostic_refs"] == ["diag:1"]
    assert "raw_text" not in dumped
    assert "embedding" not in dumped


@pytest.mark.parametrize(
    "record",
    [
        ProcessingJob(
            job_id="job-1",
            stage="candidate_generation",
            status="pending",
            attempt_count=0,
            retry_after=None,
            last_error_code=None,
            input_refs=("source:1",),
            output_paths=(),
        ),
        DependencyRecord(
            dependency_id="dep-1",
            upstream_ref="source:1",
            downstream_ref="job-1",
            required_state="succeeded",
            stale_on_hash_change=True,
        ),
        FailureRecord(
            failure_id="failure-1",
            job_id="job-1",
            failure_class="validation",
            error_code="missing_review_packet",
            retryable=False,
            redacted_message="Review packet is required before readiness handoff.",
            occurred_at="2026-06-08T00:00:00Z",
        ),
    ],
)
def test_operational_records_serialize_to_json_safe_dicts(record: object) -> None:
    dumped = record.to_dict()  # type: ignore[attr-defined]  # ty:ignore[unresolved-attribute]
    assert isinstance(dumped, dict)
    assert dumped


def test_pipeline_failure_taxonomy_covers_m195_failure_modes() -> None:
    assert {
        "network",
        "source",
        "resource",
        "llm",
        "artifact",
        "schema",
        "review",
        "validation",
        "queue",
    } <= PIPELINE_FAILURE_CLASSES
    assert {
        "network_unavailable",
        "arxiv_unavailable",
        "rate_limited",
        "resource_limit",
        "llm_limit",
        "stale_hash",
        "low_quality_source",
        "source_missing",
        "partial_artifact",
        "schema_validation_failed",
        "missing_review_packet",
        "incomplete_review_packet",
    } <= PIPELINE_FAILURE_CODES
    assert RETRYABLE_FAILURE_CODES <= PIPELINE_FAILURE_CODES


def test_failure_record_validates_pipeline_failure_taxonomy() -> None:
    record = FailureRecord(
        failure_id="failure-network",
        job_id="job-1",
        failure_class="network",
        error_code="network_unavailable",
        retryable=True,
        redacted_message="external dependency unavailable",
        occurred_at="2026-06-08T00:00:00Z",
    )

    assert record.to_dict()["error_code"] == "network_unavailable"

    with pytest.raises(ValueError, match="failure_class must be one of"):
        FailureRecord(
            failure_id="failure-unknown-class",
            job_id="job-1",
            failure_class="unknown",
            error_code="network_unavailable",
            retryable=True,
            redacted_message="external dependency unavailable",
            occurred_at="2026-06-08T00:00:00Z",
        )

    with pytest.raises(ValueError, match="error_code must be one of"):
        FailureRecord(
            failure_id="failure-unknown-code",
            job_id="job-1",
            failure_class="network",
            error_code="unknown_error",
            retryable=False,
            redacted_message="external dependency unavailable",
            occurred_at="2026-06-08T00:00:00Z",
        )

    with pytest.raises(ValueError, match="retryable must match"):
        FailureRecord(
            failure_id="failure-retry-mismatch",
            job_id="job-1",
            failure_class="network",
            error_code="network_unavailable",
            retryable=False,
            redacted_message="external dependency unavailable",
            occurred_at="2026-06-08T00:00:00Z",
        )

    with pytest.raises(ValueError, match="redacted_message must be metadata-only"):
        FailureRecord(
            failure_id="failure-raw-message",
            job_id="job-1",
            failure_class="source",
            error_code="low_quality_source",
            retryable=False,
            redacted_message="raw_text leaked",
            occurred_at="2026-06-08T00:00:00Z",
        )


def test_processing_job_rejects_negative_attempt_count() -> None:
    with pytest.raises(ValueError, match="attempt_count must be >= 0"):
        ProcessingJob(
            job_id="job-1",
            stage="candidate_generation",
            status="pending",
            attempt_count=-1,
            retry_after=None,
            last_error_code=None,
            input_refs=(),
            output_paths=(),
        )


def test_tool_invocation_record_is_sanitized_helper_evidence_only() -> None:
    record = ToolInvocationRecord(
        invocation_id="tool-1",
        tool_name="review_assistance",
        model="MiniMax-M3-512k",
        prompt_version="universal_kb_review_assistance_v1",
        input_hash="sha256:abc",
        schema_version="review-assistance.v1",
        redaction_state="redacted",
        diagnostic_refs=("diag:1",),
        latency_ms=123,
        cost_units=None,
    )

    dumped = record.to_sanitized_dict()
    assert dumped["helper_evidence_only"] is True
    assert dumped["minimax_source_of_truth"] is False
    assert dumped["raw_prompt_persisted"] is False
    assert dumped["credential_value_logged"] is False
    assert "prompt" not in dumped
    assert "raw_text" not in dumped


@pytest.mark.parametrize("redaction_state", ["raw", "unredacted", "contains_secret"])
def test_tool_invocation_record_rejects_unsafe_redaction_state(redaction_state: str) -> None:
    with pytest.raises(ValueError, match="redaction_state must be safe"):
        ToolInvocationRecord(
            invocation_id="tool-1",
            tool_name="review_assistance",
            model="MiniMax-M3-512k",
            prompt_version="v1",
            input_hash="sha256:abc",
            schema_version="review-assistance.v1",
            redaction_state=redaction_state,
            diagnostic_refs=(),
        )


def test_article_artifact_defaults_reuse_universal_safety_flags() -> None:
    flags = article_default_safety_flags()

    for key, value in SafetyFlags().to_dict().items():
        assert flags[key] is value
    assert flags["trusted_kg_import_allowed"] is False
    assert flags["raw_text_included"] is False


def test_chunk_validation_serialization_exposes_universal_safety_flags() -> None:
    result = ContractValidationResult(
        valid_package=False,
        import_eligible_chunk_count=0,
        refused_chunk_count=1,
        diagnostics=[],
    )

    dumped = validation_to_dict(result)

    assert dumped["safety_flags"] == SafetyFlags().to_dict()
    assert dumped["ladybugdb_written"] is False
    assert dumped["production_import_attempted"] is False
