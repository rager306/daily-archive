from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "data/architecture-assessment/m197-reactive-event-contract.json"
DRY_RUN_SCRIPT = ROOT / "scripts/run_m197_reactive_dry_run.py"


def _contract() -> dict:
    return json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))


def test_m197_reactive_governance_ratchet_surfaces_exist() -> None:
    expected = [
        ROOT / "tests/test_m197_reactive_event_contract.py",
        ROOT / "tests/test_m197_reactive_runner.py",
        ROOT / "tests/test_m197_reactive_dry_run.py",
        ROOT / "tests/test_m197_queue_compatibility.py",
        ROOT / "tests/test_m197_realistic_no_write_rehearsal.py",
        ROOT / "data/architecture-assessment/m197-s09-scope-verification.md",
        ROOT / "data/architecture-assessment/m197-s10-scope-verification.md",
        ROOT / "data/architecture-assessment/m197-s11-scope-verification.md",
    ]

    for path in expected:
        assert path.exists(), path


def test_m197_reactive_contract_keeps_write_import_and_schema_readiness_blocked() -> None:
    contract = _contract()

    assert contract["schema_version"] == "m197.reactive_event.v1"
    for field in (
        "graph_writes_allowed",
        "schema_migration_allowed",
        "import_eligible",
    ):
        assert field in contract["required_fields"]
    assert "import_eligible=true" not in json.dumps(contract).lower()
    assert "graph_writes_allowed=true" not in json.dumps(contract).lower()
    assert "schema_migration_allowed=true" not in json.dumps(contract).lower()


def test_m197_dry_run_script_keeps_safe_default_command_shape() -> None:
    text = DRY_RUN_SCRIPT.read_text(encoding="utf-8")

    assert "DEFAULT_EVENTS_PATH" in text
    assert "artifacts/m197-reactive-dry-run/events.jsonl" in text
    assert "run_reactive_stages_bounded" in text
    assert "graph backend" in text.lower()
    assert "schema migrations" in text.lower()
    assert any(phrase in text.lower() for phrase in ("import eligible", "imports eligible"))
    forbidden_imports = (
        "workflows.universal_kb.queue",
        "workflows.universal_kb.rehearsal",
        "workflows.universal_kb.smoke_runner",
        "workflows.universal_kb.smoke",
    )
    for import_path in forbidden_imports:
        assert import_path not in text


def test_m197_scope_artifacts_preserve_queue_rehearsal_smoke_boundaries() -> None:
    artifacts = [
        ROOT / "data/architecture-assessment/m197-s09-scope-verification.md",
        ROOT / "data/architecture-assessment/m197-s10-scope-verification.md",
        ROOT / "data/architecture-assessment/m197-s11-scope-verification.md",
    ]

    for path in artifacts:
        text = path.read_text(encoding="utf-8").lower()
        assert "queue.py" in text
        assert "rehearsal.py" in text
        assert "smoke_runner.py" in text
        assert "smoke.py" in text
        assert "not edited" in text
        assert "graph" in text
        assert "import" in text


def test_m197_forbidden_payload_terms_remain_payload_shaped() -> None:
    forbidden_terms = set(_contract()["forbidden_payload_terms"])

    assert "api_key" in forbidden_terms
    assert "secret_value" in forbidden_terms
    assert "embedding_payload" in forbidden_terms
    assert "vector_payload" in forbidden_terms
    assert "raw_prompt_payload" in forbidden_terms
    assert "raw_prompt" not in forbidden_terms
