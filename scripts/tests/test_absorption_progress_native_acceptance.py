"""M514-1lss2s/S08/T06 — the acceptance-package gate-ledger half of the progress check.

The S08 acceptance package is the artifact that binds all fourteen S08 evidence
carriers by repository-relative path + pointer + sha256, carries the T02
frozen-threshold evaluation and names every mandatory gate with a computed
``s09_authorized``. R141 is explicit that the Governor may never read an
artifact's existence or a terminal GSD state as delivered capability, so
publishing the package without this sibling half would let its mere presence read
as progress. This file proves the half that now sees it is honest:

* the rule's label set is exactly the domain's literal set, so a label the oracle
  can emit cannot be unregistered and a registered label cannot be unemittable;
* the committed positive fixture is clean, including the boundary case where the
  declaration authority is absent (absence is silence, never compliance);
* every committed negative record raises its own labelled violation and nothing
  else, and the matrix is required to cover all thirteen labels plus each of the
  five pooled-resource refusal shapes exactly once;
* the live package is resolved by pointer, its carrier pins recompute from the
  committed bytes, its blocking set re-derives from the gate ledger, and it
  validates clean;
* the real service path reports no ``acceptance_package_*`` finding and consumes
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
    _acceptance_package_violations,
    effective_progress_contract,
    missing_progress_vocabulary,
)
from governor.application.trajectory_service import TrajectoryService  # noqa: E402
from governor.domain.absorption_progress import (  # noqa: E402
    ACCEPTANCE_PACKAGE_VIOLATION_LABELS,
)

RULE_PATH = "scripts/governor/checks/grobid_absorption_progress.yaml"
SCHEMA_PATH = "data/parse/absorption_progress.schema.yaml"
BASELINE_PATH = "artifacts/parse_compat/absorption_progress_baseline.json"
PACKAGE_PATH = "artifacts/parse_compat/grobid_retirement_acceptance_package.json"
PINS_PATH = "artifacts/parse_compat/absorption_progress_reference_pins.json"
ROLLBACK_PATH = "artifacts/parse_compat/native_corpus_rollback_rehearsal.json"
RESOURCE_PATH = "artifacts/parse_compat/native_corpus_resource_same_n.json"
DECLARATION_MANIFEST = "data/parse/layout_bakeoff.yaml"
POSITIVE_FIXTURE = "governor/fixtures/absorption_progress/native_acceptance.positive.json"
NEGATIVE_FIXTURE = "governor/fixtures/absorption_progress/native_acceptance.negative.json"
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
def package_rule(contract: dict) -> dict:
    return contract["acceptance_package"]

@pytest.fixture(scope="module")
def reader() -> FilesystemAbsorptionProgressReader:
    return FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, BASELINE_PATH)

@pytest.fixture(scope="module")
def live_evidence(contract: dict, reader: FilesystemAbsorptionProgressReader) -> dict:
    return reader.read_acceptance_package(contract)

@pytest.fixture(scope="module")
def live_package() -> dict:
    return _load_json(PACKAGE_PATH)

# ── the closed vocabulary is one set, in two places ────────────────────────

def test_rule_registers_exactly_the_domain_literal_vocabulary(package_rule: dict) -> None:
    """The rule may not register a label the oracle cannot emit, or vice versa."""
    assert set(package_rule["violations"]) == set(ACCEPTANCE_PACKAGE_VIOLATION_LABELS)
    assert len(ACCEPTANCE_PACKAGE_VIOLATION_LABELS) == 13

def test_merged_contract_vocabulary_is_complete(contract: dict) -> None:
    assert missing_progress_vocabulary(contract) == []

def test_a_dropped_acceptance_package_block_fails_closed(contract: dict) -> None:
    stripped = dict(contract)
    stripped.pop("acceptance_package")
    assert "acceptance_package" in missing_progress_vocabulary(stripped)

def test_the_acceptance_package_half_is_not_a_competing_dimension(package_rule: dict) -> None:
    """It rides the existing `absorption_progress` check; it registers nothing new."""
    assert "dimension" not in package_rule
    assert "severity" not in package_rule
    for rel in (DIMENSIONS_PATH, SEVERITY_PATH):
        text = (_REPO_ROOT / rel).read_text(encoding="utf-8")
        assert "acceptance_package" not in text, rel
        assert "absorption_progress" not in text, rel

# ── evidence resolution on the live tree ───────────────────────────────────

def test_live_package_is_resolved_by_pointer_and_validates_clean(
    contract: dict, live_evidence: dict
) -> None:
    assert live_evidence["available"] is True
    assert live_evidence["errors"] == []
    assert live_evidence["package_exists"] is True
    assert isinstance(live_evidence["package"], dict)
    assert len(live_evidence["reference_pins"]) == 3
    assert live_evidence["rollback"] is not None
    assert live_evidence["resource"] is not None
    assert _acceptance_package_violations(contract, live_evidence) == set()

def test_live_package_pins_recompute_from_the_committed_bytes(
    contract: dict, live_evidence: dict, live_package: dict
) -> None:
    """A stale pin must fail here, not be discovered as a stale positive control."""
    assert _acceptance_package_violations(contract, live_evidence) == set()
    for binding in live_package["evidence"]:
        manifest = binding["manifest"]
        assert binding["revision_sha256"] == "sha256:" + _bare_sha(manifest), manifest
    for manifest, recomputed in live_evidence["carrier_sha256"].items():
        assert recomputed == "sha256:" + _bare_sha(manifest), manifest

def test_live_package_names_every_gate_and_recomputes_authorization(
    contract: dict, live_package: dict
) -> None:
    """Every declared gate is present and the blocking set is derived, not trusted."""
    expected = contract["acceptance_package"]["expected"]
    gate_ids = [gate["gate_id"] for gate in live_package["mandatory_gates"]]
    assert set(gate_ids) == set(expected["required_gate_ids"])
    assert len(gate_ids) == expected["minimum_mandatory_gates"] == 21

    non_blocking = set(expected["non_blocking_states"])
    recomputed = [
        (gate["gate_id"], gate["state"])
        for gate in live_package["mandatory_gates"]
        if gate["required"] is True and gate["state"] not in non_blocking
    ]
    declared = [(entry["gate_id"], entry["state"]) for entry in live_package["blocking_gates"]]
    assert declared == recomputed
    # T04 (M515/S02): both extraction thresholds now resolve `met`, so their
    # gates left the blocking set (19 -> 17). T03 (M515/S03) derived the Java
    # resource gate as a measured `resource_pair`/`met`, dropping it from the
    # blocking set (17 -> 16). S09 remains unauthorized on the body-axis and
    # required-scope gates.
    assert len(declared) == 16

    authorization = live_package["authorization"]
    assert authorization["s09_authorized"] is False
    assert authorization["gsd_lifecycle_authorizes"] is False
    assert authorization["basis"] == expected["authorization_basis"]

def test_live_package_keeps_the_body_residual_verbatim(live_package: dict) -> None:
    body = [gate for gate in live_package["mandatory_gates"] if gate["kind"] == "body_axis"]
    assert len(body) == 1
    assert body[0]["state"] == "not_assessable"
    assert body[0]["reason"] == "p1_localization_uncertain"

def test_live_rollback_carries_byte_identity(live_evidence: dict) -> None:
    rollback = live_evidence["rollback"]
    assert rollback["claim_scope"] == "rehearsal_only"
    assert rollback["corpus_claim"]["byte_identity_holds"] is True
    assert rollback["corpus_claim"]["native_fragment_leak_free"] is True
    assert rollback["documents_byte_identical"] == rollback["corpus_claim"]["rehearsed_documents"]
    assert rollback["documents_rehearsed"] == rollback["corpus_claim"]["declared_documents"]
    assert rollback["residuals"]["byte_identity_violations"] == 0

def test_live_resource_reports_a_measured_pair_and_typed_absences(
    live_evidence: dict, package_rule: dict
) -> None:
    resource = live_evidence["resource"]
    assert resource["pairing"]["rule"] == "same_documents_only"
    assert resource["pairing"]["pooled"] == "forbidden"
    assert resource["pairing"]["saving_claim"] == "forbidden"

    artifact = _load_json(RESOURCE_PATH)
    same_n_manifests = {
        entry["carrier"]["manifest"]
        for entry in artifact["reference_carriers"]
        if entry.get("same_n") is True
    }
    assert same_n_manifests

    observed = [
        row for row in resource["reference_metrics"] if row["state"] == "observed"
    ]
    absent = [
        row
        for row in resource["reference_metrics"]
        if row["state"] == "not_observed"
    ]
    assert observed, "the corpus receipt must carry at least one measured reference row"
    assert absent, "the metrics the carrier cannot supply must stay typed absences"
    assert observed[0]["id"] == "java_latency_ms"
    source = observed[0]["source"]
    assert source is not None
    assert source["kind"] == "measured_reference"
    assert source["manifest"] in same_n_manifests
    assert source["pointer"] == "documents[].latency_ms"
    assert source["measurement_state"] == "measured"

    vocabulary = set(package_rule["resource_carrier"]["absence_reason_vocabulary"])
    assert vocabulary
    for row in absent:
        assert row["source"] is None
        assert row["reason"] in vocabulary
    assert resource["residuals"]["reference_rows_bound"] == len(observed)

def test_live_tree_service_reports_no_acceptance_finding() -> None:
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    flags = service._check_absorption_progress()
    leaked = [
        f for f in flags
        if "acceptance_package" in (f.flag or "") or "acceptance_package" in (f.evidence or "")
    ]
    assert leaked == [], [f.flag for f in flags]
    # The composed absorption check also carries the retirement-readiness half,
    # which T05 rebinds to the re-issued acceptance package. This module owns the
    # acceptance-package half, so assert no acceptance finding and no finding
    # from any other half; a readiness-only finding is tracked by its own module.
    unrelated = [
        f
        for f in flags
        if "acceptance_package" not in (f.evidence or "")
        and "retirement_readiness" not in (f.evidence or "")
    ]
    assert unrelated == [], [f.flag for f in flags]

def test_the_service_path_consumes_the_acceptance_observation(monkeypatch) -> None:
    """The half must fail closed through the real service, not only when called
    directly.

    This is the regression guard against the *inert half* failure mode the S04
    threat review found (MEM1948): a sibling block whose oracle is never fed by
    the service reads as clean on every tree, so the half could be deleted or
    silently regress without any check noticing. Patching the reader to return a
    claim-without-package bundle must raise the composed finding through
    `_check_absorption_progress`, which proves the observation is consumed.
    """
    def broken(self, contract):  # noqa: ANN001, ARG001
        return {"available": True, "package": None, "package_exists": False}

    monkeypatch.setattr(
        FilesystemAbsorptionProgressReader, "read_acceptance_package", broken
    )
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    flags = service._check_absorption_progress()
    broken_flags = [f for f in flags if f.flag == "absorption_progress_broken"]
    assert broken_flags, [f.flag for f in flags]
    assert "acceptance_package_claim_without_package" in broken_flags[0].evidence

# ── fixtures: positive clean, negative matrix covers the closed set ────────

def test_positive_fixture_is_clean(contract: dict) -> None:
    records = _load_json(POSITIVE_FIXTURE)
    assert isinstance(records, list) and records
    for record in records:
        assert _acceptance_package_violations(contract, record) == set()

def test_negative_fixture_triggers_every_registered_label(contract: dict) -> None:
    records = _load_json(NEGATIVE_FIXTURE)
    assert isinstance(records, list) and records
    observed: set[str] = set()
    for entry in records:
        label = entry["violation"]
        violations = _acceptance_package_violations(contract, entry["record"])
        observed |= violations
        assert label in violations, (label, sorted(violations))
        # Each record is a clean bundle with exactly one mutation, so the target
        # label is the only finding: a mutation that trips a second label is a
        # real overlap and must be made explicit rather than tolerated.
        assert violations == {label}, (label, sorted(violations))
    assert observed <= set(ACCEPTANCE_PACKAGE_VIOLATION_LABELS)
    assert set(ACCEPTANCE_PACKAGE_VIOLATION_LABELS) <= observed

def test_negative_fixture_registration_matches_the_literal_set(package_rule: dict) -> None:
    required = set(
        package_rule["fixtures"]["negative"]["expect"]["require_violations"]
    )
    assert required == set(ACCEPTANCE_PACKAGE_VIOLATION_LABELS)

def test_absent_evidence_is_unverified_never_refuted(contract: dict) -> None:
    """Non-vacuity boundary: silence is not compliance, and it is not a finding."""
    assert _acceptance_package_violations(contract, None) == set()
    assert _acceptance_package_violations(contract, {}) == set()
    assert _acceptance_package_violations(contract, {"available": False}) == set()

def test_the_declaration_authority_gates_availability(contract: dict) -> None:
    """A tree without the S03 authority is silence, not a claim and not a finding."""
    reader = FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, BASELINE_PATH)
    rule = contract["acceptance_package"]
    absent = reader.read_acceptance_package(
        {
            "acceptance_package": {
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
    assert _acceptance_package_violations(contract, absent) == set()

def test_a_resolvable_claim_without_a_package_is_refused(contract: dict) -> None:
    """The shape R141 forbids: a declared package with no evidence."""
    records = _load_json(NEGATIVE_FIXTURE)
    by_label = {entry["violation"]: entry["record"] for entry in records}
    missing = by_label["acceptance_package_claim_without_package"]
    assert missing["available"] is True
    assert missing["package"] is None
    assert _acceptance_package_violations(contract, missing) == {
        "acceptance_package_claim_without_package"
    }

# ── mutation semantics the oracle must refuse ──────────────────────────────

def _clean_bundle() -> dict:
    return copy.deepcopy(_load_json(POSITIVE_FIXTURE)[0])

def test_a_stale_carrier_pin_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["package"]["evidence"][0]["revision_sha256"] = "sha256:" + "9" * 64
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_carrier_stale"
    }

    record = _clean_bundle()
    record["carrier_sha256"][record["package"]["evidence"][0]["manifest"]] = (
        "sha256:" + "9" * 64
    )
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_carrier_stale"
    }

def test_a_detached_threshold_verdict_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["package"]["evaluation"]["thresholds"][0]["family_id"] = "stage.renamed"
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_threshold_unbound"
    }

    record = _clean_bundle()
    record["package"]["evaluation"]["frozen_reference_path"] = "artifacts/parse_compat/x.json"
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_threshold_unbound"
    }

def test_a_dropped_mandatory_gate_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    gates = record["package"]["mandatory_gates"]
    dropped = [gate for gate in gates if gate["gate_id"] == "h21_revision_label_residual"]
    assert len(dropped) == 1 and dropped[0]["blocks_s09"] is False
    record["package"]["mandatory_gates"] = [
        gate for gate in gates if gate["gate_id"] != "h21_revision_label_residual"
    ]
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_mandatory_gate_dropped"
    }

def test_an_unmet_gate_authorization_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["package"]["authorization"]["s09_authorized"] = True
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_unmet_gate_authorized"
    }

def test_a_lifecycle_authorization_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["package"]["authorization"]["gsd_lifecycle_authorizes"] = True
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_lifecycle_authorization"
    }

def test_a_renamed_body_residual_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    for gate in record["package"]["mandatory_gates"]:
        if gate["kind"] == "body_axis":
            gate["reason"] = "p1_localization_unknown"
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_body_residual_renamed"
    }

def test_a_pooled_resource_claim_is_refused(contract: dict) -> None:
    # The committed observed-with-source row is accepted: a measured same-n pair
    # is exactly what the receipt is allowed to report.
    assert _acceptance_package_violations(contract, _clean_bundle()) == set()

    record = _clean_bundle()
    record["resource"]["pairing"]["pooled"] = "allowed"
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_pooled_resource_claim"
    }

    record = _clean_bundle()
    record["resource"]["pairing"]["saving_claim"] = "allowed"
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_pooled_resource_claim"
    }

    # An observed row with no source is a number standing on nothing.
    record = _clean_bundle()
    for row in record["resource"]["reference_metrics"]:
        if row["state"] == "observed":
            row["source"] = None
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_pooled_resource_claim"
    }

    # An observed source must pin the carrier's pointer and sha256.
    record = _clean_bundle()
    for row in record["resource"]["reference_metrics"]:
        if row["state"] == "observed":
            del row["source"]["sha256"]
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_pooled_resource_claim"
    }

    # A typed absence must name a reason from the closed vocabulary.
    record = _clean_bundle()
    for row in record["resource"]["reference_metrics"]:
        if row["state"] == "not_observed":
            row["reason"] = "made_up_reason"
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_pooled_resource_claim"
    }

    # The declared bound-row count must equal the rows that actually bind a source.
    record = _clean_bundle()
    record["resource"]["residuals"]["reference_rows_bound"] = 99
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_pooled_resource_claim"
    }

def test_a_rollback_claim_without_byte_identity_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["rollback"]["corpus_claim"]["byte_identity_holds"] = False
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_rollback_without_byte_identity"
    }

    record = _clean_bundle()
    record["rollback"]["residuals"]["byte_identity_violations"] = 1
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_rollback_without_byte_identity"
    }

def test_a_product_coverage_claim_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["package"]["product_coverage_effect"] = "some"
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_coverage_claimed"
    }

    record = _clean_bundle()
    record["package"]["locks"]["deployed"] = "true"
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_coverage_claimed"
    }

def test_a_duplicate_carrier_binding_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["package"]["evidence"].append(copy.deepcopy(record["package"]["evidence"][0]))
    assert _acceptance_package_violations(contract, record) == {
        "acceptance_package_evidence_drift"
    }
