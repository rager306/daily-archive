"""M514-1lss2s/S06/T07 — the native-route receipt half of the progress check.

The S06 route receipt and the T06 same-n resource receipt are the first
source-bound evidence that one PDF->native path ends in a typed document result
on the declared 3-document / 202-page slice with *no Java* and no prior TEI,
measured against a ceiling frozen *before* the benchmark and paired with a pinned
same-n reference. The product ``java_independence`` axis legitimately reports
``not_measured`` while the production route is Java (D499), so a scoped,
diagnostic pair of receipts is exactly the progress the Governor could not see.
This file proves the half that now sees it is honest rather than optimistic:

* the rule's label set is exactly the domain's literal set, so a label the oracle
  can emit cannot be unregistered and a registered label cannot be unemittable;
* the committed positive fixture is clean, including the boundary case where the
  claim carrier is absent (absence is silence, never compliance);
* every committed negative record raises its own labelled violation and nothing
  else, and the matrix is required to cover all thirteen labels;
* the route and resource receipts are resolved from the live tree by pointer,
  recompute the declaration / cascade / corpus carrier hashes, the composite
  freeze revision and each measured reference pin, and validate clean;
* the receipts' byte pins match the live carriers, so the positive control cannot
  pass by being stale;
* the live route keeps the declared 3-document / 202-page population, the full
  P0..P9 stage boundary, a typed family matrix and a typed body axis;
* the real service path reports no ``native_route_*`` finding;
* the S04 ``native_family_receipt`` half is consumed by the real service path as
  well — the regression guard against the inert-half failure mode MEM1948 — so it
  is no longer vacuously silent;
* the half adds no dimension and no severity of its own (D492/D499).

The fixtures are bound to the *live* rule (their paths are the rule's own paths)
and carry the live resolved bundle, so the fixture exercises the real contract
rather than a copy of it. The negative fixture is serialized compactly because it
repeats a complete clean bundle per mutation; each record still raises exactly its
label. Nothing here executes a subprocess, reads a network resource, or writes to
the tree.
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
    _native_route_violations,
    effective_progress_contract,
    missing_progress_vocabulary,
)
from governor.application.trajectory_service import TrajectoryService  # noqa: E402
from governor.domain.absorption_progress import (  # noqa: E402
    NATIVE_ROUTE_VIOLATION_LABELS,
)

RULE_PATH = "scripts/governor/checks/grobid_absorption_progress.yaml"
SCHEMA_PATH = "data/parse/absorption_progress.schema.yaml"
BASELINE_PATH = "artifacts/parse_compat/absorption_progress_baseline.json"
RECEIPT_PATH = "artifacts/parse_compat/native_route_e2e.json"
RESOURCE_PATH = "artifacts/parse_compat/native_resource_same_n.json"
DECLARATION_MANIFEST = "data/parse/layout_bakeoff.yaml"
STAGE_SOURCE_MANIFEST = "data/parse/grobid_compatibility.yaml"
POSITIVE_FIXTURE = "governor/fixtures/absorption_progress/native_route.positive.json"
NEGATIVE_FIXTURE = "governor/fixtures/absorption_progress/native_route.negative.json"
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
def route_rule(contract: dict) -> dict:
    return contract["native_route_e2e"]


@pytest.fixture(scope="module")
def reader() -> FilesystemAbsorptionProgressReader:
    return FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, BASELINE_PATH)


@pytest.fixture(scope="module")
def live_evidence(contract: dict, reader: FilesystemAbsorptionProgressReader) -> dict:
    return reader.read_native_route_e2e(contract)


@pytest.fixture(scope="module")
def live_receipt() -> dict:
    return _load_json(RECEIPT_PATH)


@pytest.fixture(scope="module")
def live_resource() -> dict:
    return _load_json(RESOURCE_PATH)


# ── the closed vocabulary is one set, in two places ────────────────────────

def test_rule_registers_exactly_the_domain_literal_vocabulary(route_rule: dict) -> None:
    """The rule may not register a label the oracle cannot emit, or vice versa."""
    assert set(route_rule["violations"]) == set(NATIVE_ROUTE_VIOLATION_LABELS)
    assert len(NATIVE_ROUTE_VIOLATION_LABELS) == 13


def test_merged_contract_vocabulary_is_complete(contract: dict) -> None:
    assert missing_progress_vocabulary(contract) == []


def test_a_dropped_native_route_block_fails_closed(contract: dict) -> None:
    stripped = dict(contract)
    stripped.pop("native_route_e2e")
    assert "native_route_e2e" in missing_progress_vocabulary(stripped)


def test_the_native_route_half_is_not_a_competing_dimension(route_rule: dict) -> None:
    """It rides the existing `absorption_progress` check; it registers nothing new."""
    assert "dimension" not in route_rule
    assert "severity" not in route_rule
    for rel in (DIMENSIONS_PATH, SEVERITY_PATH):
        text = (_REPO_ROOT / rel).read_text(encoding="utf-8")
        assert "native_route_e2e" not in text, rel
        assert "absorption_progress" not in text, rel


# ── evidence resolution on the live tree ───────────────────────────────────

def test_live_receipts_are_resolved_by_pointer_and_validate_clean(
    contract: dict, live_evidence: dict
) -> None:
    assert live_evidence["available"] is True
    assert live_evidence["errors"] == []
    assert live_evidence["receipt_exists"] is True
    assert live_evidence["resource_receipt_exists"] is True
    assert isinstance(live_evidence["receipt"], dict)
    assert isinstance(live_evidence["resource_receipt"], dict)
    assert live_evidence["corpus_document_count"] == 3
    assert live_evidence["corpus_page_denominator"] == 202
    assert live_evidence["stage_ids"] == [f"P{index}" for index in range(10)]
    assert live_evidence["admission_stage"] == "P0"
    assert live_evidence["stages_in_slice"] == ["P1", "P2"]
    assert _native_route_violations(contract, live_evidence) == set()


def test_live_receipts_keep_the_frozen_carrier_pins(
    contract: dict, live_evidence: dict, live_receipt: dict, live_resource: dict
) -> None:
    """A stale pin must fail here, not be discovered as a stale positive control."""
    declaration_sha = "sha256:" + _bare_sha(DECLARATION_MANIFEST)
    assert live_evidence["declaration_sha256"] == declaration_sha
    assert live_receipt["declaration"]["revision_sha256"] == declaration_sha
    assert live_resource["declaration"]["revision_sha256"] == declaration_sha

    expected_freeze = contract["native_route_e2e"]["limit_policy"]["freeze_revision"]["value"]
    assert live_evidence["freeze_revision_recomputed"] == expected_freeze
    assert live_receipt["limits"]["freeze_revision"] == expected_freeze
    assert live_resource["limits"]["freeze_revision"] == expected_freeze

    assert live_evidence["stage_source_sha256"] == "sha256:" + _bare_sha(STAGE_SOURCE_MANIFEST)

    pins = live_evidence["resource_reference_pins"]
    assert pins
    for manifest, pin in pins.items():
        assert pin["declared_sha256"] == _bare_sha(manifest)
        assert pin["recomputed_sha256"] == "sha256:" + _bare_sha(manifest)


def test_live_route_keeps_the_declared_slice_and_typed_cells(
    contract: dict, live_evidence: dict, live_receipt: dict, live_resource: dict
) -> None:
    """3 documents / 202 pages, a total P0..P9 stance per document, typed families."""
    expected = contract["native_route_e2e"]["expected"]
    corpus_pins = {entry["id"]: entry for entry in live_evidence["corpus_documents"]}
    assert set(corpus_pins) == {"hub-2507.19457", "rlm-2512.24601", "reca-2605.26525v1"}
    assert sum(entry["pages"] for entry in corpus_pins.values()) == 202

    documents = live_receipt["documents"]
    assert len(documents) == 3
    assert {document["id"] for document in documents} == set(corpus_pins)
    for document in documents:
        assert document["sha256"] == corpus_pins[document["id"]]["sha256"]
        assert document["pages"] == corpus_pins[document["id"]]["pages"]
        for cell in document["stages"]:
            assert cell["stance"] in expected["stage_stance_vocabulary"], cell
            if cell["stage"] in live_evidence["stages_out_of_slice"]:
                assert cell["stance"] == expected["out_of_slice_stance"], cell
            if cell["stage"] == live_evidence["admission_stage"]:
                assert cell["stance"] == expected["admission_stance"], cell
        assert {cell["stage"] for cell in document["stages"]} == set(
            live_evidence["stage_ids"]
        )
        for cell in document["families"]:
            assert cell["disposition"] in expected["family_disposition_vocabulary"], cell
            assert cell["disposition"] != expected["forbidden_family_token"], cell
            if cell["disposition"] == "unmet":
                assert cell["reason"] in expected["family_unmet_reason_vocabulary"], cell
            if cell["disposition"] == "refused":
                assert cell["reason"] in expected["family_refusal_reason_vocabulary"], cell

    for population in live_receipt["populations"]:
        assert population["population"] in expected["population_vocabulary"]
        assert population["document_count"] == len(population["documents"])
        assert population["page_denominator"] == sum(
            document["pages"] for document in population["documents"]
        )
        body = population["body"] if "body" in population else None
        assert body is None  # body lives on documents and residuals, not populations

    body = live_receipt["residuals"]["body"]
    assert body["plane_preservation"] in expected["body_verdict_vocabulary"]
    assert body["plane_gap_reason"] in expected["body_reason_vocabulary"]

    resource_corpus = live_resource["corpus"]
    assert resource_corpus["document_count"] == 3
    assert resource_corpus["page_denominator"] == 202
    assert {
        (entry["id"], entry["sha256"]) for entry in resource_corpus["documents"]
    } == {(entry["id"], entry["sha256"]) for entry in live_evidence["corpus_documents"]}


def test_live_tree_service_reports_no_native_route_finding() -> None:
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    flags = service._check_absorption_progress()
    leaked = [
        f for f in flags
        if "native_route" in (f.flag or "") or "native_route" in (f.evidence or "")
    ]
    assert leaked == [], [f.flag for f in flags]
    assert flags == [], [f.flag for f in flags]


def test_the_service_path_consumes_the_route_observation(monkeypatch) -> None:
    """The route half must fail closed through the real service, not only when
    called directly.

    This is the regression guard against the *inert half* failure mode the S04
    threat review found (MEM1948): a sibling block whose oracle is never fed by
    the service reads as clean on every tree, so the half could be deleted or
    silently regress without any check noticing. Patching the reader to return a
    claim-without-evidence bundle must raise the composed finding through
    `_check_absorption_progress`, which proves the observation is consumed by the
    live path.
    """
    def broken(self, contract):  # noqa: ANN001, ARG001
        return {"available": True, "receipt": None}

    monkeypatch.setattr(
        FilesystemAbsorptionProgressReader, "read_native_route_e2e", broken
    )
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    flags = service._check_absorption_progress()
    broken_flags = [f for f in flags if f.flag == "absorption_progress_broken"]
    assert broken_flags, [f.flag for f in flags]
    assert "native_route_claim_without_receipt" in broken_flags[0].evidence


def test_the_service_path_consumes_the_family_observation(monkeypatch) -> None:
    """The S04 `native_family_receipt` half was written but never wired into the
    observation map, so it was inert (MEM1948). This guards the repair: the live
    service must consume the family observation, so a broken family bundle must
    surface as a composed finding.
    """
    def broken(self, contract):  # noqa: ANN001, ARG001
        return {"available": True, "receipt": None}

    monkeypatch.setattr(
        FilesystemAbsorptionProgressReader, "read_native_family_receipt", broken
    )
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    flags = service._check_absorption_progress()
    broken_flags = [f for f in flags if f.flag == "absorption_progress_broken"]
    assert broken_flags, [f.flag for f in flags]
    assert "native_family_claim_without_receipt" in broken_flags[0].evidence


# ── fixtures: positive clean, negative matrix covers the closed set ────────

def test_positive_fixture_is_clean(contract: dict) -> None:
    records = _load_json(POSITIVE_FIXTURE)
    assert isinstance(records, list) and records
    for record in records:
        assert _native_route_violations(contract, record) == set()


def test_negative_fixture_triggers_every_registered_label(contract: dict) -> None:
    records = _load_json(NEGATIVE_FIXTURE)
    assert isinstance(records, list) and records
    observed: set[str] = set()
    for entry in records:
        label = entry["violation"]
        violations = _native_route_violations(contract, entry["record"])
        observed |= violations
        assert label in violations, (label, sorted(violations))
        # Each record is a clean bundle with exactly one mutation, so the target
        # label is the only finding: a mutation that trips a second label is a
        # real overlap and must be made explicit rather than tolerated.
        assert violations == {label}, (label, sorted(violations))
    assert observed <= set(NATIVE_ROUTE_VIOLATION_LABELS)
    assert set(NATIVE_ROUTE_VIOLATION_LABELS) <= observed


def test_negative_fixture_registration_matches_the_literal_set(route_rule: dict) -> None:
    required = set(
        route_rule["fixtures"]["negative"]["expect"]["require_violations"]
    )
    assert required == set(NATIVE_ROUTE_VIOLATION_LABELS)


def test_absent_evidence_is_unverified_never_refuted(contract: dict) -> None:
    """Non-vacuity boundary: silence is not compliance, and it is not a finding."""
    assert _native_route_violations(contract, None) == set()
    assert _native_route_violations(contract, {}) == set()
    assert _native_route_violations(contract, {"available": False}) == set()


def test_the_declaration_authority_gates_availability(contract: dict) -> None:
    """A tree without the S03 authority is silence, not a claim and not a finding."""
    reader = FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, BASELINE_PATH)
    rule = contract["native_route_e2e"]
    absent = reader.read_native_route_e2e(
        {
            "native_route_e2e": {
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
    assert _native_route_violations(contract, absent) == set()


def test_a_resolvable_claim_without_a_receipt_is_refused(contract: dict) -> None:
    """The shape R141 forbids: a declared route receipt with no evidence."""
    records = _load_json(NEGATIVE_FIXTURE)
    by_label = {entry["violation"]: entry["record"] for entry in records}
    missing = by_label["native_route_claim_without_receipt"]
    assert missing["available"] is True
    assert missing["receipt"] is None
    assert _native_route_violations(contract, missing) == {
        "native_route_claim_without_receipt"
    }


# ── mutation semantics the oracle must refuse ──────────────────────────────

def _clean_bundle() -> dict:
    return copy.deepcopy(_load_json(POSITIVE_FIXTURE)[0])


def test_a_stale_route_or_freeze_pin_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipt"]["declaration"]["revision_sha256"] = "sha256:" + "9" * 64
    assert _native_route_violations(contract, record) == {"native_route_receipt_stale"}

    record = _clean_bundle()
    record["receipt"]["limits"]["freeze_revision"] = "sha256:" + "9" * 64
    assert _native_route_violations(contract, record) == {"native_route_receipt_stale"}


def test_a_stage_outside_the_slice_counted_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    out_of_slice = set(record["stages_out_of_slice"])
    for document in record["receipt"]["documents"]:
        for cell in document["stages"]:
            if cell["stage"] in out_of_slice:
                cell["stance"] = "native_measured"
                break
        break
    assert _native_route_violations(contract, record) == {
        "native_route_stage_outside_slice_counted"
    }


def test_an_unbound_resource_pair_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["resource_receipt"] = None
    assert _native_route_violations(contract, record) == {
        "native_route_resource_pair_unbound"
    }

    record = _clean_bundle()
    for manifest, pin in record["resource_reference_pins"].items():
        pin["recomputed_sha256"] = "sha256:" + "9" * 64
    assert _native_route_violations(contract, record) == {
        "native_route_resource_pair_unbound"
    }


def test_an_untyped_metric_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipt"]["populations"][0]["metrics"].append(
        {"id": "native_route_mystery", "direction": "ceiling", "value": 1}
    )
    assert _native_route_violations(contract, record) == {"native_route_metric_untyped"}


def test_an_untyped_body_gap_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipt"]["residuals"]["body"]["plane_preservation"] = "unknown"
    assert _native_route_violations(contract, record) == {
        "native_route_body_gap_untyped"
    }


def test_a_required_resource_receipt_absent_is_unbound(contract: dict) -> None:
    record = _clean_bundle()
    record["resource_receipt"] = None
    record["resource_receipt_exists"] = False
    assert _native_route_violations(contract, record) == {
        "native_route_resource_pair_unbound"
    }
