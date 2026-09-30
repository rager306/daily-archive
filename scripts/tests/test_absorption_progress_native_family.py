"""M514-1lss2s/S04/T06 — the native family receipts half of the progress check.

The S04 native family receipts are the first source-bound evidence that the real
native parsers — not the TEI-derived copies of the Java carrier — produced header
and bibliography atoms on the declared 3-document slice, and that the 79 typed
baseline exclusions keep their denominator. The product ``java_independence`` axis
legitimately reports ``not_measured`` while the production route is Java (D499),
so a scoped, diagnostic receipt is exactly the progress the Governor could not
see. This file proves the half that now sees it is honest rather than optimistic:

* the rule's label set is exactly the domain's literal set, so a label the oracle
  can emit cannot be unregistered and a registered label cannot be unemittable;
* the committed positive fixture is clean, including the boundary case where the
  claim carrier is absent (absence is silence, never compliance);
* every committed negative record raises its own labelled violation and nothing
  else, and the matrix is required to cover all nine labels;
* the receipts are resolved from the live tree by pointer, recompute both source
  hashes and validate clean through the pure oracle;
* the receipts' byte pins match the live frozen carriers, so the positive control
  cannot pass by being stale, and the exclusion denominator still partitions 79;
* an independent oracle is bound to the preserved-TEI pins, not to the native
  pipeline output;
* the real service path reports no ``native_family_*`` finding;
* the half adds no dimension and no severity of its own (D492/D499).

The fixtures are synthetic but bound to the *live* rule: their carrier paths are
the rule's own paths, so the fixture exercises the real contract rather than a
copy of it. The negative fixture is serialized compactly because it repeats a
complete clean bundle per mutation; each record still raises exactly its label.
Nothing here executes a subprocess, reads a network resource, or writes to the
tree.
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
    _native_family_receipt_violations,
    effective_progress_contract,
    missing_progress_vocabulary,
)
from governor.application.trajectory_service import TrajectoryService  # noqa: E402
from governor.domain.absorption_progress import (  # noqa: E402
    NATIVE_FAMILY_VIOLATION_LABELS,
)

RULE_PATH = "scripts/governor/checks/grobid_absorption_progress.yaml"
SCHEMA_PATH = "data/parse/absorption_progress.schema.yaml"
BASELINE_PATH = "artifacts/parse_compat/absorption_progress_baseline.json"
PARITY_RECEIPT = "artifacts/parse_compat/native_family_parity.json"
EXCLUSION_RECEIPT = "artifacts/parse_compat/native_exclusion_recovery.json"
DECLARATION_MANIFEST = "data/parse/layout_bakeoff.yaml"
PRESERVED_PROVENANCE = "crates/da-parse/tests/fixtures/preserved/provenance.yaml"
HOLDOUT_MANIFEST = "data/parse/holdout_manifests.yaml"
RELEASE_PROBE = "artifacts/parse_compat/external_holdout_release_probe.json"
POSITIVE_FIXTURE = "governor/fixtures/absorption_progress/native_family.positive.json"
NEGATIVE_FIXTURE = "governor/fixtures/absorption_progress/native_family.negative.json"
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
def native_rule(contract: dict) -> dict:
    return contract["native_family_receipt"]


@pytest.fixture(scope="module")
def reader() -> FilesystemAbsorptionProgressReader:
    return FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, BASELINE_PATH)


# ── the closed vocabulary is one set, in two places ────────────────────────

def test_rule_registers_exactly_the_domain_literal_vocabulary(native_rule: dict) -> None:
    """The rule may not register a label the oracle cannot emit, or vice versa."""
    assert set(native_rule["violations"]) == set(NATIVE_FAMILY_VIOLATION_LABELS)
    assert len(NATIVE_FAMILY_VIOLATION_LABELS) == 9


def test_merged_contract_vocabulary_is_complete(contract: dict) -> None:
    assert missing_progress_vocabulary(contract) == []


def test_a_dropped_native_family_block_fails_closed(contract: dict) -> None:
    stripped = dict(contract)
    stripped.pop("native_family_receipt")
    assert "native_family_receipt" in missing_progress_vocabulary(stripped)


def test_the_native_family_half_is_not_a_competing_dimension(native_rule: dict) -> None:
    """It rides the existing `absorption_progress` check; it registers nothing new."""
    assert "dimension" not in native_rule
    assert "severity" not in native_rule
    for rel in (DIMENSIONS_PATH, SEVERITY_PATH):
        text = (_REPO_ROOT / rel).read_text(encoding="utf-8")
        assert "native_family_receipt" not in text, rel
        assert "absorption_progress" not in text, rel


# ── evidence resolution on the live tree ───────────────────────────────────

def test_live_receipts_are_resolved_by_pointer_and_validate_clean(
    contract: dict, reader: FilesystemAbsorptionProgressReader
) -> None:
    evidence = reader.read_native_family_receipt(contract)

    assert isinstance(evidence, dict)
    assert evidence["available"] is True
    assert evidence["declaration"] is not None
    assert evidence["errors"] == []
    assert {rid: entry["exists"] for rid, entry in evidence["receipts"].items()} == {
        "parity": True,
        "exclusion": True,
    }
    assert {rid: entry["document"] is not None for rid, entry in evidence["receipts"].items()} == {
        "parity": True,
        "exclusion": True,
    }
    assert _native_family_receipt_violations(contract, evidence) == set()


def test_live_receipt_pins_match_the_frozen_carrier_bytes(
    contract: dict, reader: FilesystemAbsorptionProgressReader
) -> None:
    """A stale pin must fail here, not be discovered as a stale positive control."""
    evidence = reader.read_native_family_receipt(contract)
    parity = evidence["receipts"]["parity"]["document"]
    exclusion = evidence["receipts"]["exclusion"]["document"]

    assert parity["declaration"]["manifest"] == DECLARATION_MANIFEST
    assert parity["declaration"]["revision_sha256"] == "sha256:" + _bare_sha(DECLARATION_MANIFEST)
    assert (
        parity["populations"][0]["revision_sha256"]
        == "sha256:" + _bare_sha(DECLARATION_MANIFEST)
    )
    assert parity["oracle"]["manifest"] == PRESERVED_PROVENANCE
    assert (
        parity["oracle"]["revision_sha256"] == "sha256:" + _bare_sha(PRESERVED_PROVENANCE)
    )

    provenance = _load_yaml(PRESERVED_PROVENANCE)
    pins = {entry["path"]: entry["sha256"] for entry in provenance["fixtures"]}
    for document in parity["oracle"]["documents"]:
        assert pins[document["fixture_path"]] == document["sha256"]

    assert exclusion["declaration"]["revision_sha256"] == "sha256:" + _bare_sha(HOLDOUT_MANIFEST)
    assert exclusion["probe"]["revision_sha256"] == "sha256:" + _bare_sha(RELEASE_PROBE)
    assert exclusion["baseline"]["revision_sha256"] == "sha256:" + _bare_sha(BASELINE_PATH)


def test_live_receipts_keep_the_declared_populations(
    contract: dict, reader: FilesystemAbsorptionProgressReader
) -> None:
    evidence = reader.read_native_family_receipt(contract)
    parity = evidence["receipts"]["parity"]["document"]
    exclusion = evidence["receipts"]["exclusion"]["document"]

    corpus = _load_yaml(DECLARATION_MANIFEST)["corpus"]["documents"]
    pinned = {entry["id"]: entry["sha256"] for entry in corpus}
    declared = {entry["id"]: entry["sha256"] for entry in parity["populations"][0]["documents"]}
    assert declared == pinned
    assert parity["populations"][0]["document_count"] == len(corpus) == 3
    assert parity["populations"][0]["page_denominator"] == 202

    assert exclusion["population"]["assigned"] == 610
    assert exclusion["population"]["excluded"] == 79
    assert exclusion["population"]["measurable"] == 531
    assert exclusion["population"]["targeted"] == 79
    assert exclusion["population"]["reasons"] == {
        "header_date_absent": 72,
        "header_title_absent": 5,
        "header_author_absent": 2,
    }


def test_live_exclusion_denominator_still_partitions_the_79(reader) -> None:
    exclusion = _load_json(EXCLUSION_RECEIPT)
    assert sum(row["denominator"] for row in exclusion["aggregates"]) == 79
    assert len(exclusion["recoveries"]) == exclusion["population"]["targeted"] == 79
    assert exclusion["integrity_failures"] == []


def test_live_tree_service_reports_no_native_family_finding() -> None:
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    flags = service._check_absorption_progress()
    leaked = [
        f for f in flags
        if "native_family" in (f.flag or "") or "native_family" in (f.evidence or "")
    ]
    assert leaked == [], [f.flag for f in flags]
    assert flags == [], [f.flag for f in flags]


# ── fixtures: positive clean, negative matrix covers the closed set ────────

def test_positive_fixture_is_clean(contract: dict) -> None:
    records = _load_json(POSITIVE_FIXTURE)
    assert isinstance(records, list) and records
    for record in records:
        assert _native_family_receipt_violations(contract, record) == set()


def test_negative_fixture_triggers_every_registered_label(contract: dict) -> None:
    records = _load_json(NEGATIVE_FIXTURE)
    assert isinstance(records, list) and records
    observed: set[str] = set()
    for entry in records:
        label = entry["violation"]
        violations = _native_family_receipt_violations(contract, entry["record"])
        observed |= violations
        assert label in violations, (label, sorted(violations))
        # Each record is a clean receipt with exactly one mutation, so the target
        # label is the only finding: a mutation that trips a second label is a
        # real overlap and must be made explicit rather than tolerated.
        assert violations == {label}, (label, sorted(violations))
    assert observed <= set(NATIVE_FAMILY_VIOLATION_LABELS)
    assert set(NATIVE_FAMILY_VIOLATION_LABELS) <= observed


def test_negative_fixture_registration_matches_the_literal_set(native_rule: dict) -> None:
    required = set(
        native_rule["fixtures"]["negative"]["expect"]["require_violations"]
    )
    assert required == set(NATIVE_FAMILY_VIOLATION_LABELS)


def test_absent_evidence_is_unverified_never_refuted(contract: dict) -> None:
    """Non-vacuity boundary: silence is not compliance, and it is not a finding."""
    assert _native_family_receipt_violations(contract, None) == set()
    assert _native_family_receipt_violations(contract, {}) == set()
    assert _native_family_receipt_violations(contract, {"available": False}) == set()


def test_a_resolvable_claim_without_a_receipt_is_refused(contract: dict) -> None:
    """The shape R141 forbids: a declared native family half with no evidence."""
    records = _load_json(NEGATIVE_FIXTURE)
    by_label = {entry["violation"]: entry["record"] for entry in records}
    missing = by_label["native_family_claim_without_receipt"]
    assert missing["available"] is True
    assert missing["receipts"]["parity"]["document"] is None
    assert _native_family_receipt_violations(contract, missing) == {
        "native_family_claim_without_receipt"
    }


def test_the_declaration_authority_gates_availability(contract: dict) -> None:
    """A tree without the S03 authority is silence, not a claim and not a finding."""
    reader = FilesystemAbsorptionProgressReader(
        _REPO_ROOT, SCHEMA_PATH, BASELINE_PATH
    )
    rule = contract["native_family_receipt"]
    absent = reader.read_native_family_receipt(
        {
            "native_family_receipt": {
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
    assert _native_family_receipt_violations(contract, absent) == set()


# ── mutation semantics the oracle must refuse ──────────────────────────────

def _clean_bundle() -> dict:
    return copy.deepcopy(_load_json(POSITIVE_FIXTURE)[0])


def test_a_stale_source_pin_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipts"]["parity"]["document"]["declaration"]["revision_sha256"] = "sha256:" + "9" * 64
    assert _native_family_receipt_violations(contract, record) == {"native_family_receipt_stale"}


def test_an_oracle_bound_to_the_wrong_tei_bytes_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    # A different recomputed carrier byte means the oracle no longer describes the
    # preserved bytes it names.
    record["carriers"]["preserved_tei_provenance"]["sha256"] = "sha256:" + "9" * 64
    assert _native_family_receipt_violations(contract, record) == {"native_family_receipt_stale"}


def test_a_dropped_exclusion_row_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipts"]["exclusion"]["document"]["recoveries"] = record["receipts"][
        "exclusion"
    ]["document"]["recoveries"][:-1]
    assert _native_family_receipt_violations(contract, record) == {
        "native_family_exclusion_denominator_dropped"
    }


def test_a_shrunken_exclusion_population_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    # A reason count that no longer matches the frozen 72/5/2 distribution is a
    # population mismatch and nothing else: the aggregates still partition 79, so
    # the denominator label must stay silent.
    record["receipts"]["exclusion"]["document"]["population"]["reasons"][
        "header_date_absent"
    ] = 71
    assert _native_family_receipt_violations(contract, record) == {
        "native_family_receipt_population_mismatch"
    }


def test_an_untyped_family_disposition_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipts"]["parity"]["document"]["populations"][0]["matrix"][0]["families"][0][
        "disposition"
    ] = "assumed"
    assert _native_family_receipt_violations(contract, record) == {
        "native_family_receipt_population_mismatch"
    }


def test_a_self_scoring_oracle_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipts"]["parity"]["document"]["oracle"]["same_generator_refused"] = False
    assert _native_family_receipt_violations(contract, record) == {
        "native_family_oracle_not_independent"
    }


def test_a_reopened_promotion_lock_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipts"]["parity"]["document"]["locks"]["deployed"] = "true"
    assert _native_family_receipt_violations(contract, record) == {
        "native_family_product_coverage_claimed"
    }


def test_a_leaked_redaction_flag_is_refused(contract: dict) -> None:
    record = _clean_bundle()
    record["receipts"]["exclusion"]["document"]["redaction"]["raw_pdf"] = True
    assert _native_family_receipt_violations(contract, record) == {
        "native_family_product_coverage_claimed"
    }
