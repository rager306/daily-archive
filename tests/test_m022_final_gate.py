from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path

import pytest

from research_graph.repair.bounded_chunk_repair import build_bounded_chunk_repair_contract
from research_graph.workflows.review_packet_prototype import (
    build_reviewer_packet_prototype,
    render_reviewer_packet_markdown,
)
from scripts.verify_m022_final_gate import (
    FINAL_GATE_SCHEMA_VERSION,
    build_final_gate,
    validate_final_gate,
    verify_files,
)
from scripts.verify_m022_final_gate import (
    main as final_gate_cli_main,
)

CONTRACT_FIXTURE = Path("tests/fixtures/chunk_repair_contract.json")
LOCATOR_FIXTURE = Path("tests/fixtures/bounded_locator_batch.json")
FORBIDDEN_RENDER_VALUES = ("DO_NOT_LEAK", "SECRET", "NEVER LEAK")


def _locator_batch() -> dict[str, object]:
    batch = json.loads(LOCATOR_FIXTURE.read_text(encoding="utf-8"))
    locators = batch["locators"]
    expanded = []
    for copy_index in range(2):
        for locator in locators:
            item = deepcopy(locator)
            item["locator_id"] = f"{locator['locator_id']}-copy-{copy_index + 1}"
            for span in item["source_spans"]:
                span["span_id"] = f"{span['span_id']}-copy-{copy_index + 1}"
            expanded.append(item)
    batch["locators"] = expanded
    batch["summary"] = {
        **batch["summary"],
        "locator_count": 6,
        "located_count": 6,
        "review_required_count": 2,
        "ambiguous_span_count": 2,
        "retrieval_only_count": 2,
    }
    return batch


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
    contract["safety_boundary"] = {
        "import_eligible": False,
        "ladybugdb_written": False,
        "production_write_attempted": False,
        "promoted_to_fact": False,
        "semantic_ready_for_kg": False,
        "source_payloads_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "secrets_included": False,
        "trusted_kg_import_allowed": False,
    }
    return contract


def _s03_payload() -> dict[str, object]:
    return build_bounded_chunk_repair_contract(_s02_contract(), _locator_batch(), max_target_count=6)


def _write_safe_inputs(tmp_path: Path) -> dict[str, Path]:
    repair = _s03_payload()
    s02 = _s02_contract()
    prototype = build_reviewer_packet_prototype(repair, s02_contract=s02)
    paths = {
        "packets_json": tmp_path / "reviewer-packet-prototype.json",
        "packets_markdown": tmp_path / "reviewer-packet-prototype.md",
        "assessment_json": tmp_path / "independent-packet-assessment.json",
        "assessment_markdown": tmp_path / "independent-packet-assessment.md",
        "repair_prototype": tmp_path / "bounded-repair-prototype.json",
        "s02_contract": tmp_path / "chunk-repair-contract.json",
        "final_gate": tmp_path / "m022-final-gate.json",
    }
    paths["packets_json"].write_text(json.dumps(prototype, indent=2), encoding="utf-8")
    paths["packets_markdown"].write_text(render_reviewer_packet_markdown(prototype), encoding="utf-8")
    paths["assessment_json"].write_text(json.dumps(prototype["assessment"], indent=2), encoding="utf-8")
    paths["assessment_markdown"].write_text("# Independent Packet Assessment\n\nVerdict: blocked_pending_semantic_acceptance\n\nImport allowed: false\n", encoding="utf-8")
    paths["repair_prototype"].write_text(json.dumps(repair, indent=2), encoding="utf-8")
    paths["s02_contract"].write_text(json.dumps(s02, indent=2), encoding="utf-8")
    return paths


def _verify(paths: dict[str, Path]) -> dict[str, object]:
    return verify_files(
        paths["packets_json"],
        paths["packets_markdown"],
        paths["assessment_json"],
        paths["assessment_markdown"],
        paths["repair_prototype"],
        paths["s02_contract"],
        paths.get("final_gate") if paths.get("final_gate", Path("missing")).exists() else None,
    )


def _mutate_json(path: Path, mutator) -> None:
    payload = json.loads(path.read_text(encoding="utf-8"))
    mutator(payload)
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def test_final_gate_accepts_safe_fixture_and_builds_deterministic_shape(tmp_path: Path) -> None:
    paths = _write_safe_inputs(tmp_path)

    final_gate = build_final_gate(
        packet_json_path=paths["packets_json"],
        packet_markdown_path=paths["packets_markdown"],
        assessment_json_path=paths["assessment_json"],
        assessment_markdown_path=paths["assessment_markdown"],
        repair_prototype_path=paths["repair_prototype"],
        s02_contract_path=paths["s02_contract"],
    )
    paths["final_gate"].write_text(json.dumps(final_gate, indent=2, sort_keys=True), encoding="utf-8")
    summary = _verify(paths)

    assert summary["passed"] is True
    assert final_gate["schema_version"] == FINAL_GATE_SCHEMA_VERSION
    assert final_gate["packet_summary"]["packet_count"] == 6
    assert final_gate["packet_summary"]["review_status_counts"] == {"pending_review": 6}
    assert final_gate["assessment_verdict"] == "blocked_pending_semantic_acceptance"
    assert sorted(final_gate["requirement_outcomes"]) == ["R024", "R027", "R028", "R029"]
    assert final_gate["blocked_boundaries"]["kg_import_blocked"] is True
    assert final_gate["blocked_boundaries"]["import_allowed"] is False
    assert final_gate["blocked_boundaries"]["semantic_ready_for_kg"] is False
    assert final_gate["final_recommendation"]["action"] == "human_semantic_review_or_bounded_repair_only"
    assert final_gate["final_recommendation"]["kg_import_allowed"] is False
    assert not validate_final_gate(final_gate, expected_packet_count=6)


def test_cli_writes_final_gate_and_emits_redacted_summary(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    paths = _write_safe_inputs(tmp_path)

    code = final_gate_cli_main(
        [
            "--packets-json",
            str(paths["packets_json"]),
            "--packets-markdown",
            str(paths["packets_markdown"]),
            "--assessment-json",
            str(paths["assessment_json"]),
            "--assessment-markdown",
            str(paths["assessment_markdown"]),
            "--repair-prototype",
            str(paths["repair_prototype"]),
            "--s02-contract",
            str(paths["s02_contract"]),
            "--write-final-gate",
            str(paths["final_gate"]),
        ]
    )
    out = capsys.readouterr()

    assert code == 0
    assert paths["final_gate"].exists()
    assert "M022 final gate verified:" in out.out
    assert "schema=m022-final-gate.v1" in out.out
    assert "packets=6" in out.out
    assert "blocked_pending_semantic_acceptance" in out.out
    assert "DO_NOT_LEAK" not in out.out
    assert all(value not in out.out for value in FORBIDDEN_RENDER_VALUES)


@pytest.mark.parametrize(
    ("mutator", "code", "path"),
    [
        (lambda paths: paths["packets_json"].unlink(), "file not found", ""),
        (lambda paths: paths["packets_json"].write_text('{"broken": ', encoding="utf-8"), "JSON is malformed", ""),
        (
            lambda paths: _mutate_json(paths["packets_json"], lambda payload: payload["packets"].pop()),
            "final_gate_packet_count_not_six",
            "/packets",
        ),
        (
            lambda paths: _mutate_json(paths["packets_json"], lambda payload: payload["packets"][0].__setitem__("review_status", "accepted")),
            "packet_not_pending_review",
            "/packets/0/review_status",
        ),
        (
            lambda paths: _mutate_json(paths["packets_json"], lambda payload: payload["packets"][0].__setitem__("importable", True)),
            "packet_importable",
            "/packets/0/importable",
        ),
        (
            lambda paths: _mutate_json(paths["packets_json"], lambda payload: payload["packets"][0].__setitem__("semantic_ready_for_kg", True)),
            "packet_semantic_ready",
            "/packets/0/semantic_ready_for_kg",
        ),
        (
            lambda paths: _mutate_json(paths["packets_json"], lambda payload: payload["packets"][0].__setitem__("api_key", "DO_NOT_LEAK")),
            "secret_leakage",
            "/packets/0/api_key",
        ),
        (
            lambda paths: _mutate_json(paths["assessment_json"], lambda payload: payload.__setitem__("verdict", "import_ready")),
            "assessment_verdict_drift",
            "/verdict",
        ),
        (
            lambda paths: _mutate_json(paths["assessment_json"], lambda payload: payload.__setitem__("import_allowed", True)),
            "assessment_import_allowed",
            "/import_allowed",
        ),
        (
            lambda paths: _mutate_json(paths["assessment_json"], lambda payload: payload.__setitem__("semantic_ready_for_kg", True)),
            "assessment_semantic_ready",
            "/semantic_ready_for_kg",
        ),
        (
            lambda paths: _mutate_json(paths["assessment_json"], lambda payload: payload["unsafe_counters"].__setitem__("semantic_ready_count", 1)),
            "unsafe_counter_nonzero",
            "/unsafe_counters/semantic_ready_count",
        ),
        (
            lambda paths: _mutate_json(paths["assessment_json"], lambda payload: payload["unsafe_counters"].__setitem__("vectors_included", True)),
            "unsafe_counter_true",
            "/unsafe_counters/vectors_included",
        ),
        (
            lambda paths: _mutate_json(paths["repair_prototype"], lambda payload: payload["diagnostics"].__setitem__("production_import_attempted", True)),
            "repair_diagnostic_unsafe_boolean",
            "/diagnostics/production_import_attempted",
        ),
        (
            lambda paths: _mutate_json(paths["s02_contract"], lambda payload: payload["safety_boundary"].__setitem__("trusted_kg_import_allowed", True)),
            "s02_safety_boundary_unsafe",
            "/safety_boundary/trusted_kg_import_allowed",
        ),
    ],
)
def test_source_artifact_failures_are_redacted_and_fail_closed(tmp_path: Path, mutator, code: str, path: str, capsys: pytest.CaptureFixture[str]) -> None:
    paths = _write_safe_inputs(tmp_path)
    mutator(paths)

    cli_code = final_gate_cli_main(
        [
            "--packets-json",
            str(paths["packets_json"]),
            "--packets-markdown",
            str(paths["packets_markdown"]),
            "--assessment-json",
            str(paths["assessment_json"]),
            "--assessment-markdown",
            str(paths["assessment_markdown"]),
            "--repair-prototype",
            str(paths["repair_prototype"]),
            "--s02-contract",
            str(paths["s02_contract"]),
        ]
    )
    out = capsys.readouterr()

    assert cli_code == 2
    assert code in out.err
    if path:
        assert path in out.err
    assert all(value not in out.err for value in FORBIDDEN_RENDER_VALUES)


@pytest.mark.parametrize(
    ("mutator", "code", "path"),
    [
        (lambda gate: gate.__setitem__("schema_version", "wrong"), "final_gate_schema_mismatch", "/schema_version"),
        (lambda gate: gate["packet_summary"].__setitem__("packet_count", 5), "final_gate_packet_count_mismatch", "/packet_summary/packet_count"),
        (lambda gate: gate["packet_summary"].__setitem__("review_status_counts", {"accepted": 1}), "final_gate_review_status_not_pending", "/packet_summary/review_status_counts"),
        (lambda gate: gate["blocked_boundaries"].__setitem__("kg_import_blocked", False), "final_gate_boundary_polarity_drift", "/blocked_boundaries/kg_import_blocked"),
        (lambda gate: gate["blocked_boundaries"].__setitem__("import_allowed", True), "final_gate_boundary_polarity_drift", "/blocked_boundaries/import_allowed"),
        (lambda gate: gate["final_recommendation"].__setitem__("kg_import_allowed", True), "final_gate_recommendation_unsafe_claim", "/final_recommendation/kg_import_allowed"),
        (lambda gate: gate["requirement_outcomes"]["R024"].__setitem__("import_allowed_claimed", True), "final_gate_requirement_unsafe_claim", "/requirement_outcomes/R024/import_allowed_claimed"),
        (lambda gate: gate.__setitem__("api_key", "DO_NOT_LEAK"), "secret_leakage", "/api_key"),
    ],
)
def test_final_gate_json_validation_rejects_drift_and_unsafe_claims(tmp_path: Path, mutator, code: str, path: str) -> None:
    paths = _write_safe_inputs(tmp_path)
    final_gate = build_final_gate(
        packet_json_path=paths["packets_json"],
        packet_markdown_path=paths["packets_markdown"],
        assessment_json_path=paths["assessment_json"],
        assessment_markdown_path=paths["assessment_markdown"],
        repair_prototype_path=paths["repair_prototype"],
        s02_contract_path=paths["s02_contract"],
    )
    mutated = deepcopy(final_gate)
    mutator(mutated)

    findings = validate_final_gate(mutated, expected_packet_count=6)

    assert {finding.code for finding in findings} >= {code}
    assert path in {finding.path for finding in findings}
    assert "DO_NOT_LEAK" not in json.dumps([finding.__dict__ for finding in findings], sort_keys=True)


def test_markdown_code_fence_and_forbidden_marker_are_rejected(tmp_path: Path) -> None:
    paths = _write_safe_inputs(tmp_path)
    paths["packets_markdown"].write_text(
        paths["packets_markdown"].read_text(encoding="utf-8") + "\n```raw_text leak marker```\n",
        encoding="utf-8",
    )

    summary = _verify(paths)

    assert summary["passed"] is False
    codes = {finding["code"] for finding in summary["findings"]}
    assert "markdown_code_fence" in codes
    assert "markdown_forbidden_marker" in codes
