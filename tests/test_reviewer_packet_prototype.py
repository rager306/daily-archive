from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path

import pytest

from arxiv_archive.bounded_chunk_repair import build_bounded_chunk_repair_contract
from arxiv_archive.chunk_repair_contract import MARKDOWN_FORBIDDEN_PATTERNS
from arxiv_archive.reviewer_packet_prototype import (
    ALLOWED_NON_IMPORTING_DECISIONS,
    REVIEWER_PACKET_ASSESSMENT_VERSION,
    REVIEWER_PACKET_PROTOTYPE_VERSION,
    ReviewerPacketError,
    build_reviewer_packet_prototype,
    render_reviewer_packet_markdown,
    summarize_reviewer_packet_prototype,
)

CONTRACT_FIXTURE = Path("tests/fixtures/chunk_repair_contract.json")
LOCATOR_FIXTURE = Path("tests/fixtures/bounded_locator_batch.json")
FORBIDDEN_RENDER_VALUES = ("DO_NOT_LEAK", "SECRET", "NEVER LEAK")


def _locator_batch() -> dict[str, object]:
    return json.loads(LOCATOR_FIXTURE.read_text(encoding="utf-8"))


def _s02_contract() -> dict[str, object]:
    contract = json.loads(CONTRACT_FIXTURE.read_text(encoding="utf-8"))
    locators = _locator_batch()["locators"]
    contract["repair_targets"] = []
    contract["diagnostics"] = {
        **contract["diagnostics"],
        "target_count": 0,
        "pending_review_count": 0,
    }
    contract["stable_ids"] = {
        "source_ids": ["source-synthetic-paper-1-full-text"],
        "locator_ids": [locator["locator_id"] for locator in locators],
        "span_ids": [locator["source_spans"][0]["span_id"] for locator in locators],
    }
    return contract


def _s03_payload(max_target_count: int = 6) -> dict[str, object]:
    return build_bounded_chunk_repair_contract(_s02_contract(), _locator_batch(), max_target_count=max_target_count)


def _prototype() -> dict[str, object]:
    return build_reviewer_packet_prototype(_s03_payload(), s02_contract=_s02_contract())


def test_builds_deterministic_review_packets_and_independent_assessment() -> None:
    first = _prototype()
    second = _prototype()

    assert first == second
    assert first["schema_version"] == REVIEWER_PACKET_PROTOTYPE_VERSION
    assert first["packet_count"] == 3
    assert first["assessment"]["schema_version"] == REVIEWER_PACKET_ASSESSMENT_VERSION
    assert first["assessment"]["reviewer_id"] == "independent-agent"
    assert first["assessment"]["verdict"] == "blocked_pending_semantic_acceptance"
    assert [packet["locator_id"] for packet in first["packets"]] == [
        "m021-synthetic-paper-1-claim-001",
        "m021-synthetic-paper-1-retrieval-001",
        "m021-synthetic-paper-1-method-001",
    ]


def test_packets_are_pending_non_importable_and_copy_false_safety_boundaries() -> None:
    prototype = _prototype()

    for packet in prototype["packets"]:
        assert packet["review_status"] == "pending_review"
        assert packet["importable"] is False
        assert packet["semantic_ready_for_kg"] is False
        assert packet["raw_text_embedded"] is False
        assert packet["allowed_non_importing_decisions"] == list(ALLOWED_NON_IMPORTING_DECISIONS)
        assert packet["section_lineage"]["status"] == "unresolved"
        assert packet["review_questions"]
        assert packet["before_diagnostic_codes"]
        assert packet["after_diagnostic_codes"] == ["bounded_target_created", "kg_import_blocked"]
        assert all(value is False for value in packet["safety_boundaries"].values())
        for span in packet["span_refs"]:
            assert span["raw_text_embedded"] is False
            assert len(span["span_hash"]) == 64
            assert isinstance(span["char_start"], int)
            assert isinstance(span["char_end"], int)


def test_summary_reports_counts_verdict_and_zero_unsafe_counters() -> None:
    summary = summarize_reviewer_packet_prototype(_prototype())

    assert summary["packet_count"] == 3
    assert summary["review_status_counts"] == {"pending_review": 3}
    assert summary["repair_state_counts"] == {
        "ambiguous_span": 1,
        "retrieval_only": 1,
        "review_required": 1,
    }
    assert summary["route_quality_state_counts"] == {
        "broad_signal_many_matches": 1,
        "method_location": 1,
        "retrieval_only": 1,
    }
    assert summary["assessment_verdict"] == "blocked_pending_semantic_acceptance"
    assert summary["unsafe_counters"] == {
        "packet_count": 3,
        "pending_review_count": 3,
        "accepted_count": 0,
        "importable_count": 0,
        "semantic_ready_count": 0,
        "raw_text_embedded_count": 0,
        "unsafe_safety_boundary_count": 0,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "secrets_included": False,
        "embeddings_included": False,
        "vectors_included": False,
    }


@pytest.mark.parametrize(
    ("mutator", "code", "path"),
    [
        (lambda payload: payload.__setitem__("schema_version", "wrong"), "contract_validation_failed:schema_version_mismatch", "/schema_version"),
        (lambda payload: payload.__setitem__("repair_targets", []), "empty_repair_targets", "/repair_targets"),
        (
            lambda payload: payload["repair_targets"][0].__setitem__("review_status", "accepted"),
            "contract_validation_failed:missing_reviewer",
            "/repair_targets/0/reviewer",
        ),
        (
            lambda payload: payload["repair_targets"][0]["safety_boundaries"].__setitem__("import_eligible", True),
            "contract_validation_failed:import_eligible_true",
            "/repair_targets/0/safety_boundaries/import_eligible",
        ),
    ],
)
def test_malformed_s03_inputs_fail_closed_before_packet_output(mutator, code: str, path: str) -> None:
    payload = _s03_payload()
    mutator(payload)

    with pytest.raises(ReviewerPacketError) as exc_info:
        build_reviewer_packet_prototype(payload, s02_contract=_s02_contract())

    assert exc_info.value.code == code
    assert exc_info.value.path == path


def test_s02_lineage_subset_enforcement_rejects_unknown_locator_source_and_span_ids() -> None:
    payload = _s03_payload()
    s02 = _s02_contract()
    s02["stable_ids"]["locator_ids"] = ["different-locator"]

    with pytest.raises(ReviewerPacketError) as exc_info:
        build_reviewer_packet_prototype(payload, s02_contract=s02)

    assert exc_info.value.code == "locator_id_not_in_s02_stable_ids"
    assert exc_info.value.path == "/repair_targets/0/locator_id"

    s02 = _s02_contract()
    s02["stable_ids"]["source_ids"] = ["different-source"]
    with pytest.raises(ReviewerPacketError) as source_exc:
        build_reviewer_packet_prototype(payload, s02_contract=s02)
    assert source_exc.value.code == "source_id_not_in_s02_stable_ids"

    s02 = _s02_contract()
    s02["stable_ids"]["span_ids"] = ["different-span"]
    with pytest.raises(ReviewerPacketError) as span_exc:
        build_reviewer_packet_prototype(payload, s02_contract=s02)
    assert span_exc.value.code == "span_id_not_in_s02_stable_ids"


def test_forbidden_payload_key_rejection_reports_path_and_code_without_value() -> None:
    payload = _s03_payload()
    payload["repair_targets"][0]["nested"] = {"api_key": "DO_NOT_LEAK"}

    with pytest.raises(ReviewerPacketError) as exc_info:
        build_reviewer_packet_prototype(payload, s02_contract=_s02_contract())

    rendered_error = str(exc_info.value)
    assert exc_info.value.code == "secret_leakage"
    assert exc_info.value.path == "/repair_targets/0/nested/api_key"
    assert "DO_NOT_LEAK" not in rendered_error


def test_markdown_renderer_is_redacted_json_derived_and_has_no_code_fences_or_forbidden_marker_keys() -> None:
    markdown = render_reviewer_packet_markdown(_prototype())

    assert "# S04 Reviewer Packet Prototype" in markdown
    assert "Packet count: 3" in markdown
    assert "Assessment verdict: blocked_pending_semantic_acceptance" in markdown
    assert "Import allowed: false" in markdown
    assert "Before diagnostic codes:" in markdown
    assert "After diagnostic codes:" in markdown
    assert "pending_semantic_acceptance" in markdown
    assert "```" not in markdown
    assert all(pattern not in markdown for pattern in MARKDOWN_FORBIDDEN_PATTERNS)
    assert all(value not in markdown for value in FORBIDDEN_RENDER_VALUES)


def test_markdown_renderer_rejects_mutated_packet_safety_and_importability() -> None:
    prototype = _prototype()
    prototype["packets"][0]["importable"] = True

    with pytest.raises(ReviewerPacketError) as exc_info:
        render_reviewer_packet_markdown(prototype)

    assert exc_info.value.code == "packet_importable"
    assert exc_info.value.path == "/packets/0/importable"

    prototype = _prototype()
    prototype["packets"][0]["safety_boundaries"]["semantic_ready_for_kg"] = True
    with pytest.raises(ReviewerPacketError) as safety_exc:
        render_reviewer_packet_markdown(prototype)
    assert safety_exc.value.code == "packet_unsafe_safety_boundary"


def test_assessment_blocks_import_and_next_step_readiness_until_semantic_acceptance() -> None:
    assessment = _prototype()["assessment"]

    assert assessment["import_allowed"] is False
    assert assessment["semantic_ready_for_kg"] is False
    assert assessment["dimension_results"]["semantic_usefulness"]["blocks_import"] is True
    assert assessment["dimension_results"]["next_step_readiness"]["status"] == "blocked_pending_semantic_acceptance"
    assert assessment["packet_findings"] == [
        {
            "code": "pending_semantic_acceptance",
            "path": "/packets",
            "packet_id": "all",
            "blocks_import": True,
        }
    ]


def test_input_payload_is_not_mutated() -> None:
    payload = _s03_payload()
    original = deepcopy(payload)

    build_reviewer_packet_prototype(payload, s02_contract=_s02_contract())

    assert payload == original
