from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUNBOOK = ROOT / "data/architecture-assessment/m198-operator-readiness-runbook.md"

REQUIRED_SNIPPETS = (
    "scripts/run_m198_readiness_rehearsal.py",
    "scripts/run_m198_smoke_parity_audit.py",
    "scripts/run_m198_disabled_backend_safety.py",
    "scripts/run_m198_validation_package.py",
    "m198.readiness_rehearsal.v1",
    "m198.smoke_parity_audit.v1",
    "m198.disabled_backend_safety.v1",
    "m198.validation_package.v1",
    "m198-gitnexus-impact-gates.json",
    "gitnexus analyze",
    "repo=daily-archive",
    "uv run pyrefly check",
    "tests/test_m198_validation_package.py",
    "tests/test_m195_governance_ratchets.py",
    "R076",
    "R077",
    "R078",
)

FORBIDDEN_SNIPPETS = (
    "gitnexus analyze --repo daily-archive",
    "import_eligible=true",
    "import_eligible = true",
    "graph_writes_allowed=true",
    "graph_writes_allowed = true",
    "schema_migration_allowed=true",
    "schema_migration_allowed = true",
    "raw_prompt_payload",
    "source_text_payload",
    "embedding_payload",
    "vector_payload",
    "secret_value",
)


def _text() -> str:
    return RUNBOOK.read_text(encoding="utf-8")


def test_operator_runbook_contains_required_commands_and_contracts() -> None:
    text = _text()

    for snippet in REQUIRED_SNIPPETS:
        assert snippet in text


def test_operator_runbook_documents_exit_codes_and_interpretation() -> None:
    text = _text()

    assert "`0`:" in text
    assert "`2`:" in text
    assert "inspect aggregate `blockers` first" in text
    assert "Warnings do not authorize promotion" in text
    assert "metadata_only" in text
    assert "payload_policy_confirmed" in text


def test_operator_runbook_preserves_non_goals() -> None:
    text = _text()

    assert "Production graph import." in text
    assert "Schema migration." in text
    assert "Queue dependency semantic changes." in text
    assert "Smoke runtime semantic changes." in text
    assert "Rehearsal runtime semantic changes." in text
    assert "Retired graph readiness shim restoration." in text
    assert "Import eligibility promotion." in text


def test_operator_runbook_avoids_forbidden_instructions() -> None:
    lowered = _text().lower()

    for snippet in FORBIDDEN_SNIPPETS:
        assert snippet.lower() not in lowered
    retired_module = ".".join(("arxiv_archive", "graph_readiness_review"))
    assert retired_module not in lowered
