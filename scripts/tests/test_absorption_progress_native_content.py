"""M514-1lss2s/S05/T07 — the native content receipt half of the progress check.

The S05 content receipt is the first source-bound evidence that the native body,
section / reading-order, visual and callout bands measured the *declared*
3-document slice — with a total group partition, a typed per-document family
matrix and the T01 body-character axis — against an *independent* oracle. The
product ``java_independence`` axis legitimately reports ``not_measured`` while the
production route is Java (D499), so a scoped, diagnostic receipt is exactly the
progress the Governor could not see. This file proves the half that now sees it is
honest rather than optimistic:

* the rule's label set is exactly the domain's literal set, so a label the oracle
  can emit cannot be unregistered and a registered label cannot be unemittable;
* the committed positive fixture is clean, including the boundary case where the
  claim carrier is absent (absence is silence, never compliance);
* every committed negative record raises its own labelled violation and nothing
  else, and the matrix is required to cover all eleven labels;
* the receipt is resolved from the live tree by pointer, recomputes the
  declaration / oracle / segmentation carrier hashes and the preserved-TEI
  fixture pins, and validates clean through the pure oracle;
* the receipt's byte pins match the live carriers, so the positive control cannot
  pass by being stale;
* the live receipt keeps the declared 3-document / 202-page population, the total
  group partition and the per-family typed matrix, including the *unmet* required
  families (typed, never ``out_of_scope``) whose independent oracle still carries
  positive rows;
* the real service path reports no ``native_content_*`` finding;
* the half adds no dimension and no severity of its own (D492/D499).

The fixtures are synthetic but bound to the *live* rule: their declaration and
receipt paths are the rule's own paths, so the fixture exercises the real contract
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
    _native_content_receipt_violations,
    effective_progress_contract,
    missing_progress_vocabulary,
)
from governor.application.trajectory_service import TrajectoryService  # noqa: E402
from governor.domain.absorption_progress import (  # noqa: E402
    NATIVE_CONTENT_VIOLATION_LABELS,
)

RULE_PATH = "scripts/governor/checks/grobid_absorption_progress.yaml"
SCHEMA_PATH = "data/parse/absorption_progress.schema.yaml"
BASELINE_PATH = "artifacts/parse_compat/absorption_progress_baseline.json"
RECEIPT_PATH = "artifacts/parse_compat/native_content_receipt.json"
DECLARATION_MANIFEST = "data/parse/layout_bakeoff.yaml"
ORACLE_MANIFEST = "crates/da-parse/tests/fixtures/preserved/provenance.yaml"
SEGMENTATION_MANIFEST = "data/parse/document_segmentation.yaml"
SEGMENTATION_SOURCE = "artifacts/parse_compat/document_segmentation_oracle_source.json"
POSITIVE_FIXTURE = "governor/fixtures/absorption_progress/native_content.positive.json"
NEGATIVE_FIXTURE = "governor/fixtures/absorption_progress/native_content.negative.json"
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
def content_rule(contract: dict) -> dict:
    return contract["native_content_receipt"]


@pytest.fixture(scope="module")
def reader() -> FilesystemAbsorptionProgressReader:
    return FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, BASELINE_PATH)


@pytest.fixture(scope="module")
def live_evidence(contract: dict, reader: FilesystemAbsorptionProgressReader) -> dict:
    return reader.read_native_content_receipt(contract)


@pytest.fixture(scope="module")
def live_receipt() -> dict:
    return _load_json(RECEIPT_PATH)


# ── the closed vocabulary is one set, in two places ────────────────────────

def test_rule_registers_exactly_the_domain_literal_vocabulary(content_rule: dict) -> None:
    """The rule may not register a label the oracle cannot emit, or vice versa."""
    assert set(content_rule["violations"]) == set(NATIVE_CONTENT_VIOLATION_LABELS)
    assert len(NATIVE_CONTENT_VIOLATION_LABELS) == 11


def test_merged_contract_vocabulary_is_complete(contract: dict) -> None:
    assert missing_progress_vocabulary(contract) == []


def test_a_dropped_native_content_block_fails_closed(contract: dict) -> None:
    stripped = dict(contract)
    stripped.pop("native_content_receipt")
    assert "native_content_receipt" in missing_progress_vocabulary(stripped)


def test_the_native_content_half_is_not_a_competing_dimension(content_rule: dict) -> None:
    """It rides the existing `absorption_progress` check; it registers nothing new."""
    assert "dimension" not in content_rule
    assert "severity" not in content_rule
    for rel in (DIMENSIONS_PATH, SEVERITY_PATH):
        text = (_REPO_ROOT / rel).read_text(encoding="utf-8")
        assert "native_content_receipt" not in text, rel
        assert "absorption_progress" not in text, rel


# ── evidence resolution on the live tree ───────────────────────────────────

def test_live_receipt_is_resolved_by_pointer_and_validates_clean(
    contract: dict, live_evidence: dict
) -> None:
    assert live_evidence["available"] is True
    assert live_evidence["declaration"] is not None
    assert live_evidence["errors"] == []
    assert live_evidence["receipt_exists"] is True
    assert isinstance(live_evidence["receipt"], dict)
    assert live_evidence["corpus_document_count"] == 3
    assert live_evidence["corpus_page_denominator"] == 202
    assert set(live_evidence["oracle_fixture_pins"]) and all(
        pin.get("recomputed_sha256") for pin in live_evidence["oracle_fixture_pins"].values()
    )
    assert _native_content_receipt_violations(contract, live_evidence) == set()


def test_live_receipt_pins_match_the_frozen_carrier_bytes(
    contract: dict, live_evidence: dict, live_receipt: dict
) -> None:
    """A stale pin must fail here, not be discovered as a stale positive control."""
    assert live_receipt["declaration"]["manifest"] == DECLARATION_MANIFEST
    assert live_receipt["declaration"]["pointer"] == "java_free_vertical_slice"
    assert (
        live_receipt["declaration"]["revision_sha256"]
        == "sha256:" + _bare_sha(DECLARATION_MANIFEST)
    )
    assert live_receipt["oracle"]["manifest"] == ORACLE_MANIFEST
    assert (
        live_receipt["oracle"]["revision_sha256"] == "sha256:" + _bare_sha(ORACLE_MANIFEST)
    )
    assert (
        live_receipt["oracle"]["segmentation_revision_sha256"]
        == "sha256:" + _bare_sha(SEGMENTATION_MANIFEST)
    )
    assert (
        live_receipt["oracle"]["segmentation_source_sha256"]
        == "sha256:" + _bare_sha(SEGMENTATION_SOURCE)
    )

    provenance = _load_yaml(ORACLE_MANIFEST)
    pins = {entry["path"]: entry["sha256"] for entry in provenance["fixtures"]}
    for document in live_receipt["oracle"]["documents"]:
        assert pins[document["fixture_path"]] == document["sha256"]

    assert live_evidence["declaration_sha256"] == "sha256:" + _bare_sha(DECLARATION_MANIFEST)
    assert live_evidence["oracle_sha256"] == "sha256:" + _bare_sha(ORACLE_MANIFEST)
    assert live_evidence["segmentation_manifest_sha256"] == "sha256:" + _bare_sha(
        SEGMENTATION_MANIFEST
    )
    assert live_evidence["segmentation_source_sha256"] == "sha256:" + _bare_sha(
        SEGMENTATION_SOURCE
    )


def test_live_receipt_keeps_the_declared_population_and_total_partition(
    live_receipt: dict
) -> None:
    corpus = _load_yaml(DECLARATION_MANIFEST)["corpus"]
    pinned = {entry["id"]: entry["sha256"] for entry in corpus["documents"]}

    populations = live_receipt["populations"]
    assert len(populations) >= 1
    declared: dict[str, str] = {}
    total_documents = 0
    total_pages = 0
    for population in populations:
        assert population["document_count"] == len(population["documents"])
        assert population["page_denominator"] == sum(
            document["pages"] for document in population["documents"]
        )
        assert len(population["matrix"]) == len(population["documents"])
        for document in population["documents"]:
            declared[document["id"]] = document["sha256"]
        total_documents += population["document_count"]
        total_pages += population["page_denominator"]
    assert declared == pinned
    assert total_documents == corpus["corpus_n"] == 3
    assert total_pages == corpus["page_denominator"] == 202

    partition = live_receipt["partition"]
    assert partition["state"] == "partitioned"
    assert partition["declared_document_count"] == 3
    assigned = {assignment["document_id"] for assignment in partition["assignments"]}
    assert assigned == set(pinned)
    assert live_receipt["pooling"]["group_strata_pooled"] is False
    assert live_receipt["redaction"] and not any(live_receipt["redaction"].values())


def test_live_matrix_keeps_the_required_families_typed_and_the_oracle_non_vacuous(
    content_rule: dict, live_receipt: dict
) -> None:
    """A required family may be ``unmet``, but never ``out_of_scope`` and never met
    by a vacuous oracle."""
    expected = content_rule["expected"]
    dispositions = set(expected["family_dispositions"])
    reasons = set(expected["unmet_reasons"])
    required = set(expected["required_families"])

    rows = [row for population in live_receipt["populations"] for row in population["matrix"]]
    assert rows
    for row in rows:
        assert {cell["family"] for cell in row["families"]} == set(expected["families"])
        for cell in row["families"]:
            assert cell["disposition"] in dispositions, cell
            assert cell["disposition"] != expected["forbidden_family_token"], cell
            if cell["disposition"] == "unmet":
                assert cell["unmet_reason"] in reasons, cell
                assert cell["observed_count"] is None
            if cell["disposition"] == "measured":
                assert cell["unmet_reason"] is None
                assert cell["observed_count"] is not None

    for family in required:
        positive = sum(
            cell["oracle_positive_rows"]
            for row in rows
            for cell in row["families"]
            if cell["family"] == family
        )
        assert positive > 0, f"the oracle is vacant for required family {family}"


def test_live_tree_service_reports_no_native_content_finding() -> None:
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    flags = service._check_absorption_progress()
    leaked = [
        f for f in flags
        if "native_content" in (f.flag or "") or "native_content" in (f.evidence or "")
    ]
    assert leaked == [], [f.flag for f in flags]
    assert flags == [], [f.flag for f in flags]


def test_the_service_path_consumes_the_observation(monkeypatch) -> None:
    """The half must fail closed through the real service, not only when called
    directly.

    This is the regression guard against the *inert half* failure mode the S04
    threat review found: a sibling block whose oracle is never fed by the service
    reads as clean on every tree, so the half could be deleted or silently
    regress without any check noticing. Patching the reader to return a
    claim-without-evidence bundle must raise the composed finding through
    `_check_absorption_progress`, which proves the observation is consumed by the
    live path.
    """
    def broken(self, contract):  # noqa: ANN001, ARG001
        return {"available": True, "receipt": None}

    monkeypatch.setattr(
        FilesystemAbsorptionProgressReader, "read_native_content_receipt", broken
    )
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    flags = service._check_absorption_progress()
    broken_flags = [f for f in flags if f.flag == "absorption_progress_broken"]
    assert broken_flags, [f.flag for f in flags]
    assert "native_content_claim_without_receipt" in broken_flags[0].evidence


# ── fixtures: positive clean, negative matrix covers the closed set ────────

def test_positive_fixture_is_clean(contract: dict) -> None:
    records = _load_json(POSITIVE_FIXTURE)
    assert isinstance(records, list) and records
    for record in records:
        assert _native_content_receipt_violations(contract, record) == set()


def test_negative_fixture_triggers_every_registered_label(contract: dict) -> None:
    records = _load_json(NEGATIVE_FIXTURE)
    assert isinstance(records, list) and records
    observed: set[str] = set()
    for entry in records:
        label = entry["violation"]
        violations = _native_content_receipt_violations(contract, entry["record"])
        observed |= violations
        assert label in violations, (label, sorted(violations))
        # Each record is a clean bundle with exactly one mutation, so the target
        # label is the only finding: a mutation that trips a second label is a
        # real overlap and must be made explicit rather than tolerated.
        assert violations == {label}, (label, sorted(violations))
    assert observed <= set(NATIVE_CONTENT_VIOLATION_LABELS)
    assert set(NATIVE_CONTENT_VIOLATION_LABELS) <= observed


def test_negative_fixture_registration_matches_the_literal_set(content_rule: dict) -> None:
    required = set(
        content_rule["fixtures"]["negative"]["expect"]["require_violations"]
    )
    assert required == set(NATIVE_CONTENT_VIOLATION_LABELS)


def test_absent_evidence_is_unverified_never_refuted(contract: dict) -> None:
    """Non-vacuity boundary: silence is not compliance, and it is not a finding."""
    assert _native_content_receipt_violations(contract, None) == set()
    assert _native_content_receipt_violations(contract, {}) == set()
    assert _native_content_receipt_violations(contract, {"available": False}) == set()


def test_the_declaration_authority_gates_availability(contract: dict) -> None:
    """A tree without the S03 authority is silence, not a claim and not a finding."""
    reader = FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, BASELINE_PATH)
    rule = contract["native_content_receipt"]
    absent = reader.read_native_content_receipt(
        {
            "native_content_receipt": {
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
    assert _native_content_receipt_violations(contract, absent) == set()


def test_a_resolvable_claim_without_a_receipt_is_refused(contract: dict) -> None:
    """The shape R141 forbids: a declared content receipt with no evidence."""
    records = _load_json(NEGATIVE_FIXTURE)
    by_label = {entry["violation"]: entry["record"] for entry in records}
    missing = by_label["native_content_claim_without_receipt"]
    assert missing["available"] is True
    assert missing["receipt"] is None
    assert _native_content_receipt_violations(contract, missing) == {
        "native_content_claim_without_receipt"
    }


# ── mutation semantics the oracle must refuse ──────────────────────────────

def _clean_bundle() -> dict:
    return copy.deepcopy(_load_json(POSITIVE_FIXTURE)[0])


def test_a_stale_receipt_or_sidecar_pin_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipt"]["declaration"]["revision_sha256"] = "sha256:" + "9" * 64
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_receipt_stale"
    }

    record = _clean_bundle()
    record["receipt"]["oracle"]["segmentation_source_sha256"] = "sha256:" + "9" * 64
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_receipt_stale"
    }

    record = _clean_bundle()
    fixture_path = next(iter(record["receipt"]["oracle"]["documents"]))["fixture_path"]
    record["oracle_fixture_pins"][fixture_path]["sha256"] = "9" * 64
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_receipt_stale"
    }


def test_a_shrunk_group_partition_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipt"]["partition"]["assignments"] = record["receipt"]["partition"][
        "assignments"
    ][:-1]
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_group_partition_untyped"
    }


def test_a_pooled_group_strata_flag_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipt"]["pooling"]["group_strata_pooled"] = True
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_group_partition_untyped"
    }


def test_a_group_without_its_typed_source_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    assignment = record["receipt"]["partition"]["assignments"][0]
    assignment["source"] = {"kind": "bytes_absent"}
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_group_partition_untyped"
    }


def test_a_required_family_moved_to_out_of_scope_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    cell = record["receipt"]["populations"][0]["matrix"][0]["families"][0]
    assert cell["family"] == "figure"
    cell["disposition"] = "out_of_scope"
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_required_family_out_of_scope"
    }


def test_an_unmet_family_without_its_typed_reason_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    cell = record["receipt"]["populations"][0]["matrix"][0]["families"][0]
    cell["unmet_reason"] = None
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_receipt_population_mismatch"
    }


def test_an_absent_only_oracle_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    for row in record["receipt"]["populations"][0]["matrix"]:
        for cell in row["families"]:
            cell["oracle_positive_rows"] = 0
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_oracle_absence_only"
    }


def test_a_self_scoring_oracle_or_a_wrong_carrier_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipt"]["oracle"]["same_generator_refused"] = False
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_oracle_not_independent"
    }

    record = _clean_bundle()
    record["receipt"]["oracle"]["manifest"] = "crates/da-parse/tests/fixtures/other.yaml"
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_oracle_not_independent"
    }


def test_a_guard_less_zero_or_a_java_call_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipt"]["guard"]["identity_isolated"] = False
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_guard_not_installed"
    }

    record = _clean_bundle()
    record["receipt"]["guard"]["observed_java_calls"] = 1
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_java_calls_nonzero"
    }


def test_a_reopened_promotion_lock_threshold_or_redaction_flag_is_refused(
    contract: dict
) -> None:
    record = _clean_bundle()
    record["receipt"]["locks"]["deployed"] = "true"
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_product_coverage_claimed"
    }

    record = _clean_bundle()
    record["receipt"]["thresholds"] = "accepted"
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_product_coverage_claimed"
    }

    record = _clean_bundle()
    record["receipt"]["redaction"]["raw_pdf"] = True
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_product_coverage_claimed"
    }


def test_an_unresolved_body_gap_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    body = record["receipt"]["populations"][0]["matrix"][0]["body"]
    body["plane_preservation"] = "content_loss"
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_body_gap_unresolved"
    }

    record = _clean_bundle()
    body = record["receipt"]["populations"][0]["matrix"][0]["body"]
    body["plane_absent_units"] = 3
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_body_gap_unresolved"
    }

    record = _clean_bundle()
    body = record["receipt"]["populations"][0]["matrix"][0]["body"]
    body["body_reason"] = "whatever"
    assert _native_content_receipt_violations(contract, record) == {
        "native_content_body_gap_unresolved"
    }
