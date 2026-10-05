"""M514-1lss2s/S03/T06 — the Java-free slice half of the absorption progress check.

The product ``java_independence`` axis legitimately reports ``not_measured`` while
the production parse route is Java (D499), so a *scoped, diagnostic* Java-free
receipt is exactly the progress the Governor could not see. This file proves the
half that now sees it is honest rather than optimistic:

* the rule's label set is exactly the domain's literal set, so a label the oracle
  can emit cannot be unregistered and a registered label cannot be unemittable;
* the committed positive fixture is clean, including the boundary case where the
  claim carrier is absent (absence is silence, never compliance);
* every committed negative record raises its own labelled violation, and the
  matrix is required to cover all eight labels;
* the receipt is resolved from the live tree by pointer, recomputes both source
  hashes, and validates clean through the pure oracle;
* the receipt's byte pins match the live declaration and cascade sources, so the
  positive control cannot pass by being stale;
* the real service path reports no ``java_free_slice_*`` finding;
* the half adds no dimension and no severity of its own (D492/D499).

The fixtures are synthetic but bound to the *live* rule: their declaration and
stage-id manifest paths are the rule's own paths, so the fixture exercises the
real contract rather than a copy of it. Nothing here executes a subprocess, reads
a network resource, or writes to the tree.
"""

from __future__ import annotations

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
    _java_free_slice_violations,
    effective_progress_contract,
    missing_progress_vocabulary,
)
from governor.application.trajectory_service import TrajectoryService  # noqa: E402
from governor.domain.absorption_progress import (  # noqa: E402
    JAVA_FREE_SLICE_VIOLATION_LABELS,
)

RULE_PATH = "scripts/governor/checks/grobid_absorption_progress.yaml"
SCHEMA_PATH = "data/parse/absorption_progress.schema.yaml"
BASELINE_PATH = "artifacts/parse_compat/absorption_progress_baseline.json"
RECEIPT_PATH = "artifacts/parse_compat/java_free_vertical_slice.json"
DECLARATION_MANIFEST = "data/parse/layout_bakeoff.yaml"
STAGE_ID_MANIFEST = "data/parse/grobid_compatibility.yaml"
POSITIVE_FIXTURE = "governor/fixtures/absorption_progress/java_free_slice.positive.json"
NEGATIVE_FIXTURE = "governor/fixtures/absorption_progress/java_free_slice.negative.json"
DIMENSIONS_PATH = "scripts/governor/checks/dimensions.yaml"
SEVERITY_PATH = "scripts/governor/checks/assurance_severity.yaml"


def _load_yaml(rel: str) -> dict:
    return yaml.safe_load((_REPO_ROOT / rel).read_text(encoding="utf-8"))


def _load_json(rel: str):
    return json.loads((_REPO_ROOT / rel).read_text(encoding="utf-8"))


def _sha256(rel: str) -> str:
    return "sha256:" + hashlib.sha256((_REPO_ROOT / rel).read_bytes()).hexdigest()


@pytest.fixture(scope="module")
def rule_contract() -> dict:
    return _load_yaml(RULE_PATH)


@pytest.fixture(scope="module")
def contract(rule_contract: dict) -> dict:
    return effective_progress_contract(_load_yaml(SCHEMA_PATH), rule_contract)


@pytest.fixture(scope="module")
def java_free_rule(contract: dict) -> dict:
    return contract["java_free_slice"]


# ── the closed vocabulary is one set, in two places ────────────────────────

def test_rule_registers_exactly_the_domain_literal_vocabulary(java_free_rule: dict) -> None:
    """The rule may not register a label the oracle cannot emit, or vice versa."""
    assert set(java_free_rule["violations"]) == set(JAVA_FREE_SLICE_VIOLATION_LABELS)
    assert len(JAVA_FREE_SLICE_VIOLATION_LABELS) == 8


def test_merged_contract_vocabulary_is_complete(contract: dict) -> None:
    assert missing_progress_vocabulary(contract) == []


def test_the_java_free_half_is_not_a_competing_dimension(java_free_rule: dict) -> None:
    """It rides the existing `absorption_progress` check; it registers nothing new."""
    assert "dimension" not in java_free_rule
    assert "severity" not in java_free_rule
    for rel in (DIMENSIONS_PATH, SEVERITY_PATH):
        text = (_REPO_ROOT / rel).read_text(encoding="utf-8")
        assert "java_free_slice" not in text, rel
        assert "absorption_progress" not in text, rel


# ── evidence resolution on the live tree ───────────────────────────────────

def test_live_receipt_is_resolved_by_pointer_and_validates_clean(contract: dict) -> None:
    reader = FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, BASELINE_PATH)
    evidence = reader.read_java_free_slice(contract)

    assert isinstance(evidence, dict)
    assert evidence["available"] is True
    assert evidence["declaration"] is not None
    assert evidence["receipt"] is not None
    assert evidence["receipt_exists"] is True
    assert evidence["errors"] == []
    assert evidence["cascade_stage_ids"] == [
        "P0", "P1", "P2", "P3", "P4", "P5", "P6", "P7", "P8", "P9",
    ]
    # The route now carries its own twelve-document corpus
    # (`native_route_corpus`); the rule's java_free_slice corpus pointer and the
    # committed receipt move with it in the sibling S03 tasks (T02B/T04).
    assert len(evidence["corpus_documents"]) == 12
    assert _java_free_slice_violations(contract, evidence) == set()


def test_committed_receipt_pins_match_the_live_source_bytes() -> None:
    """A stale pin must fail here, not be discovered as a stale positive control."""
    receipt = _load_json(RECEIPT_PATH)
    assert receipt["declaration"]["manifest"] == DECLARATION_MANIFEST
    assert receipt["declaration"]["pointer"] == "java_free_vertical_slice"
    assert receipt["declaration"]["revision_sha256"] == _sha256(DECLARATION_MANIFEST)
    assert receipt["stage_id_source"]["manifest"] == STAGE_ID_MANIFEST
    assert receipt["stage_id_source"]["pointer"] == "defaults.cascade_stages"
    assert receipt["stage_id_source"]["revision_sha256"] == _sha256(STAGE_ID_MANIFEST)


def test_live_tree_service_reports_no_java_free_slice_finding() -> None:
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    flags = service._check_absorption_progress()
    leaked = [
        f for f in flags
        if "java_free_slice" in (f.flag or "") or "java_free_slice" in (f.evidence or "")
    ]
    assert leaked == [], [f.flag for f in flags]
    assert flags == [], [f.flag for f in flags]


# ── fixtures: positive clean, negative matrix covers the closed set ────────

def test_positive_fixture_is_clean(contract: dict) -> None:
    records = _load_json(POSITIVE_FIXTURE)
    assert isinstance(records, list) and records
    for record in records:
        assert _java_free_slice_violations(contract, record) == set(), record.get("case")


def test_negative_fixture_triggers_every_registered_label(contract: dict) -> None:
    records = _load_json(NEGATIVE_FIXTURE)
    assert isinstance(records, list) and records
    observed: set[str] = set()
    for entry in records:
        label = entry["violation"]
        violations = _java_free_slice_violations(contract, entry["record"])
        observed |= violations
        assert label in violations, (label, sorted(violations))
    assert observed <= set(JAVA_FREE_SLICE_VIOLATION_LABELS)
    assert set(JAVA_FREE_SLICE_VIOLATION_LABELS) <= observed


def test_negative_fixture_registration_matches_the_literal_set(java_free_rule: dict) -> None:
    required = set(
        java_free_rule["fixtures"]["negative"]["expect"]["require_violations"]
    )
    assert required == set(JAVA_FREE_SLICE_VIOLATION_LABELS)


def test_absent_evidence_is_unverified_never_refuted(contract: dict) -> None:
    """Non-vacuity boundary: silence is not compliance, and it is not a finding."""
    assert _java_free_slice_violations(contract, None) == set()
    assert _java_free_slice_violations(contract, {}) == set()

    reader = FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, BASELINE_PATH)
    rule = contract["java_free_slice"]
    absent_carrier = reader.read_java_free_slice(
        {"java_free_slice": {**rule, "declaration": {"manifest": "data/parse/absent.yaml",
                                                     "pointer": "java_free_vertical_slice"}}}
    )
    assert absent_carrier is not None
    assert absent_carrier["available"] is False
    assert _java_free_slice_violations(contract, absent_carrier) == set()


def test_a_resolvable_claim_without_a_receipt_is_refused(contract: dict) -> None:
    """The shape R141 forbids: a declared Java-free slice with no evidence."""
    records = _load_json(NEGATIVE_FIXTURE)
    by_label = {entry["violation"]: entry["record"] for entry in records}
    missing = by_label["java_free_slice_claim_without_receipt"]
    assert missing["available"] is True
    assert missing["receipt"] is None
    assert _java_free_slice_violations(contract, missing) == {
        "java_free_slice_claim_without_receipt"
    }
