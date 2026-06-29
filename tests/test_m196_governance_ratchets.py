from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "data/architecture-assessment/m196-staged-validation-contract.json"
RETIRED_MODULE = ".".join(("arxiv_archive", "graph_readiness_review"))


def _contract() -> dict:
    return json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))


def test_m196_contract_and_observability_ratchets_exist() -> None:
    expected = [
        ROOT / "tests/test_m196_staged_validation_contract.py",
        ROOT / "tests/test_m196_queue_resilience.py",
        ROOT / "tests/test_m196_run_artifact_observability.py",
        ROOT / "tests/test_m195_governance_ratchets.py",
    ]

    for path in expected:
        assert path.exists(), path


def test_m196_staged_contract_keeps_write_and_import_readiness_blocked() -> None:
    contract = _contract()

    assert contract["graph_writes_allowed"] is False
    assert contract["import_eligible_allowed"] is False
    assert contract["requires_network"] is False
    assert RETIRED_MODULE in contract["blocked_readiness"]
    for stage in contract["stages"]:
        assert stage["graph_writes_allowed"] is False
        assert stage["import_eligible_allowed"] is False
        assert stage["requires_network"] is False
        assert "import_eligible=true" not in stage["command"].lower()
        assert RETIRED_MODULE not in stage["command"]


def test_m196_scope_artifacts_keep_blocked_readiness_disclaimers() -> None:
    artifacts = [
        ROOT / "data/architecture-assessment/m196-s02-scope-verification.md",
        ROOT / "data/architecture-assessment/m196-s03-scope-verification.md",
        ROOT / "data/architecture-assessment/m196-s04-scope-verification.md",
    ]

    for path in artifacts:
        text = path.read_text(encoding="utf-8").lower()
        assert any(phrase in text for phrase in ("does not enable", "does not run", "blocked"))
        assert "graph" in text
        assert "import" in text


def test_m196_forbidden_payload_terms_remain_payload_shaped() -> None:
    forbidden_terms = set(_contract()["forbidden_payload_terms"])

    assert "api_key" in forbidden_terms
    assert "secret_value" in forbidden_terms
    assert "embedding_payload" in forbidden_terms
    assert "vector_payload" in forbidden_terms
    assert "raw_prompt" not in forbidden_terms
    assert "raw_prompt_payload" in forbidden_terms
