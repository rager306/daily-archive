from __future__ import annotations

import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CONTRACT = ROOT / "data/architecture-assessment/m198-gitnexus-impact-gates.json"

REQUIRED_GATE_IDS = {
    "queue_dependency_semantics",
    "readiness_report_generator",
    "no_write_governance_ratchets",
    "retired_graph_readiness_alias",
    "readiness_write_import_flags",
}

REQUIRED_NON_GOALS = {
    "production_graph_import",
    "schema_migration",
    "queue_dependency_semantic_change",
    "smoke_semantic_change",
    "rehearsal_semantic_change",
    "retired_graph_readiness_shim",
    "import_eligible_true",
}


def _contract() -> dict[str, Any]:
    return json.loads(CONTRACT.read_text(encoding="utf-8"))


def _gates(contract: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {gate["id"]: gate for gate in contract["gates"]}


def test_gitnexus_impact_gate_contract_shape() -> None:
    contract = _contract()

    assert contract["schema_version"] == "m198.gitnexus_impact_gates.v1"
    assert contract["milestone"] == "M198-t5wlml"
    assert contract["repo"] == "daily-archive"
    assert set(_gates(contract)) == REQUIRED_GATE_IDS
    assert REQUIRED_NON_GOALS.issubset(set(contract["required_non_goals"]))


def test_gitnexus_refresh_command_is_supported_form() -> None:
    contract = _contract()
    refresh = contract["index_refresh"]

    assert refresh["command"] == "gitnexus analyze"
    assert refresh["cwd"] == "/root/daily-archive"
    assert refresh["required_after_committing_new_symbols"] is True
    assert "gitnexus analyze --repo daily-archive" in refresh["unsupported_commands"]
    assert refresh["command"] not in refresh["unsupported_commands"]


def test_gitnexus_detect_changes_is_repo_scoped() -> None:
    contract = _contract()
    detect_changes = contract["detect_changes"]

    assert detect_changes["tool"] == "gitnexus_detect_changes"
    assert detect_changes["repo_required"] is True
    assert detect_changes["repo"] == "daily-archive"
    assert detect_changes["scope"] == "all"
    assert detect_changes["required_before_commit"] is True


def test_queue_dependency_gate_is_high_and_out_of_scope() -> None:
    gates = _gates(_contract())
    queue_gate = gates["queue_dependency_semantics"]

    assert queue_gate["expected_risk"] == "HIGH"
    assert queue_gate["status"] == "out_of_scope_for_m198_readiness_reporting"
    assert queue_gate["target"] == (
        "Method:src/research_graph/workflows/universal_kb/queue.py:"
        "UniversalKBQueue._dependencies_satisfied#1"
    )
    assert "run_universal_kb_no_write_rehearsal" in queue_gate["affected_processes"]
    assert "run_article" in queue_gate["affected_processes"]
    assert "warn_user_on_high_or_critical" in queue_gate["required_before_edit"]
    assert "queue_tests" in queue_gate["required_before_edit"]


def test_readiness_surface_gates_require_governance_tests() -> None:
    gates = _gates(_contract())

    report_gate = gates["readiness_report_generator"]
    assert report_gate["expected_risk"] == "LOW"
    assert "focused_report_tests" in report_gate["required_before_edit"]
    assert "no_write_governance_ratchets" in report_gate["required_before_edit"]

    flag_gate = gates["readiness_write_import_flags"]
    assert flag_gate["expected_risk"] == "BLOCKED_IF_TRUE"
    assert "m198_readiness_contract_tests" in flag_gate["required_before_edit"]
    assert "m198_no_write_governance_tests" in flag_gate["required_before_edit"]

    retired_alias_gate = gates["retired_graph_readiness_alias"]
    assert retired_alias_gate["expected_risk"] == "BLOCKED_IF_RESTORED"
    assert "m195_retired_alias_ratchet" in retired_alias_gate["required_before_edit"]
