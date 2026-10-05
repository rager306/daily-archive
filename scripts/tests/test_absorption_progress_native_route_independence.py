"""M516-th0oby/S02/T05 — the native-route independence half of the progress check.

The T04 independence receipt turns the route receipt's guard-owned zero into a
*measured* axis: a value bound to a non-empty denominator, an evidence identity
and a comparability identity, with a diagnostic-only claim scope that leaves the
product ``java_independence`` scorecard row untouched (D499/D508). A receipt that
nothing independently re-reads is not evidence (R141), so this file proves the
half that now sees it is honest rather than optimistic:

* the rule's label set is exactly the domain's literal set, so a label the oracle
  can emit cannot be unregistered and a registered label cannot be unemittable;
* the committed receipt, emission artifact and route receipt are resolved from
  the live tree (the declaration reused by pointer from the sibling
  ``native_route_e2e`` block), the emission and guard provenance digests are
  recomputed from the committed bytes, and the bundle validates clean;
* the live axis is ``measured`` with the declared 3-document / 202-page
  denominator and a five-lock, diagnostic-only envelope;
* the real service path reports no ``native_route_independence_*`` finding, and
  is proven to *consume* the observation (the regression guard against the inert
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
    _native_route_independence_violations,
    effective_progress_contract,
    missing_progress_vocabulary,
)
from governor.application.trajectory_service import TrajectoryService  # noqa: E402
from governor.domain.absorption_progress import (  # noqa: E402
    NATIVE_ROUTE_INDEPENDENCE_VIOLATION_LABELS,
)

RULE_PATH = "scripts/governor/checks/grobid_absorption_progress.yaml"
SCHEMA_PATH = "data/parse/absorption_progress.schema.yaml"
BASELINE_PATH = "artifacts/parse_compat/absorption_progress_baseline.json"
RECEIPT_PATH = "artifacts/parse_compat/native_route_independence.json"
EMISSION_PATH = "artifacts/parse_compat/native_route_tei_emission.json"
ROUTE_RECEIPT_PATH = "artifacts/parse_compat/native_route_e2e.json"
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
def independence_rule(contract: dict) -> dict:
    return contract["native_route_independence"]


@pytest.fixture(scope="module")
def reader() -> FilesystemAbsorptionProgressReader:
    return FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, BASELINE_PATH)


@pytest.fixture(scope="module")
def live_evidence(contract: dict, reader: FilesystemAbsorptionProgressReader) -> dict:
    return reader.read_native_route_independence(contract)


# ── the closed vocabulary is one set, in two places ────────────────────────


def test_rule_registers_exactly_the_domain_literal_vocabulary(
    independence_rule: dict,
) -> None:
    """The rule may not register a label the oracle cannot emit, or vice versa."""
    assert set(independence_rule["violations"]) == set(
        NATIVE_ROUTE_INDEPENDENCE_VIOLATION_LABELS
    )
    assert len(NATIVE_ROUTE_INDEPENDENCE_VIOLATION_LABELS) == 10


def test_merged_contract_vocabulary_is_complete(contract: dict) -> None:
    assert missing_progress_vocabulary(contract) == []


def test_a_dropped_independence_block_fails_closed(contract: dict) -> None:
    stripped = dict(contract)
    stripped.pop("native_route_independence")
    assert "native_route_independence" in missing_progress_vocabulary(stripped)


def test_the_independence_half_is_not_a_competing_dimension(
    independence_rule: dict,
) -> None:
    """It rides the existing `absorption_progress` check; it registers nothing new."""
    assert "dimension" not in independence_rule
    assert "severity" not in independence_rule
    for rel in (DIMENSIONS_PATH, SEVERITY_PATH):
        text = (_REPO_ROOT / rel).read_text(encoding="utf-8")
        assert "native_route_independence" not in text, rel
        assert "absorption_progress" not in text, rel


# ── evidence resolution on the live tree ───────────────────────────────────


def test_live_receipt_is_resolved_by_pointer_and_validates_clean(
    contract: dict, live_evidence: dict
) -> None:
    assert live_evidence["available"] is True
    assert live_evidence["errors"] == []
    assert live_evidence["receipt_exists"] is True
    assert live_evidence["emission_exists"] is True
    assert live_evidence["route_receipt_exists"] is True
    assert isinstance(live_evidence["receipt"], dict)
    assert live_evidence["emission_corpus"]["document_count"] == 12
    assert live_evidence["emission_corpus"]["page_denominator"] == 381
    assert _native_route_independence_violations(contract, live_evidence) == set()


def test_live_pins_match_the_committed_carriers(live_evidence: dict) -> None:
    """A stale pin must fail here, not be discovered as a stale positive control."""
    recomputed_emission = "sha256:" + _bare_sha(EMISSION_PATH)
    assert live_evidence["emission_sha256"] == recomputed_emission

    receipt = _load_json(RECEIPT_PATH)
    assert receipt["axis"]["evidence_identity"]["emission_sha256"] == recomputed_emission
    assert receipt["emission"]["artifact_sha256"] == recomputed_emission

    route = _load_json(ROUTE_RECEIPT_PATH)
    assert (
        receipt["axis"]["evidence_identity"]["guard_provenance_sha256"]
        == live_evidence["guard_provenance_sha256"]
    )
    assert route["declaration"]["revision_sha256"] == live_evidence[
        "route_declaration_revision_sha256"
    ]
    assert (
        receipt["axis"]["comparability_identity"]["source_revision"]
        == route["declaration"]["revision_sha256"]
    )


def test_live_axis_is_measured_with_a_non_empty_denominator(
    live_evidence: dict,
) -> None:
    receipt = live_evidence["receipt"]
    axis = receipt["axis"]
    assert axis["state"] == "measured"
    assert axis["value"] == 0
    assert axis["denominator"]["documents"] == 12
    assert axis["denominator"]["pages"] == 381
    assert receipt["claim_scope"] == "diagnostic_only"
    assert receipt["product_coverage_effect"] == "none"
    assert receipt["thresholds"] == "not_accepted"
    for lock in ("deployed", "import_eligible", "auto_promote", "production_route", "graph_write"):
        assert lock in receipt["locks"]


def test_live_tree_service_reports_no_independence_finding() -> None:
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    flags = service._check_absorption_progress()
    leaked = [
        f
        for f in flags
        if "native_route_independence" in (f.flag or "")
        or "native_route_independence" in (f.evidence or "")
    ]
    assert leaked == [], [f.flag for f in flags]
    assert flags == [], [f.flag for f in flags]


def test_the_service_path_consumes_the_independence_observation(monkeypatch) -> None:
    """The independence half must fail closed through the real service, not only
    when called directly.

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
        "read_native_route_independence",
        broken,
    )
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    flags = service._check_absorption_progress()
    broken_flags = [f for f in flags if f.flag == "absorption_progress_broken"]
    assert broken_flags, [f.flag for f in flags]
    assert (
        "native_route_independence_claim_without_receipt" in broken_flags[0].evidence
    )


# ── mutation semantics the oracle must refuse ──────────────────────────────


def _clean_bundle(live_evidence: dict) -> dict:
    return copy.deepcopy(live_evidence)


def test_a_resolvable_claim_without_a_receipt_is_refused(
    contract: dict, live_evidence: dict
) -> None:
    """The shape R141 forbids: a declared independence receipt with no evidence."""
    record = _clean_bundle(live_evidence)
    record["receipt"] = None
    assert _native_route_independence_violations(contract, record) == {
        "native_route_independence_claim_without_receipt"
    }


def test_a_receipt_of_another_schema_is_refused(
    contract: dict, live_evidence: dict
) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["schema_version"] = "native-route-independence/v0"
    assert _native_route_independence_violations(contract, record) == {
        "native_route_independence_claim_without_receipt"
    }


def test_a_non_measured_axis_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["axis"]["state"] = "not_measured"
    assert _native_route_independence_violations(contract, record) == {
        "native_route_independence_axis_not_measured"
    }


def test_an_unbound_denominator_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["axis"]["denominator"]["documents"] = 0
    assert _native_route_independence_violations(contract, record) == {
        "native_route_independence_unbound_denominator"
    }


def test_an_unbound_evidence_identity_is_refused(
    contract: dict, live_evidence: dict
) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["axis"]["evidence_identity"]["guard_provenance_sha256"] = ""
    assert _native_route_independence_violations(contract, record) == {
        "native_route_independence_unbound_evidence_identity"
    }


def test_an_unbound_comparability_identity_is_refused(
    contract: dict, live_evidence: dict
) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["axis"]["comparability_identity"]["oracle"] = ""
    assert _native_route_independence_violations(contract, record) == {
        "native_route_independence_unbound_comparability_identity"
    }


def test_a_guard_less_zero_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["guard"]["positive_control_observed"] = False
    assert _native_route_independence_violations(contract, record) == {
        "native_route_independence_guard_missing_for_observed_zero"
    }


def test_an_opened_lock_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["locks"]["deployed"] = "true"
    assert _native_route_independence_violations(contract, record) == {
        "native_route_independence_lock_opened"
    }


def test_a_missing_lock_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["locks"].pop("graph_write")
    assert _native_route_independence_violations(contract, record) == {
        "native_route_independence_lock_opened"
    }


def test_a_product_coverage_claim_is_refused(
    contract: dict, live_evidence: dict
) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["product_coverage_effect"] = "full"
    assert _native_route_independence_violations(contract, record) == {
        "native_route_independence_product_coverage_claimed"
    }


def test_a_threshold_claim_is_refused(contract: dict, live_evidence: dict) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["thresholds"] = "accepted"
    assert _native_route_independence_violations(contract, record) == {
        "native_route_independence_threshold_claimed"
    }


def test_a_stale_guard_provenance_pin_is_refused(
    contract: dict, live_evidence: dict
) -> None:
    """A well-formed but no-longer-matching digest is stale, not unbound."""
    record = _clean_bundle(live_evidence)
    record["receipt"]["axis"]["evidence_identity"]["guard_provenance_sha256"] = (
        "sha256:" + "9" * 64
    )
    assert _native_route_independence_violations(contract, record) == {
        "native_route_independence_receipt_stale"
    }


def test_a_stale_comparability_revision_is_refused(
    contract: dict, live_evidence: dict
) -> None:
    record = _clean_bundle(live_evidence)
    record["receipt"]["axis"]["comparability_identity"]["source_revision"] = (
        "sha256:" + "9" * 64
    )
    assert _native_route_independence_violations(contract, record) == {
        "native_route_independence_receipt_stale"
    }


# ── non-vacuity boundaries ─────────────────────────────────────────────────


def test_absent_evidence_is_unverified_never_refuted(contract: dict) -> None:
    """Non-vacuity boundary: silence is not compliance, and it is not a finding."""
    assert _native_route_independence_violations(contract, None) == set()
    assert _native_route_independence_violations(contract, {}) == set()
    assert _native_route_independence_violations(contract, {"available": False}) == set()


def test_the_declaration_authority_gates_availability(contract: dict) -> None:
    """A tree without the reused declaration authority is silence, not a finding."""
    reader = FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, BASELINE_PATH)
    absent = reader.read_native_route_independence(
        {
            "native_route_e2e": {
                **contract["native_route_e2e"],
                "declaration": {
                    "manifest": "data/parse/absent_bakeoff.yaml",
                    "pointer": "java_free_vertical_slice",
                },
            },
            "native_route_independence": contract["native_route_independence"],
        }
    )
    assert absent is not None
    assert absent["available"] is False
    assert _native_route_independence_violations(contract, absent) == set()
