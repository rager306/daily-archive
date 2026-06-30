from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "data/architecture-assessment/m198-readiness-evidence-contract.json"
M197_CONTRACT_PATH = ROOT / "data/architecture-assessment/m197-reactive-event-contract.json"


def _contract() -> dict:
    return json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))


def _m197_contract() -> dict:
    return json.loads(M197_CONTRACT_PATH.read_text(encoding="utf-8"))


def test_m198_readiness_contract_has_required_identity_state_and_refs() -> None:
    contract = _contract()

    assert contract["schema_version"] == "m198.readiness_evidence.v1"
    assert contract["scope"] == "reactive_readiness_preconditions_no_write"
    required = set(contract["required_fields"])
    assert {
        "schema_version",
        "evidence_id",
        "source_kind",
        "correlation_id",
        "status",
        "drift_class",
        "timestamp",
        "evidence_refs",
        "diagnostics",
        "non_goals",
    } <= required


def test_m198_readiness_contract_models_expected_source_kinds_and_drift_classes() -> None:
    contract = _contract()

    assert {
        "reactive_dry_run",
        "sync_no_write_rehearsal",
        "smoke_boundary",
        "graph_readiness_validate_only",
        "disabled_backend",
        "governance_ratchet",
    } <= set(contract["source_kinds"])
    assert {"expected", "warning", "blocker", "not_applicable"} <= set(
        contract["drift_classes"]
    )
    assert {"pass", "warning", "blocker"} <= set(contract["statuses"])


def test_m198_readiness_contract_keeps_write_schema_and_import_blocked() -> None:
    contract = _contract()
    blocked = contract["blocked_readiness"]

    assert blocked["graph_writes_allowed"] is False
    assert blocked["schema_migration_allowed"] is False
    assert blocked["import_eligible"] is False
    assert blocked["production_graph_import"] is False
    assert blocked["schema_migration"] is False
    assert blocked["queue_dependency_semantic_change"] is False
    assert blocked["smoke_semantic_change"] is False
    assert blocked["rehearsal_semantic_change"] is False
    assert blocked["retired_graph_readiness_shim"] is False
    assert {
        "graph_writes_allowed",
        "schema_migration_allowed",
        "import_eligible",
    } <= set(contract["required_fields"])


def test_m198_blocked_transitions_name_future_work_not_current_readiness() -> None:
    blocked = set(_contract()["blocked_transitions"])

    assert {
        "production_graph_import",
        "schema_migration",
        "queue_dependency_semantic_change",
        "smoke_semantic_change",
        "rehearsal_semantic_change",
        "retired_graph_readiness_shim",
        "import_eligible_true",
    } <= blocked


def test_m198_payload_safety_terms_match_m197_shape() -> None:
    contract = _contract()
    m197 = _m197_contract()

    assert set(m197["forbidden_payload_terms"]) <= set(contract["forbidden_payload_terms"])
    assert "raw_prompt" not in set(contract["forbidden_payload_terms"])
    assert {
        "payload_kind",
        "payload_sha256",
        "payload_size_bytes",
        "payload_redaction_status",
        "source_ref",
        "artifact_ref",
        "checksum_sha256",
    } <= set(contract["allowed_payload_metadata"])


def test_m198_source_kind_expectations_preserve_no_write_boundaries() -> None:
    expectations = _contract()["source_kind_expectations"]

    assert "does not create queue.sqlite" in expectations["reactive_dry_run"]
    assert "must not emit standalone queue_events.json" in expectations[
        "sync_no_write_rehearsal"
    ]
    assert "no smoke source semantic edits" in expectations["smoke_boundary"]
    assert "retired shim remains blocked" in expectations["graph_readiness_validate_only"]
    assert "no production graph writes" in expectations["disabled_backend"]
    assert "requires explicit non_goals" in expectations["governance_ratchet"]
