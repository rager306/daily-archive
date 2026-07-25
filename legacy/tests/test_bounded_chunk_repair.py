from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path

import pytest

from research_graph.infrastructure.repair.bounded_chunk_repair import (
    BoundedChunkRepairError,
    build_bounded_chunk_repair_contract,
    render_bounded_chunk_repair_markdown,
    summarize_bounded_chunk_repair_contract,
)
from research_graph.infrastructure.repair.chunk_repair_contract import (
    MARKDOWN_FORBIDDEN_PATTERNS,
    expected_audit_from_contract,
    scan_forbidden_payload_keys,
    validate_chunk_repair_contract,
)
from scripts import render_bounded_repair_prototype, verify_bounded_repair_prototype

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
        # pyrefly: ignore [not-iterable]
        "locator_ids": [locator["locator_id"] for locator in locators],  # ty:ignore[not-iterable]
        # pyrefly: ignore [not-iterable]
        "span_ids": [locator["source_spans"][0]["span_id"] for locator in locators],  # ty:ignore[not-iterable]
    }
    return contract


def _build(max_target_count: int = 6) -> dict[str, object]:
    return build_bounded_chunk_repair_contract(
        _contract(), _locator_batch(), max_target_count=max_target_count
    )


def test_builds_deterministic_non_empty_targets_that_validate_against_contract() -> None:
    first = _build()
    second = _build()
    validation = validate_chunk_repair_contract(
        first, expected_audit=expected_audit_from_contract(first)
    )

    assert validation.passed is True
    assert first == second
    # pyrefly: ignore [not-iterable]
    assert [target["locator_id"] for target in first["repair_targets"]] == [  # ty:ignore[not-iterable]
        "m021-synthetic-paper-1-claim-001",
        "m021-synthetic-paper-1-retrieval-001",
        "m021-synthetic-paper-1-method-001",
    ]
    assert first["diagnostics"]["target_count"] == 3  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    assert validation.import_eligible_count == 0
    assert validation.production_write_count == 0
    assert validation.semantic_ready_count == 0


def test_selected_targets_cover_required_route_quality_and_repair_states() -> None:
    payload = _build()
    # pyrefly: ignore [not-iterable]
    states = {target["repair_state"] for target in payload["repair_targets"]}  # ty:ignore[not-iterable]
    # pyrefly: ignore [not-iterable]
    route_quality = {target["route_quality_state"] for target in payload["repair_targets"]}  # ty:ignore[not-iterable]
    # pyrefly: ignore [not-iterable]
    routes = {target["route"] for target in payload["repair_targets"]}  # ty:ignore[not-iterable]

    assert "ambiguous_span" in states
    assert "retrieval_only" in states
    assert "review_required" in states
    assert "broad_signal_many_matches" in route_quality
    assert any(
        "overlapping_signal_window" in target["before_diagnostics"]["codes"]
        # pyrefly: ignore [not-iterable]
        for target in payload["repair_targets"]  # ty:ignore[not-iterable]
    )
    assert "method_location" in routes


def test_output_metadata_is_redacted_pending_review_and_section_lineage_unresolved() -> None:
    payload = _build()
    rendered = json.dumps(payload, sort_keys=True)

    assert scan_forbidden_payload_keys(payload) == []
    assert not any(f'"{key}"' in rendered for key in FORBIDDEN_KEYS)
    # pyrefly: ignore [not-iterable]
    for target in payload["repair_targets"]:  # ty:ignore[not-iterable]
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

    # pyrefly: ignore [not-iterable]
    assert [target["locator_id"] for target in payload["repair_targets"]] == [  # ty:ignore[not-iterable]
        "m021-synthetic-paper-1-claim-001",
        "m021-synthetic-paper-1-retrieval-001",
    ]
    assert payload["diagnostics"]["target_count"] == 2  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]


@pytest.mark.parametrize(
    ("mutator", "code", "path"),
    [
        (
            lambda batch: batch.__setitem__("schema_version", "wrong"),
            "locator_schema_mismatch",
            "/schema_version",
        ),
        (
            lambda batch: batch["locators"][0].__setitem__("locator_id", "unknown-locator"),
            "unresolved_locator_id",
            "/locators/0/locator_id",
        ),
        (
            lambda batch: batch["locators"][0]["source_spans"][0].__setitem__(
                "source_id", "unknown-source"
            ),
            "unresolved_source_id",
            "/locators/0/source_spans/0/source_id",
        ),
        (
            lambda batch: batch["locators"][0]["source_spans"][0].__setitem__(
                "span_id", "unknown-span"
            ),
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
def test_fail_closed_for_malformed_or_unresolved_locator_inputs(
    mutator, code: str, path: str
) -> None:
    batch = _locator_batch()
    mutator(batch)

    with pytest.raises(BoundedChunkRepairError) as exc_info:
        build_bounded_chunk_repair_contract(_contract(), batch)

    assert exc_info.value.code == code
    assert exc_info.value.path == path
    assert "synthetic" not in str(exc_info.value) or code.startswith("unresolved")


def test_forbidden_nested_metadata_key_is_rejected_without_leaking_value() -> None:
    batch = _locator_batch()
    batch["locators"][0]["metadata"] = {"api_key": "DO_NOT_LEAK"}  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]

    with pytest.raises(BoundedChunkRepairError) as exc_info:
        build_bounded_chunk_repair_contract(_contract(), batch)

    assert exc_info.value.code == "secret_leakage"
    assert exc_info.value.path == "/locators/0/metadata/api_key"
    assert "DO_NOT_LEAK" not in str(exc_info.value)


def test_no_eligible_locators_fails_closed() -> None:
    batch = _locator_batch()
    # pyrefly: ignore [not-iterable]
    for locator in batch["locators"]:  # ty:ignore[not-iterable]
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


def test_markdown_renderer_reports_counts_classifications_and_redacted_safety() -> None:
    markdown = render_bounded_chunk_repair_markdown(_build())

    assert "# S03 Bounded Repair Prototype" in markdown
    assert "Selected target count: 3" in markdown
    assert "Repair State Counts" in markdown
    assert "Route Quality Counts" in markdown
    assert "Before diagnostic codes:" in markdown
    assert "After diagnostic codes:" in markdown
    assert "Classification: explicit needs-review or non-repairable" in markdown
    assert "Source payload included: false" in markdown
    assert all(pattern not in markdown for pattern in MARKDOWN_FORBIDDEN_PATTERNS)


def test_render_cli_writes_only_after_json_and_markdown_validation(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    renderer = render_bounded_repair_prototype
    contract_path = tmp_path / "contract.json"
    contract_path.write_text(json.dumps(_contract()), encoding="utf-8")
    json_output = tmp_path / "prototype.json"
    markdown_output = tmp_path / "prototype.md"

    summary = renderer.render_prototype_files(
        contract_path,
        LOCATOR_FIXTURE,
        json_output,
        markdown_output,
        max_target_count=6,
    )

    assert summary["target_count"] == 3
    assert json_output.exists()
    assert markdown_output.exists()

    bad_json_output = tmp_path / "bad.json"
    bad_markdown_output = tmp_path / "bad.md"
    monkeypatch.setattr(
        renderer, "render_bounded_chunk_repair_markdown", lambda payload: "```unsafe```"
    )

    with pytest.raises(renderer.BoundedRepairPrototypeRenderError):
        renderer.render_prototype_files(
            contract_path,
            LOCATOR_FIXTURE,
            bad_json_output,
            bad_markdown_output,
            max_target_count=6,
        )

    assert not bad_json_output.exists()
    assert not bad_markdown_output.exists()


def test_verify_cli_accepts_generated_artifacts_and_rejects_unsafe_or_unresolved_targets(
    tmp_path: Path,
) -> None:
    renderer = render_bounded_repair_prototype
    verifier = verify_bounded_repair_prototype
    contract_path = tmp_path / "contract.json"
    contract_path.write_text(json.dumps(_contract()), encoding="utf-8")
    json_output = tmp_path / "prototype.json"
    markdown_output = tmp_path / "prototype.md"
    renderer.render_prototype_files(
        contract_path,
        LOCATOR_FIXTURE,
        json_output,
        markdown_output,
        max_target_count=6,
    )

    summary = verifier.verify_files(json_output, markdown_output, contract_path)

    assert summary["passed"] is True
    assert summary["target_count"] == 3
    assert summary["unsafe_counters_zero"] is True

    unsafe_payload = json.loads(json_output.read_text(encoding="utf-8"))
    unsafe_payload["repair_targets"][0]["safety_boundaries"]["import_eligible"] = True
    unsafe_payload["diagnostics"]["import_eligible_count"] = 1
    unsafe_payload["repair_targets"][0]["locator_id"] = "not-in-s02"
    unsafe_payload_path = tmp_path / "unsafe.json"
    unsafe_payload_path.write_text(json.dumps(unsafe_payload), encoding="utf-8")
    unsafe_markdown_path = tmp_path / "unsafe.md"
    unsafe_markdown_path.write_text(
        markdown_output.read_text(encoding="utf-8") + "\n```\n", encoding="utf-8"
    )

    unsafe_summary = verifier.verify_files(unsafe_payload_path, unsafe_markdown_path, contract_path)
    codes = {finding["code"] for finding in unsafe_summary["findings"]}

    assert unsafe_summary["passed"] is False
    assert "unsafe_target_safety_flag" in codes
    assert "locator_id_not_in_s02_stable_ids" in codes
    assert "markdown_validation_failed:markdown_forbidden_pattern" in codes


def test_render_cli_rejects_malformed_locator_batch_before_write(tmp_path: Path) -> None:
    renderer = render_bounded_repair_prototype
    contract_path = tmp_path / "contract.json"
    contract_path.write_text(json.dumps(_contract()), encoding="utf-8")
    malformed = tmp_path / "malformed.json"
    malformed.write_text("{not-json", encoding="utf-8")
    json_output = tmp_path / "prototype.json"
    markdown_output = tmp_path / "prototype.md"

    with pytest.raises(renderer.BoundedRepairPrototypeRenderError):
        renderer.render_prototype_files(
            contract_path,
            malformed,
            json_output,
            markdown_output,
            max_target_count=6,
        )

    assert not json_output.exists()
    assert not markdown_output.exists()
