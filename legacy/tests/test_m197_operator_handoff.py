from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HANDOFF = ROOT / "data/architecture-assessment/m197-operator-handoff.md"


def _handoff_text() -> str:
    return HANDOFF.read_text(encoding="utf-8")


def test_m197_operator_handoff_exists_and_names_reader_action() -> None:
    text = _handoff_text()

    assert "Reader: a future operator or agent" in text
    assert "run the reactive dry-run pilot" in text
    assert "inspect its JSONL lifecycle events" in text
    assert "avoid claiming graph/import readiness" in text


def test_m197_operator_handoff_contains_command_and_expected_events() -> None:
    text = _handoff_text()

    assert "uv run python scripts/run_m197_reactive_dry_run.py" in text
    assert "--events artifacts/m197-reactive-dry-run/events.jsonl" in text
    assert "m197_reactive_events=4" in text
    for event_type in ("stage.started", "stage.completed"):
        assert event_type in text
    for stage_id in ("dry_run.schema_gate", "dry_run.projection_safety"):
        assert stage_id in text


def test_m197_operator_handoff_preserves_safety_invariants() -> None:
    text = _handoff_text()

    for invariant in (
        "graph_writes_allowed=false",
        "schema_migration_allowed=false",
        "import_eligible=false",
    ):
        assert invariant in text
    for forbidden in (
        "api_key",
        "secret_value",
        "embedding_payload",
        "vector_payload",
        "raw_prompt_payload",
    ):
        assert forbidden in text
    assert "import_eligible=true" in text
    assert "not authorize" in text


def test_m197_operator_handoff_links_evidence_and_non_goals() -> None:
    text = _handoff_text()

    for evidence in (
        "tests/test_m197_reactive_dry_run.py",
        "tests/test_m197_queue_compatibility.py",
        "tests/test_m197_realistic_no_write_rehearsal.py",
        "tests/test_m197_governance_ratchets.py",
        "m197-s09-scope-verification.md",
        "m197-s10-scope-verification.md",
        "m197-s11-scope-verification.md",
        "m197-s12-scope-verification.md",
    ):
        assert evidence in text
    for non_goal in (
        "production graph import",
        "schema migration",
        "queue dependency semantic changes",
        "direct graph backend writes",
    ):
        assert non_goal in text


def test_m197_operator_handoff_names_final_sweep_command() -> None:
    text = _handoff_text()

    assert "Run the final compatibility sweep" in text
    for test_path in (
        "tests/test_m197_operator_handoff.py",
        "tests/test_m197_governance_ratchets.py",
        "tests/test_m197_realistic_no_write_rehearsal.py",
        "tests/test_m197_queue_compatibility.py",
        "tests/test_m196_governance_ratchets.py",
        "tests/test_m195_governance_ratchets.py",
    ):
        assert test_path in text
