from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "data/architecture-assessment/m197-reactive-event-contract.json"


def _contract() -> dict:
    return json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))


def test_reactive_event_contract_has_required_identity_and_state_fields() -> None:
    contract = _contract()

    assert contract["schema_version"] == "m197.reactive_event.v1"
    assert contract["scope"] == "no_write_async_reactive_pilot"

    required = set(contract["required_fields"])
    assert {
        "schema_version",
        "event_type",
        "job_id",
        "stage_id",
        "correlation_id",
        "phase",
        "status",
        "attempt",
        "timestamp",
        "artifact_refs",
        "diagnostics",
    } <= required


def test_reactive_event_contract_models_job_stage_and_failure_events() -> None:
    event_types = set(_contract()["event_types"])

    assert {
        "job.created",
        "job.claimed",
        "job.heartbeat",
        "job.retry_scheduled",
        "job.completed",
        "job.failed_retryable",
        "job.failed_terminal",
        "job.cancelled",
        "stage.started",
        "stage.artifact_registered",
        "stage.completed",
        "stage.failed_retryable",
        "stage.failed_terminal",
        "stage.timeout",
        "stage.cancelled",
    } <= event_types


def test_reactive_event_contract_keeps_graph_and_import_readiness_blocked() -> None:
    contract = _contract()
    blocked = contract["blocked_readiness"]

    assert blocked["graph_writes_allowed"] is False
    assert blocked["schema_migration_allowed"] is False
    assert blocked["import_eligible"] is False
    assert blocked["production_graph_import"] is False
    assert blocked["ladybugdb_write"] is False
    assert blocked["falkordb_write"] is False

    required = set(contract["required_fields"])
    assert {"graph_writes_allowed", "schema_migration_allowed", "import_eligible"} <= required
    assert any("import_eligible must remain false" in rule for rule in contract["ordering_rules"])


def test_reactive_event_contract_payload_terms_are_payload_shaped() -> None:
    contract = _contract()
    forbidden = set(contract["forbidden_payload_terms"])
    allowed = set(contract["allowed_payload_metadata"])

    assert {
        "raw_prompt_payload",
        "source_text_payload",
        "paper_text_payload",
        "chunk_text_payload",
        "embedding_payload",
        "vector_payload",
        "api_key",
        "secret_value",
    } <= forbidden
    assert "raw_prompt" not in forbidden
    assert {
        "payload_kind",
        "payload_sha256",
        "payload_size_bytes",
        "payload_redaction_status",
        "source_ref",
        "artifact_ref",
    } <= allowed


def test_reactive_event_contract_terminal_and_retry_statuses_are_disjoint() -> None:
    contract = _contract()
    terminal = set(contract["terminal_statuses"])
    retryable = set(contract["retryable_statuses"])

    assert {"completed", "failed_terminal", "cancelled", "timeout"} <= terminal
    assert {"failed_retryable", "retry_scheduled"} <= retryable
    assert terminal.isdisjoint(retryable)
