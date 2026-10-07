"""M516-th0oby/S05/T04 — the required-scope downgrade half of the progress check.

S05/T01 recorded the 12 pending required-scope families as ``not_applicable``
downgrades in the parity ledger (each with an owner, a reason drawn from the
seven outside-corpus-parity reasons, a decision reference and a rationale), and
S05/T03 carried every one of those downgrades through the mandatory gate ledger
as a non-blocking ``required_family_downgrade`` gate. A downgrade is a
*disposition*, never a measurement (D550), so the governor must read the same
registry the runtime reads and must never count a recorded downgrade as a
measurement.

This file proves the half that now reads it is honest:

* the rule's label set is exactly the domain's literal set, so a label the oracle
  can emit cannot be unregistered and a registered label cannot be unemittable;
* the seven-reason vocabulary the rule carries is exactly the schema's
  ``required_scope.pending_reason_vocabulary``, so the restated list cannot drift;
* the committed positive fixture is clean, including the boundary case where the
  declaration authority is absent (absence is silence, never compliance);
* every committed negative record raises its own labelled violation and nothing
  else, and the matrix covers all five labels exactly once;
* the live tree resolves the registry, its 12 records match the 12
  ``required_family_downgrade``/``not_applicable`` gates, no record targets a
  measured family and the half is clean;
* the real service path reports no ``required_scope_downgrade_*`` finding and
  consumes the observation (the regression guard against the inert-half failure
  mode, MEM1948);
* the half adds no dimension and no severity of its own (D492/D550).

The fixtures are bound to the *live* rule and carry a resolved bundle, so the
fixture exercises the real contract rather than a copy of it. Nothing here
executes a subprocess, reads a network resource, or writes to the tree.
"""

from __future__ import annotations

import copy
import json
import sys
from pathlib import Path

import pytest
import yaml

_SCRIPTS_DIR = Path(__file__).resolve().parent.parent
_REPO_ROOT = _SCRIPTS_DIR.parent
if str(_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS_DIR))

from governor.adapters.absorption_progress_reader import (  # noqa: E402
    FilesystemAbsorptionProgressReader,
    _required_scope_downgrades_violations,
    effective_progress_contract,
    missing_progress_vocabulary,
)
from governor.application.trajectory_service import TrajectoryService  # noqa: E402
from governor.domain.absorption_progress import (  # noqa: E402
    REQUIRED_SCOPE_DOWNGRADE_VIOLATION_LABELS,
)

RULE_PATH = "scripts/governor/checks/grobid_absorption_progress.yaml"
SCHEMA_PATH = "data/parse/absorption_progress.schema.yaml"
BASELINE_PATH = "artifacts/parse_compat/absorption_progress_baseline.json"
POSITIVE_FIXTURE = (
    "governor/fixtures/absorption_progress/required_scope_downgrades.positive.json"
)
NEGATIVE_FIXTURE = (
    "governor/fixtures/absorption_progress/required_scope_downgrades.negative.json"
)
DIMENSIONS_PATH = "scripts/governor/checks/dimensions.yaml"
SEVERITY_PATH = "scripts/governor/checks/assurance_severity.yaml"

def _load_yaml(rel: str) -> dict:
    return yaml.safe_load((_REPO_ROOT / rel).read_text(encoding="utf-8"))

def _load_json(rel: str):
    return json.loads((_REPO_ROOT / rel).read_text(encoding="utf-8"))

@pytest.fixture(scope="module")
def rule_contract() -> dict:
    return _load_yaml(RULE_PATH)

@pytest.fixture(scope="module")
def contract(rule_contract: dict) -> dict:
    return effective_progress_contract(_load_yaml(SCHEMA_PATH), rule_contract)

@pytest.fixture(scope="module")
def downgrade_rule(contract: dict) -> dict:
    return contract["required_scope_downgrades"]

@pytest.fixture(scope="module")
def reader() -> FilesystemAbsorptionProgressReader:
    return FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, BASELINE_PATH)

@pytest.fixture(scope="module")
def live_evidence(contract: dict, reader: FilesystemAbsorptionProgressReader) -> dict:
    return reader.read_required_scope_downgrades(contract)

# ── the closed vocabulary is one set, in two places ────────────────────────

def test_rule_registers_exactly_the_domain_literal_vocabulary(
    downgrade_rule: dict,
) -> None:
    """The rule may not register a label the oracle cannot emit, or vice versa."""
    assert set(downgrade_rule["violations"]) == set(
        REQUIRED_SCOPE_DOWNGRADE_VIOLATION_LABELS
    )
    assert len(REQUIRED_SCOPE_DOWNGRADE_VIOLATION_LABELS) == 5

def test_the_rule_reason_vocabulary_matches_the_schema(
    downgrade_rule: dict, contract: dict
) -> None:
    """The restated seven reasons may not drift from the schema's closed list."""
    schema = _load_yaml(SCHEMA_PATH)
    assert sorted(downgrade_rule["expected"]["reason_vocabulary"]) == sorted(
        schema["required_scope"]["pending_reason_vocabulary"]
    )

def test_merged_contract_vocabulary_is_complete(contract: dict) -> None:
    assert missing_progress_vocabulary(contract) == []

def test_a_dropped_required_scope_downgrades_block_fails_closed(contract: dict) -> None:
    stripped = dict(contract)
    stripped.pop("required_scope_downgrades")
    assert "required_scope_downgrades" in missing_progress_vocabulary(stripped)

def test_the_downgrade_half_is_not_a_competing_dimension(downgrade_rule: dict) -> None:
    """It rides the existing `absorption_progress` check; it registers nothing new."""
    assert "dimension" not in downgrade_rule
    assert "severity" not in downgrade_rule
    for rel in (DIMENSIONS_PATH, SEVERITY_PATH):
        text = (_REPO_ROOT / rel).read_text(encoding="utf-8")
        assert "required_scope_downgrade" not in text, rel
        assert "absorption_progress" not in text, rel

# ── evidence resolution on the live tree ───────────────────────────────────

def test_live_registry_is_resolved_and_validates_clean(
    contract: dict, live_evidence: dict
) -> None:
    assert live_evidence["available"] is True
    assert live_evidence["errors"] == []
    assert isinstance(live_evidence["records"], dict)
    assert isinstance(live_evidence["required_families"], list)
    assert isinstance(live_evidence["gate_ledger"], list)
    assert _required_scope_downgrades_violations(contract, live_evidence) == set()

def test_live_registry_has_twelve_records_matching_twelve_gates(
    contract: dict, live_evidence: dict, downgrade_rule: dict
) -> None:
    """The 12 recorded families and the 12 downgrade gates are the same set."""
    expected = downgrade_rule["expected"]
    kind = expected["gate_kind"]
    state = expected["gate_state"]
    gate_families = {
        gate["family_id"]
        for gate in live_evidence["gate_ledger"]
        if gate.get("kind") == kind and gate.get("state") == state
    }
    assert len(live_evidence["records"]) == 12
    assert gate_families == set(live_evidence["records"])
    assert len(gate_families) == 12

def test_live_records_name_a_required_family_and_never_a_measured_one(
    live_evidence: dict, downgrade_rule: dict
) -> None:
    measured = downgrade_rule["families"]["measured_identity"]
    family_identity = {
        entry["family_id"]: entry["measurement_identity"]
        for entry in live_evidence["required_families"]
    }
    for family_id, record in live_evidence["records"].items():
        assert family_id in family_identity, family_id
        assert family_identity[family_id] != measured, family_id
        assert record["disposition"] in downgrade_rule["expected"]["dispositions"]
        assert record["reason"] in downgrade_rule["expected"]["reason_vocabulary"]
        for field in downgrade_rule["expected"]["required_record_fields"]:
            assert record.get(field), (family_id, field)

def test_live_tree_service_reports_no_downgrade_finding() -> None:
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    flags = service._check_absorption_progress()
    leaked = [
        f
        for f in flags
        if "required_scope_downgrade" in (f.flag or "")
        or "required_scope_downgrade" in (f.evidence or "")
    ]
    assert leaked == [], [f.flag for f in flags]

def test_the_service_path_consumes_the_downgrade_observation(monkeypatch) -> None:
    """The half must fail closed through the real service, not only when called
    directly (MEM1948): a sibling block whose oracle is never fed by the service
    reads as clean on every tree. Patching the reader to return a claim with no
    records must raise the composed finding through `_check_absorption_progress`.
    """
    def broken(self, contract):  # noqa: ANN001, ARG001
        return {
            "available": True,
            "records": {},
            "required_families": [],
            "gate_ledger": [
                {
                    "gate_id": "required_scope.rest.isalive",
                    "kind": "required_family_downgrade",
                    "state": "not_applicable",
                    "family_id": "rest.isalive",
                }
            ],
        }

    monkeypatch.setattr(
        FilesystemAbsorptionProgressReader, "read_required_scope_downgrades", broken
    )
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    flags = service._check_absorption_progress()
    broken_flags = [f for f in flags if f.flag == "absorption_progress_broken"]
    assert broken_flags, [f.flag for f in flags]
    assert "required_scope_downgrade_gate_without_record" in broken_flags[0].evidence

# ── fixtures: positive clean, negative matrix covers the closed set ────────

def test_positive_fixture_is_clean(contract: dict) -> None:
    records = _load_json(POSITIVE_FIXTURE)
    assert isinstance(records, list) and records
    for record in records:
        assert _required_scope_downgrades_violations(contract, record) == set()

def test_negative_fixture_triggers_every_registered_label(contract: dict) -> None:
    records = _load_json(NEGATIVE_FIXTURE)
    assert isinstance(records, list) and records
    observed: set[str] = set()
    for entry in records:
        label = entry["violation"]
        violations = _required_scope_downgrades_violations(contract, entry["record"])
        observed |= violations
        assert label in violations, (label, sorted(violations))
        # Each record is a clean bundle with exactly one mutation, so the target
        # label is the only finding: a mutation that trips a second label is a
        # real overlap and must be made explicit rather than tolerated.
        assert violations == {label}, (label, sorted(violations))
    assert observed <= set(REQUIRED_SCOPE_DOWNGRADE_VIOLATION_LABELS)
    assert set(REQUIRED_SCOPE_DOWNGRADE_VIOLATION_LABELS) <= observed

def test_negative_fixture_registration_matches_the_literal_set(
    downgrade_rule: dict,
) -> None:
    required = set(
        downgrade_rule["fixtures"]["negative"]["expect"]["require_violations"]
    )
    assert required == set(REQUIRED_SCOPE_DOWNGRADE_VIOLATION_LABELS)

def test_absent_evidence_is_unverified_never_refuted(contract: dict) -> None:
    """Non-vacuity boundary: silence is not compliance, and it is not a finding."""
    assert _required_scope_downgrades_violations(contract, None) == set()
    assert _required_scope_downgrades_violations(contract, {}) == set()
    assert _required_scope_downgrades_violations(contract, {"available": False}) == set()

def test_the_declaration_authority_gates_availability(contract: dict) -> None:
    """A tree without the registry authority is silence, not a claim and not a finding."""
    reader = FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, BASELINE_PATH)
    rule = contract["required_scope_downgrades"]
    absent = reader.read_required_scope_downgrades(
        {
            "required_scope_downgrades": {
                **rule,
                "declaration": {
                    "manifest": "data/parse/absent_compatibility.yaml",
                    "pointer": "parity_ledger.required_scope_downgrades",
                },
            }
        }
    )
    assert absent is not None
    assert absent["available"] is False
    assert _required_scope_downgrades_violations(contract, absent) == set()

# ── mutation semantics the oracle must refuse ──────────────────────────────

def _clean_bundle() -> dict:
    return copy.deepcopy(_load_json(POSITIVE_FIXTURE)[0])

def test_a_gate_without_a_record_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    del record["records"]["rest.isalive"]
    assert _required_scope_downgrades_violations(contract, record) == {
        "required_scope_downgrade_gate_without_record"
    }

def test_a_record_without_a_gate_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["gate_ledger"] = [
        gate
        for gate in record["gate_ledger"]
        if gate.get("family_id") != "rest.isalive"
    ]
    assert _required_scope_downgrades_violations(contract, record) == {
        "required_scope_downgrade_record_without_gate"
    }

def test_a_record_for_an_unknown_family_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["records"]["stage.not_declared"] = {
        "disposition": "not_applicable",
        "owner": "M516-th0oby/S05",
        "reason": "rest_transport_surface",
        "decision_ref": "D551",
        "rationale": "not a required-scope family",
    }
    record["gate_ledger"].append(
        {
            "gate_id": "required_scope.stage.not_declared",
            "kind": "required_family_downgrade",
            "state": "not_applicable",
            "family_id": "stage.not_declared",
        }
    )
    assert _required_scope_downgrades_violations(contract, record) == {
        "required_scope_downgrade_unknown_family"
    }

def test_a_record_for_a_measured_family_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["records"]["stage.header_composition"] = {
        "disposition": "not_applicable",
        "owner": "M516-th0oby/S05",
        "reason": "rest_transport_surface",
        "decision_ref": "D551",
        "rationale": "this family is measured by corpus parity",
    }
    record["gate_ledger"].append(
        {
            "gate_id": "required_scope.stage.header_composition",
            "kind": "required_family_downgrade",
            "state": "not_applicable",
            "family_id": "stage.header_composition",
        }
    )
    assert _required_scope_downgrades_violations(contract, record) == {
        "required_scope_downgrade_for_measured_family"
    }

def test_an_incomplete_record_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    del record["records"]["rest.isalive"]["owner"]
    assert _required_scope_downgrades_violations(contract, record) == {
        "required_scope_downgrade_record_incomplete"
    }

    record = _clean_bundle()
    record["records"]["rest.isalive"]["reason"] = "made_up_reason"
    assert _required_scope_downgrades_violations(contract, record) == {
        "required_scope_downgrade_record_incomplete"
    }
