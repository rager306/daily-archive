"""M514-1lss2s/S09/T03 — the retirement-readiness half of the progress check.

The S09 retirement-readiness receipt is the artifact that binds the S08 acceptance
package, the committed human release record, the S02 required-scope /
authorization declarations and the M513 release-decision package remainder, and
carries the T01 authorization verdict verbatim with ``families_switched: []`` and
``goal_unmet: true``. R141 requires the Governor to report the authorization axis
separately from product coverage and to fail closed, so publishing the receipt
without this sibling half would let its mere presence read as authorization.

This file proves the half that now sees it is honest:

* the rule's label set is exactly the domain's literal set, so a label the oracle
  can emit cannot be unregistered and a registered label cannot be unemittable;
* the committed positive fixture is clean, including the boundary case where the
  declaration authority is absent (absence is silence, never compliance);
* every committed negative record raises its own labelled violation and nothing
  else, and the matrix is required to cover all eight labels;
* the live receipt is resolved by pointer, its carrier pins recompute from the
  committed bytes, the human record is still ``reviewer=null`` / ``decision=pending``
  and the five locks stay closed, and it validates clean;
* the real service path reports no ``retirement_readiness_*`` finding and consumes
  the observation (the regression guard against the inert-half failure mode);
* the half adds no dimension and no severity of its own (D492/D499).

The fixtures are bound to the *live* rule and carry a resolved bundle, so the
fixture exercises the real contract rather than a copy of it. Nothing here
executes a subprocess, reads a network resource, or writes to the tree.
"""

from __future__ import annotations

import copy
import hashlib
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
    _retirement_readiness_violations,
    effective_progress_contract,
    missing_progress_vocabulary,
)
from governor.application.trajectory_service import TrajectoryService  # noqa: E402
from governor.domain.absorption_progress import (  # noqa: E402
    RETIREMENT_READINESS_VIOLATION_LABELS,
)

RULE_PATH = "scripts/governor/checks/grobid_absorption_progress.yaml"
SCHEMA_PATH = "data/parse/absorption_progress.schema.yaml"
BASELINE_PATH = "artifacts/parse_compat/absorption_progress_baseline.json"
RECEIPT_PATH = "artifacts/parse_compat/grobid_retirement_readiness.json"
ACCEPTANCE_MANIFEST = "data/parse/acceptance_manifest.yaml"
POSITIVE_FIXTURE = (
    "governor/fixtures/absorption_progress/native_retirement_readiness.positive.json"
)
NEGATIVE_FIXTURE = (
    "governor/fixtures/absorption_progress/native_retirement_readiness.negative.json"
)
DIMENSIONS_PATH = "scripts/governor/checks/dimensions.yaml"
SEVERITY_PATH = "scripts/governor/checks/assurance_severity.yaml"

def _load_yaml(rel: str) -> dict:
    return yaml.safe_load((_REPO_ROOT / rel).read_text(encoding="utf-8"))

def _load_json(rel: str):
    return json.loads((_REPO_ROOT / rel).read_text(encoding="utf-8"))

def _bare_sha(rel: str) -> str:
    return hashlib.sha256((_REPO_ROOT / rel).read_bytes()).hexdigest()

@pytest.fixture(scope="module")
def rule_contract() -> dict:
    return _load_yaml(RULE_PATH)

@pytest.fixture(scope="module")
def contract(rule_contract: dict) -> dict:
    return effective_progress_contract(_load_yaml(SCHEMA_PATH), rule_contract)

@pytest.fixture(scope="module")
def readiness_rule(contract: dict) -> dict:
    return contract["retirement_readiness"]

@pytest.fixture(scope="module")
def reader() -> FilesystemAbsorptionProgressReader:
    return FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, BASELINE_PATH)

@pytest.fixture(scope="module")
def live_evidence(contract: dict, reader: FilesystemAbsorptionProgressReader) -> dict:
    return reader.read_retirement_readiness(contract)

@pytest.fixture(scope="module")
def live_receipt() -> dict:
    return _load_json(RECEIPT_PATH)

# ── the closed vocabulary is one set, in two places ────────────────────────

def test_rule_registers_exactly_the_domain_literal_vocabulary(
    readiness_rule: dict,
) -> None:
    """The rule may not register a label the oracle cannot emit, or vice versa."""
    assert set(readiness_rule["violations"]) == set(RETIREMENT_READINESS_VIOLATION_LABELS)
    assert len(RETIREMENT_READINESS_VIOLATION_LABELS) == 8

def test_merged_contract_vocabulary_is_complete(contract: dict) -> None:
    assert missing_progress_vocabulary(contract) == []

def test_a_dropped_retirement_readiness_block_fails_closed(contract: dict) -> None:
    stripped = dict(contract)
    stripped.pop("retirement_readiness")
    assert "retirement_readiness" in missing_progress_vocabulary(stripped)

def test_the_retirement_readiness_half_is_not_a_competing_dimension(
    readiness_rule: dict,
) -> None:
    """It rides the existing `absorption_progress` check; it registers nothing new."""
    assert "dimension" not in readiness_rule
    assert "severity" not in readiness_rule
    for rel in (DIMENSIONS_PATH, SEVERITY_PATH):
        text = (_REPO_ROOT / rel).read_text(encoding="utf-8")
        assert "retirement_readiness" not in text, rel
        assert "absorption_progress" not in text, rel

# ── evidence resolution on the live tree ───────────────────────────────────

def test_live_receipt_is_resolved_by_pointer_and_validates_clean(
    contract: dict, live_evidence: dict
) -> None:
    assert live_evidence["available"] is True
    assert live_evidence["errors"] == []
    assert live_evidence["receipt_exists"] is True
    assert isinstance(live_evidence["receipt"], dict)
    assert isinstance(live_evidence["human_record"], dict)
    assert len(live_evidence["gate_ledger"]) == 21
    assert _retirement_readiness_violations(contract, live_evidence) == set()

def test_live_receipt_pins_recompute_from_the_committed_bytes(
    contract: dict, live_evidence: dict, live_receipt: dict
) -> None:
    """A stale pin must fail here, not be discovered as a stale positive control."""
    assert _retirement_readiness_violations(contract, live_evidence) == set()
    for binding in live_receipt["evidence"]:
        manifest = binding["manifest"]
        assert binding["revision_sha256"] == "sha256:" + _bare_sha(manifest), manifest
    for manifest, recomputed in live_evidence["carrier_sha256"].items():
        assert recomputed == "sha256:" + _bare_sha(manifest), manifest

def test_live_receipt_still_describes_the_blocked_state(
    contract: dict, live_receipt: dict
) -> None:
    """The human gate stays open and no family is switched without authorization."""
    expected = contract["retirement_readiness"]["expected"]
    assert live_receipt["claim_scope"] == expected["claim_scope"]
    assert live_receipt["product_coverage_effect"] == expected["product_coverage_effect"]
    assert live_receipt["goal_unmet"] is True
    assert live_receipt["families_switched"] == []
    assert live_receipt["authorization"]["verdict"] == expected["authorization_verdict"]
    assert live_receipt["authorization"]["s09_authorized"] is False
    assert live_receipt["authorization"]["lifecycle_state_authorizes"] is False
    assert live_receipt["authorization"]["basis"] == expected["authorization_basis"]

def test_live_human_gate_invariance(live_evidence: dict, live_receipt: dict) -> None:
    """The agent may not fill the human record: both sides stay pending."""
    human = live_evidence["human_record"]
    assert human["reviewer"] is None
    assert human["decision"] == "pending"
    assert live_receipt["human_gate"]["reviewer_named"] is False
    assert live_receipt["human_gate"]["decision"] == "pending"
    assert live_receipt["human_gate"]["release_recorded"] is False
    assert live_receipt["human_gate"]["package_status"] == "pending_human_review"
    assert live_receipt["human_gate"]["m513_package_status"] == "pending_human_review"

def test_live_receipt_binds_every_required_family_and_gate(
    contract: dict, live_receipt: dict
) -> None:
    expected = contract["retirement_readiness"]["expected"]
    plan_ids = {entry["family_id"] for entry in live_receipt["family_plan"]}
    assert plan_ids == set(expected["required_family_ids"])
    assert len(plan_ids) == 15
    declared_gates = set(live_receipt["authorization"]["blocking_gates"])
    assert declared_gates == set(expected["required_blocking_gate_ids"])
    assert len(declared_gates) == 20

def test_live_body_residual_is_verbatim(live_receipt: dict) -> None:
    assert live_receipt["body_residual"]["token"] == "p1_localization_uncertain"
    assert live_receipt["body_residual"]["gate_id"] == "body_axis.body_not_assessable"

def test_live_locks_stay_closed(live_receipt: dict) -> None:
    locks = live_receipt["locks"]
    assert locks["deployed"] == "false"
    assert locks["import_eligible"] == "false"
    assert locks["auto_promote"] == "forbidden"
    assert locks["production_route"] == "forbidden"
    assert locks["graph_write"] == "forbidden"

def test_live_tree_service_reports_no_retirement_finding() -> None:
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    flags = service._check_absorption_progress()
    leaked = [
        f for f in flags
        if "retirement_readiness" in (f.flag or "") or "retirement_readiness" in (f.evidence or "")
    ]
    assert leaked == [], [f.flag for f in flags]
    assert flags == [], [f.flag for f in flags]

def test_the_service_path_consumes_the_retirement_observation(monkeypatch) -> None:
    """The half must fail closed through the real service, not only when called
    directly.

    This is the regression guard against the *inert half* failure mode the S04
    threat review found (MEM1948): a sibling block whose oracle is never fed by
    the service reads as clean on every tree, so the half could be deleted or
    silently regressed without any check noticing. Patching the reader to return a
    claim-without-receipt bundle must raise the composed finding through
    `_check_absorption_progress`, which proves the observation is consumed.
    """
    def broken(self, contract):  # noqa: ANN001, ARG001
        return {"available": True, "receipt": None, "receipt_exists": False}

    monkeypatch.setattr(
        FilesystemAbsorptionProgressReader, "read_retirement_readiness", broken
    )
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    flags = service._check_absorption_progress()
    broken_flags = [f for f in flags if f.flag == "absorption_progress_broken"]
    assert broken_flags, [f.flag for f in flags]
    assert "retirement_readiness_claim_without_receipt" in broken_flags[0].evidence

# ── fixtures: positive clean, negative matrix covers the closed set ────────

def test_positive_fixture_is_clean(contract: dict) -> None:
    records = _load_json(POSITIVE_FIXTURE)
    assert isinstance(records, list) and records
    for record in records:
        assert _retirement_readiness_violations(contract, record) == set()

def test_negative_fixture_triggers_every_registered_label(contract: dict) -> None:
    records = _load_json(NEGATIVE_FIXTURE)
    assert isinstance(records, list) and records
    observed: set[str] = set()
    for entry in records:
        label = entry["violation"]
        violations = _retirement_readiness_violations(contract, entry["record"])
        observed |= violations
        assert label in violations, (label, sorted(violations))
        # Each record is a clean bundle with exactly one mutation, so the target
        # label is the only finding: a mutation that trips a second label is a
        # real overlap and must be made explicit rather than tolerated.
        assert violations == {label}, (label, sorted(violations))
    assert observed <= set(RETIREMENT_READINESS_VIOLATION_LABELS)
    assert set(RETIREMENT_READINESS_VIOLATION_LABELS) <= observed

def test_negative_fixture_registration_matches_the_literal_set(
    readiness_rule: dict,
) -> None:
    required = set(
        readiness_rule["fixtures"]["negative"]["expect"]["require_violations"]
    )
    assert required == set(RETIREMENT_READINESS_VIOLATION_LABELS)

def test_absent_evidence_is_unverified_never_refuted(contract: dict) -> None:
    """Non-vacuity boundary: silence is not compliance, and it is not a finding."""
    assert _retirement_readiness_violations(contract, None) == set()
    assert _retirement_readiness_violations(contract, {}) == set()
    assert _retirement_readiness_violations(contract, {"available": False}) == set()

def test_the_declaration_authority_gates_availability(contract: dict) -> None:
    """A tree without the S03 authority is silence, not a claim and not a finding."""
    reader = FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, BASELINE_PATH)
    rule = contract["retirement_readiness"]
    absent = reader.read_retirement_readiness(
        {
            "retirement_readiness": {
                **rule,
                "declaration": {
                    "manifest": "data/parse/absent_bakeoff.yaml",
                    "pointer": "java_free_vertical_slice",
                },
            }
        }
    )
    assert absent is not None
    assert absent["available"] is False
    assert _retirement_readiness_violations(contract, absent) == set()

def test_a_resolvable_claim_without_a_receipt_is_refused(contract: dict) -> None:
    """The shape R141 forbids: a declared blocked receipt with no evidence."""
    records = _load_json(NEGATIVE_FIXTURE)
    by_label = {entry["violation"]: entry["record"] for entry in records}
    missing = by_label["retirement_readiness_claim_without_receipt"]
    assert missing["available"] is True
    assert missing["receipt"] is None
    assert _retirement_readiness_violations(contract, missing) == {
        "retirement_readiness_claim_without_receipt"
    }

# ── mutation semantics the oracle must refuse ──────────────────────────────

def _clean_bundle() -> dict:
    return copy.deepcopy(_load_json(POSITIVE_FIXTURE)[0])

def test_a_stale_carrier_pin_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipt"]["evidence"][0]["revision_sha256"] = "sha256:" + "9" * 64
    assert _retirement_readiness_violations(contract, record) == {
        "retirement_readiness_carrier_stale"
    }

def test_a_changed_human_record_stales_the_receipt(contract: dict) -> None:
    """Filling the human record changes the carrier bytes, so the pin must mismatch.

    The oracle is deliberately not fed the live record bytes; it compares the
    receipt's pins with the reader's recomputation. Simulating the recomputed
    change (as the acceptance-package test does) proves the direction the receipt
    cannot hide: a receipt that still pins the open record is stale once the record
    bytes move.
    """
    record = _clean_bundle()
    record["carrier_sha256"][ACCEPTANCE_MANIFEST] = "sha256:" + "a" * 64
    assert _retirement_readiness_violations(contract, record) == {
        "retirement_readiness_carrier_stale"
    }

def test_a_claimed_human_decision_without_a_record_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipt"]["authorization"]["human_decision_recorded"] = True
    assert _retirement_readiness_violations(contract, record) == {
        "retirement_readiness_human_decision_claimed_without_record"
    }

    record = _clean_bundle()
    record["receipt"]["human_gate"]["release_recorded"] = True
    assert _retirement_readiness_violations(contract, record) == {
        "retirement_readiness_human_decision_claimed_without_record"
    }

def test_a_lifecycle_authorization_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipt"]["authorization"]["lifecycle_state_authorizes"] = True
    assert _retirement_readiness_violations(contract, record) == {
        "retirement_readiness_lifecycle_authorization"
    }

def test_a_family_switched_with_a_blocking_gate_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipt"]["family_plan"][0]["switched"] = True
    assert _retirement_readiness_violations(contract, record) == {
        "retirement_readiness_family_switched_with_blocking_gate"
    }

    record = _clean_bundle()
    record["receipt"]["families_switched"] = [record["receipt"]["family_plan"][0]["family_id"]]
    assert _retirement_readiness_violations(contract, record) == {
        "retirement_readiness_family_switched_with_blocking_gate"
    }

def test_a_renamed_body_residual_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipt"]["body_residual"]["token"] = "p1_localization_unknown"
    assert _retirement_readiness_violations(contract, record) == {
        "retirement_readiness_body_residual_renamed"
    }

def test_a_product_coverage_claim_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipt"]["claim_scope"] = "authorized"
    assert _retirement_readiness_violations(contract, record) == {
        "retirement_readiness_product_coverage_claimed"
    }

    record = _clean_bundle()
    record["receipt"]["product_coverage_effect"] = "some"
    assert _retirement_readiness_violations(contract, record) == {
        "retirement_readiness_product_coverage_claimed"
    }

    record = _clean_bundle()
    record["receipt"]["locks"]["deployed"] = "true"
    assert _retirement_readiness_violations(contract, record) == {
        "retirement_readiness_product_coverage_claimed"
    }

    record = _clean_bundle()
    record["receipt"]["redaction"]["raw_text"] = True
    assert _retirement_readiness_violations(contract, record) == {
        "retirement_readiness_product_coverage_claimed"
    }

def test_a_dropped_required_family_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipt"]["family_plan"] = record["receipt"]["family_plan"][:-1]
    assert _retirement_readiness_violations(contract, record) == {
        "retirement_readiness_product_coverage_claimed"
    }

def test_a_goal_marked_met_while_unauthorized_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipt"]["goal_unmet"] = False
    assert _retirement_readiness_violations(contract, record) == {
        "retirement_readiness_goal_marked_met"
    }

def test_a_receipt_for_another_schema_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipt"]["schema_version"] = "grobid-retirement-readiness.v0"
    assert _retirement_readiness_violations(contract, record) == {
        "retirement_readiness_claim_without_receipt"
    }
