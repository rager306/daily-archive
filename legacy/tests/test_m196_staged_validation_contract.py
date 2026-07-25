from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "data/architecture-assessment/m196-staged-validation-contract.json"
REQUIRED_STAGE_FIELDS = {
    "id",
    "purpose",
    "command",
    "expected_outputs",
    "acceptance_criteria",
    "max_runtime_seconds",
    "requires_network",
    "graph_writes_allowed",
    "import_eligible_allowed",
}
RETIRED_GRAPH_READINESS_MODULE = ".".join(("arxiv_archive", "graph_readiness_review"))
FORBIDDEN_COMMAND_TERMS = (
    RETIRED_GRAPH_READINESS_MODULE,
    "ladybugdb write",
    "falkordb write",
    "import_eligible=true",
)


def _contract() -> dict:
    return json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))


def test_staged_validation_contract_has_required_top_level_guards() -> None:
    contract = _contract()

    assert contract["schema_version"] == "m196-staged-validation-contract.v1"
    assert contract["milestone_id"] == "M196-0nrede"
    assert contract["graph_writes_allowed"] is False
    assert contract["import_eligible_allowed"] is False
    assert contract["requires_network"] is False
    assert "production_graph_import" in contract["blocked_readiness"]
    assert RETIRED_GRAPH_READINESS_MODULE in contract["blocked_readiness"]


def test_staged_validation_contract_defines_bounded_stages() -> None:
    contract = _contract()
    stages = contract["stages"]

    assert [stage["id"] for stage in stages] == ["contract", "smoke", "compatibility", "no_leak"]
    for stage in stages:
        assert REQUIRED_STAGE_FIELDS <= stage.keys()
        assert stage["max_runtime_seconds"] <= 240
        assert stage["requires_network"] is False
        assert stage["graph_writes_allowed"] is False
        assert stage["import_eligible_allowed"] is False
        assert stage["expected_outputs"]
        assert stage["acceptance_criteria"]
        assert stage["command"].startswith("uv run ")


def test_staged_validation_contract_commands_do_not_restore_blocked_paths() -> None:
    serialized_commands = "\n".join(stage["command"] for stage in _contract()["stages"]).lower()

    for term in FORBIDDEN_COMMAND_TERMS:
        assert term not in serialized_commands


def test_staged_validation_contract_is_metadata_only() -> None:
    contract = _contract()
    serialized = json.dumps(contract, sort_keys=True).lower()

    for term in contract["forbidden_payload_terms"]:
        assert term not in serialized.replace(term, "", 1)
