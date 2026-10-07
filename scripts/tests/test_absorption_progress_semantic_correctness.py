"""M516-th0oby/S04/T03 — the semantic-correctness half of the progress check.

The T01 receipt runs H14 (composer XML well-formedness) and H15 (bibliography
id/target integrity) over the expanded 12-document native route corpus and
publishes a *per-document* reason pair instead of one aggregate verdict. A
receipt that nothing independently re-reads is not evidence (R141), so this file
proves the half that now sees it is honest rather than optimistic:

* the rule's label set is exactly the domain's literal set, so a label the oracle
  can emit cannot be unregistered and a registered label cannot be unemittable;
* the committed receipt, emission envelope and reference corpus declaration are
  resolved from the live tree, the receipt's corpus digest, the emission
  ``sha256`` and the independent row-level reference identity are recomputed from
  the committed bytes, and the bundle validates clean;
* the real service path reports no ``semantic_correctness_*`` finding, and is
  proven to *consume* the observation (the regression guard against the inert
  half of MEM1948);
* absence of the declaration authority is silence, never compliance, and every
  mutation the oracle must refuse is exercised one at a time.

The half adds no dimension and no severity of its own (D492). Nothing here
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
    _semantic_correctness_violations,
    effective_progress_contract,
    missing_progress_vocabulary,
)
from governor.application.trajectory_service import TrajectoryService  # noqa: E402
from governor.domain.absorption_progress import (  # noqa: E402
    SEMANTIC_CORRECTNESS_VIOLATION_LABELS,
)

RULE_PATH = "scripts/governor/checks/grobid_absorption_progress.yaml"
SCHEMA_PATH = "data/parse/absorption_progress.schema.yaml"
BASELINE_PATH = "artifacts/parse_compat/absorption_progress_baseline.json"
RECEIPT_PATH = "artifacts/parse_compat/native_route_semantic_receipt.json"
EMISSION_PATH = "artifacts/parse_compat/native_route_tei_emission.json"
LAYOUT_PATH = "data/parse/layout_bakeoff.yaml"
DIMENSIONS_PATH = "scripts/governor/checks/dimensions.yaml"
SEVERITY_PATH = "scripts/governor/checks/assurance_severity.yaml"


def _load_yaml(rel: str) -> dict:
    return yaml.safe_load((_REPO_ROOT / rel).read_text(encoding="utf-8"))


def _load_json(rel: str) -> dict:
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
def semantic_rule(contract: dict) -> dict:
    return contract["semantic_correctness"]


@pytest.fixture(scope="module")
def reader() -> FilesystemAbsorptionProgressReader:
    return FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, BASELINE_PATH)


@pytest.fixture(scope="module")
def live_evidence(contract: dict, reader: FilesystemAbsorptionProgressReader) -> dict:
    return reader.read_semantic_correctness(contract)


# ── the closed vocabulary is one set, in two places ────────────────────────


def test_rule_registers_exactly_the_domain_literal_vocabulary(semantic_rule: dict) -> None:
    """The rule may not register a label the oracle cannot emit, or vice versa."""
    assert set(semantic_rule["violations"]) == set(SEMANTIC_CORRECTNESS_VIOLATION_LABELS)
    assert len(SEMANTIC_CORRECTNESS_VIOLATION_LABELS) == 10


def test_merged_contract_vocabulary_is_complete(contract: dict) -> None:
    assert missing_progress_vocabulary(contract) == []


def test_a_dropped_semantic_block_fails_closed(contract: dict) -> None:
    stripped = dict(contract)
    stripped.pop("semantic_correctness")
    assert "semantic_correctness" in missing_progress_vocabulary(stripped)


def test_the_semantic_half_is_not_a_competing_dimension(semantic_rule: dict) -> None:
    """It rides the existing `absorption_progress` check; it registers nothing new."""
    assert "dimension" not in semantic_rule
    assert "severity" not in semantic_rule
    for rel in (DIMENSIONS_PATH, SEVERITY_PATH):
        text = (_REPO_ROOT / rel).read_text(encoding="utf-8")
        assert "semantic_correctness" not in text, rel
        assert "absorption_progress" not in text, rel


# ── evidence resolution on the live tree ───────────────────────────────────


def test_live_receipt_is_resolved_by_pointer_and_validates_clean(
    contract: dict, live_evidence: dict
) -> None:
    assert live_evidence["available"] is True
    assert live_evidence["errors"] == []
    assert live_evidence["receipt_exists"] is True
    assert live_evidence["emission_exists"] is True
    assert isinstance(live_evidence["receipt"], dict)
    assert live_evidence["reference_document_count"] == 12
    assert live_evidence["corpus_document_count"] == 12
    assert live_evidence["emission_document_count"] == 12
    assert _semantic_correctness_violations(contract, live_evidence) == set()


def test_live_pins_match_the_committed_carriers(live_evidence: dict) -> None:
    """A stale pin must fail here, not be discovered as a stale positive control."""
    receipt = _load_json(RECEIPT_PATH)
    assert live_evidence["emission_sha256"] == "sha256:" + _bare_sha(EMISSION_PATH)
    assert receipt["native_route_emission_scope"]["sha256"] == live_evidence["emission_sha256"]
    assert receipt["corpus"]["documents_digest"] == live_evidence["reference_documents_digest"]


def test_live_receipt_carries_a_per_document_reason_table(live_evidence: dict) -> None:
    """The whole point of S04: per-document reasons, never one aggregate verdict."""
    receipt = live_evidence["receipt"]
    assert receipt["population"]["aggregate_verdict"] == "no_aggregate_verdict"
    documents = receipt["documents"]
    assert len(documents) == 12
    for row in documents:
        assert row["composer"]["well_formedness_reason"]
        assert row["composer"]["bibliography_target_reason"]


def test_live_tree_service_reports_no_semantic_finding() -> None:
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    flags = service._check_absorption_progress()
    leaked = [
        f
        for f in flags
        if "semantic_correctness" in (f.flag or "")
        or "semantic_correctness" in (f.evidence or "")
    ]
    assert leaked == [], [f.flag for f in flags]
    assert flags == [], [f.flag for f in flags]


def test_the_service_path_consumes_the_semantic_observation(monkeypatch) -> None:
    """The semantic half must fail closed through the real service, not only when
    called directly.

    This is the regression guard against the *inert half* failure mode the S04
    threat review found (MEM1948): a sibling block whose oracle is never fed by
    the service reads as clean on every tree, so the half could be deleted or
    silently regress without any check noticing. Patching the reader to return a
    claim-without-evidence bundle must raise the composed finding through
    `_check_absorption_progress`.
    """

    def broken(self, contract):  # noqa: ANN001, ARG001
        return {"available": True, "receipt": None}

    monkeypatch.setattr(
        FilesystemAbsorptionProgressReader,
        "read_semantic_correctness",
        broken,
    )
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    flags = service._check_absorption_progress()
    broken_flags = [f for f in flags if f.flag == "absorption_progress_broken"]
    assert broken_flags, [f.flag for f in flags]
    assert "receipt_absent" in broken_flags[0].evidence


# ── mutation semantics the oracle must refuse ──────────────────────────────


def _clean_bundle(live_evidence: dict) -> dict:
    return copy.deepcopy(live_evidence)


def test_a_required_receipt_absent_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"] = None
    assert _semantic_correctness_violations(contract, record) == {"receipt_absent"}


def test_a_receipt_of_another_schema_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["schema_version"] = "native-route-semantic-receipt/v0"
    assert _semantic_correctness_violations(contract, record) == {"receipt_absent"}


def test_a_widened_claim_scope_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["claim_scope"] = "product"
    assert "threshold_accepted" in _semantic_correctness_violations(contract, record)


def test_an_accepted_threshold_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["thresholds"] = "accepted"
    assert "threshold_accepted" in _semantic_correctness_violations(contract, record)


def test_an_opened_lock_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["locks"]["deployed"] = "true"
    assert "lock_opened" in _semantic_correctness_violations(contract, record)


def test_a_missing_lock_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["locks"].pop("graph_write")
    assert "lock_opened" in _semantic_correctness_violations(contract, record)


def test_a_rewritten_corpus_pointer_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["corpus"]["corpus_pointer"] = "data/parse/other.yaml#corpus"
    assert "corpus_pointer_mismatch" in _semantic_correctness_violations(contract, record)


def test_a_rewritten_reference_pointer_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["corpus"]["reference_pointer"] = "data/parse/other.yaml#corpus"
    assert "corpus_pointer_mismatch" in _semantic_correctness_violations(contract, record)


def test_a_relabelled_corpus_id_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["corpus"]["id"] = "some-other-corpus"
    assert "corpus_pointer_mismatch" in _semantic_correctness_violations(contract, record)


def test_a_dropped_document_row_is_refused(contract: dict, live_evidence: dict) -> None:
    """A dropped row is a count mismatch and a stale row-level identity."""
    record = _clean_bundle(live_evidence)
    record["receipt"]["documents"].pop()
    violations = _semantic_correctness_violations(contract, record)
    assert "document_count_mismatch" in violations
    assert "receipt_stale" in violations


def test_a_wrong_document_count_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["corpus"]["document_count"] = 11
    assert "document_count_mismatch" in _semantic_correctness_violations(contract, record)


def test_a_missing_well_formedness_reason_is_refused(
    contract: dict, live_evidence: dict
) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["documents"][0]["composer"]["well_formedness_reason"] = ""
    assert "per_document_reason_missing" in _semantic_correctness_violations(contract, record)


def test_an_out_of_vocabulary_bibliography_reason_is_refused(
    contract: dict, live_evidence: dict
) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["documents"][0]["composer"]["bibliography_target_reason"] = "looks_fine"
    assert "per_document_reason_missing" in _semantic_correctness_violations(contract, record)


def test_empty_composer_targets_presented_as_resolved_is_refused(
    contract: dict, live_evidence: dict
) -> None:
    record = _clean_bundle(live_evidence)
    composer = record["receipt"]["documents"][0]["composer"]
    composer["ledger"]["required_targets"] = 0
    composer["ledger"]["resolved_targets"] = 0
    composer["bibliography_target_reason"] = "all_required_targets_resolved"
    assert "empty_targets_presented_as_pass" in _semantic_correctness_violations(
        contract, record
    )


def test_placeholder_header_read_as_populated_is_refused(
    contract: dict, live_evidence: dict
) -> None:
    record = _clean_bundle(live_evidence)
    header = record["receipt"]["native_route_emission_scope"]["documents"][0]["header"]
    header["disposition"] = "typed_absence"
    header["scope_reason"] = "header_native_emitted"
    assert "placeholder_header_read_as_populated" in _semantic_correctness_violations(
        contract, record
    )


def test_empty_emitted_targets_presented_as_declared_is_refused(
    contract: dict, live_evidence: dict
) -> None:
    record = _clean_bundle(live_evidence)
    links = record["receipt"]["native_route_emission_scope"]["documents"][0][
        "reference_links"
    ]
    links["required_targets"] = 0
    links["target_occurrences"] = 0
    links["scope_reason"] = "reference_links_declared_targets"
    assert "empty_targets_presented_as_pass" in _semantic_correctness_violations(
        contract, record
    )


def test_a_stale_corpus_digest_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["corpus"]["documents_digest"] = "sha256:" + "9" * 64
    assert "receipt_stale" in _semantic_correctness_violations(contract, record)


def test_a_stale_emission_digest_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["native_route_emission_scope"]["sha256"] = "sha256:" + "9" * 64
    assert "receipt_stale" in _semantic_correctness_violations(contract, record)


def test_a_rebound_row_identity_is_refused(contract: dict, live_evidence: dict) -> None:
    """A re-bound reference sha is stale even when the stored corpus digest is
    made self-consistent: the independent row-level identity catches it."""
    record = _clean_bundle(live_evidence)
    row = record["receipt"]["documents"][0]
    row["reference_sha256"] = "9" * 64
    record["receipt"]["corpus"]["documents_digest"] = record[
        "reference_documents_digest"
    ]
    assert "receipt_stale" in _semantic_correctness_violations(contract, record)


def test_an_aggregate_verdict_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["population"]["aggregate_verdict"] = "all_documents_pass"
    assert "pinned_aggregate_rewritten" in _semantic_correctness_violations(contract, record)


# ── non-vacuity boundaries ─────────────────────────────────────────────────


def test_absent_evidence_is_unverified_never_refuted(contract: dict) -> None:
    """Non-vacuity boundary: silence is not compliance, and it is not a finding."""
    assert _semantic_correctness_violations(contract, None) == set()
    assert _semantic_correctness_violations(contract, {}) == set()
    assert _semantic_correctness_violations(contract, {"available": False}) == set()


def test_the_declaration_authority_gates_availability(contract: dict) -> None:
    """A tree without the declaration authority is silence, not a finding."""
    reader = FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, BASELINE_PATH)
    absent = reader.read_semantic_correctness(
        {
            "semantic_correctness": {
                **contract["semantic_correctness"],
                "declaration": {
                    "manifest": "data/parse/absent_bakeoff.yaml",
                    "pointer": "native_route_threshold_reference.documents",
                },
            }
        }
    )
    assert absent is not None
    assert absent["available"] is False
    assert _semantic_correctness_violations(contract, absent) == set()
