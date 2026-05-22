from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path

import pytest

from arxiv_archive.bounded_chunk_repair import (
    BoundedChunkRepairError,
    build_bounded_chunk_repair_contract,
    summarize_bounded_chunk_repair_contract,
)
from arxiv_archive.chunk_repair_contract import (
    expected_audit_from_contract,
    scan_forbidden_payload_keys,
    validate_chunk_repair_contract,
)

CONTRACT_FIXTURE = Path("tests/fixtures/chunk_repair_contract.json")
LOCATOR_FIXTURE = Path("tests/fixtures/bounded_locator_batch.json")
FORBIDDEN_KEYS = {
    "raw_text",
    "chunk_text",
    "paper_text",
    "claim_text",
    "embedding",
    "vector",
    "token",
    "api_key",
}


def _locator_batch() -> dict[str, object]:
    return json.loads(LOCATOR_FIXTURE.read_text(encoding="utf-8"))


def _contract() -> dict[str, object]:
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


def _build(max_target_count: int = 6) -> dict[str, object]:
    return build_bounded_chunk_repair_contract(_contract(), _locator_batch(), max_target_count=max_target_count)


def test_builds_deterministic_non_empty_targets_that_validate_against_contract() -> None:
    first = _build()
    second = _build()
    validation = validate_chunk_repair_contract(first, expected_audit=expected_audit_from_contract(first))

    assert validation.passed is True
    assert first == second
    assert [target["locator_id"] for target in first["repair_targets"]] == [
        "m021-synthetic-paper-1-claim-001",
        "m021-synthetic-paper-1-retrieval-001",
        "m021-synthetic-paper-1-method-001",
    ]
    assert first["diagnostics"]["target_count"] == 3
    assert validation.import_eligible_count == 0
    assert validation.production_write_count == 0
    assert validation.semantic_ready_count == 0


def test_selected_targets_cover_required_route_quality_and_repair_states() -> None:
    payload = _build()
    states = {target["repair_state"] for target in payload["repair_targets"]}
    route_quality = {target["route_quality_state"] for target in payload["repair_targets"]}
    routes = {target["route"] for target in payload["repair_targets"]}

    assert "ambiguous_span" in states
    assert "retrieval_only" in states
    assert "review_required" in states
    assert "broad_signal_many_matches" in route_quality
    assert any("overlapping_signal_window" in target["before_diagnostics"]["codes"] for target in payload["repair_targets"])
    assert "method_location" in routes


def test_output_metadata_is_redacted_pending_review_and_section_lineage_unresolved() -> None:
    payload = _build()
    rendered = json.dumps(payload, sort_keys=True)

    assert scan_forbidden_payload_keys(payload) == []
    assert not any(f'"{key}"' in rendered for key in FORBIDDEN_KEYS)
    for target in payload["repair_targets"]:
        assert target["review_status"] == "pending_review"
        assert target["reviewer"] is None
        assert target["section_path"] == ["unresolved_section_lineage"]
        assert target["section_lineage"] == {
            "status": "unresolved",
            "basis": "stable_locator_and_span_ids_only",
            "section_path_proven": False,
        }
        assert target["after_diagnostics"]["safe_to_import"] is False
        assert target["safety_boundaries"] == {
            "import_eligible": False,
            "promoted_to_fact": False,
            "trusted_kg_import_allowed": False,
            "production_write_attempted": False,
            "ladybugdb_written": False,
            "semantic_ready_for_kg": False,
            "raw_text_included": False,
            "chunk_text_included": False,
            "embeddings_included": False,
            "vectors_included": False,
            "secrets_included": False,
        }


def test_summary_reports_counts_and_zero_unsafe_safety_counters() -> None:
    summary = summarize_bounded_chunk_repair_contract(_build())

    assert summary["target_count"] == 3
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
    assert summary["unsafe_safety_counters"] == {
        "import_eligible_count": 0,
        "promoted_to_fact_count": 0,
        "production_write_count": 0,
        "semantic_ready_count": 0,
        "raw_text_included": False,
        "chunk_text_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "secrets_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }


def test_max_target_count_bounds_selection_but_keeps_stable_priority() -> None:
    payload = _build(max_target_count=2)

    assert [target["locator_id"] for target in payload["repair_targets"]] == [
        "m021-synthetic-paper-1-claim-001",
        "m021-synthetic-paper-1-retrieval-001",
    ]
    assert payload["diagnostics"]["target_count"] == 2


@pytest.mark.parametrize(
    ("mutator", "code", "path"),
    [
        (lambda batch: batch.__setitem__("schema_version", "wrong"), "locator_schema_mismatch", "/schema_version"),
        (
            lambda batch: batch["locators"][0].__setitem__("locator_id", "unknown-locator"),
            "unresolved_locator_id",
            "/locators/0/locator_id",
        ),
        (
            lambda batch: batch["locators"][0]["source_spans"][0].__setitem__("source_id", "unknown-source"),
            "unresolved_source_id",
            "/locators/0/source_spans/0/source_id",
        ),
        (
            lambda batch: batch["locators"][0]["source_spans"][0].__setitem__("span_id", "unknown-span"),
            "unresolved_span_id",
            "/locators/0/source_spans/0/span_id",
        ),
        (
            lambda batch: batch["safety_flags"].__setitem__("trusted_kg_import_allowed", True),
            "locator_validation_failed:safety_flag_true:trusted_kg_import_allowed",
            "/locators",
        ),
    ],
)
def test_fail_closed_for_malformed_or_unresolved_locator_inputs(mutator, code: str, path: str) -> None:
    batch = _locator_batch()
    mutator(batch)

    with pytest.raises(BoundedChunkRepairError) as exc_info:
        build_bounded_chunk_repair_contract(_contract(), batch)

    assert exc_info.value.code == code
    assert exc_info.value.path == path
    assert "synthetic" not in str(exc_info.value) or code.startswith("unresolved")


def test_forbidden_nested_metadata_key_is_rejected_without_leaking_value() -> None:
    batch = _locator_batch()
    batch["locators"][0]["metadata"] = {"api_key": "DO_NOT_LEAK"}

    with pytest.raises(BoundedChunkRepairError) as exc_info:
        build_bounded_chunk_repair_contract(_contract(), batch)

    assert exc_info.value.code == "secret_leakage"
    assert exc_info.value.path == "/locators/0/metadata/api_key"
    assert "DO_NOT_LEAK" not in str(exc_info.value)


def test_no_eligible_locators_fails_closed() -> None:
    batch = _locator_batch()
    for locator in batch["locators"]:
        locator["state"] = "unsupported"

    with pytest.raises(BoundedChunkRepairError) as exc_info:
        build_bounded_chunk_repair_contract(_contract(), batch)

    assert exc_info.value.code == "no_eligible_locators"
    assert exc_info.value.path == "/locators"


def test_input_contract_is_not_mutated() -> None:
    contract = _contract()
    original = deepcopy(contract)

    build_bounded_chunk_repair_contract(contract, _locator_batch())

    assert contract == original
