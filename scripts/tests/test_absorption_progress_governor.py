"""M514-1lss2s/S01/T04 tests for the absorption progress Governor check.

The closed vocabulary is DATA (``data/parse/absorption_progress.schema.yaml``),
the rule configuration is DATA
(``scripts/governor/checks/grobid_absorption_progress.yaml``), the pure oracle is
``governor.adapters.absorption_progress_reader.absorption_progress_violations``,
and the filesystem half is ``FilesystemAbsorptionProgressReader``.

The matrix proves the measurement system is honest rather than optimistic:

* identity — a substituted evidence hash, an evidence path that is not
  repository-relative, and a declared artifact that no longer resolves;
* series — a changed oracle and a scope outside the accepted series are
  ``not_comparable``, and two series inside one scorecard are refused;
* direction — a denominator smaller than the pinned census and an axis the
  census measured going silent are regressions;
* thresholds — S02 froze the thresholds and bound them to the schema revision, so
  a declared freeze revision that no longer matches the carrier bytes, and a
  threshold whose reference does not resolve, are both refused;
* resources — a savings claim without its paired CPU measurement is refused;
* the T01 domain labels (denominator sum, dropped exclusion, revision mismatch,
  skipped-as-delivered) survive contact with the composed oracle.

Tests use tracked DATA, the committed fixtures and inline value fixtures. The
end-to-end tests build a scratch tree so the real service path is exercised
without touching the repository.
"""

from __future__ import annotations

import atexit
import copy
import hashlib
import json
import shutil
import sys
import tempfile
from pathlib import Path

import pytest
import yaml

_SCRIPTS_DIR = Path(__file__).resolve().parent.parent
_REPO_ROOT = _SCRIPTS_DIR.parent
if str(_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS_DIR))

from governor.adapters.absorption_progress_reader import (  # noqa: E402
    FilesystemAbsorptionProgressReader,
    _UNRESOLVED,
    absorption_progress_violations,
    effective_progress_contract,
    missing_progress_vocabulary,
    projection_digest,
    resolve_pointer,
)
from governor.application.trajectory_service import TrajectoryService  # noqa: E402

RULE_PATH = "scripts/governor/checks/grobid_absorption_progress.yaml"
SCHEMA_PATH = "data/parse/absorption_progress.schema.yaml"
POSITIVE_FIXTURE = "governor/fixtures/absorption_progress/scorecard_clean.positive.json"
NEGATIVE_FIXTURE = "governor/fixtures/absorption_progress/scorecard_broken.negative.json"
SCORECARD_PATH = "artifacts/parse_compat/absorption_progress_baseline.json"
REFERENCE_PINS_PATH = "artifacts/parse_compat/absorption_progress_reference_pins.json"
REFERENCE_PINS_PATH = "artifacts/parse_compat/absorption_progress_reference_pins.json"
DOMAIN_MODULE = "scripts/governor/domain/absorption_progress.py"
ADAPTER_MODULE = "scripts/governor/adapters/absorption_progress_reader.py"

REV = "git:" + "52bb08a4a016e69e3224ae79f02c2f3e53897e72"
REV2 = "git:" + "11" * 20
SHA = "sha256:" + "a" * 64
EVIDENCE = "artifacts/parse_compat/external_holdout_release_probe.json"

#: The real schema digest, recomputed rather than pinned: the frozen thresholds are
#: bound to the carrier revision, so a scratch tree that copies the schema must be
#: checked against the bytes it actually read.
SCHEMA_REV = "sha256:" + hashlib.sha256((_REPO_ROOT / SCHEMA_PATH).read_bytes()).hexdigest()

#: The adapter-level labels the oracle may emit. Pinned here so a new label that
#: is not registered in the DATA vocabulary is a test failure, not a silent
#: finding.
ADAPTER_LABELS = {
    "evidence_path_not_relative",
    "evidence_artifact_missing",
    "evidence_hash_substituted",
    "oracle_changed",
    "scope_not_accepted",
    "scope_mixed_in_scorecard",
    "savings_without_paired_cpu",
    "savings_paired_cpu_not_measured",
    "coverage_regression",
    "denominator_shrink",
    "threshold_freeze_revision_mismatch",
    "threshold_reference_unbound",
    "census_projection_mismatch",
    "census_document_dropped",
    "census_disposition_untyped",
    "refused_set_unbound",
}


# ── scratch tree ───────────────────────────────────────────────────────────

_SCRATCH_ROOTS: list[Path] = []


def _make_root() -> Path:
    root = Path(tempfile.mkdtemp(prefix="governor-test-"))
    _SCRATCH_ROOTS.append(root)
    return root


def _cleanup_scratch_roots() -> None:
    for root in _SCRATCH_ROOTS:
        shutil.rmtree(root, ignore_errors=True)
    _SCRATCH_ROOTS.clear()


atexit.register(_cleanup_scratch_roots)


def _write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


# ── contract + fixtures ────────────────────────────────────────────────────


@pytest.fixture(scope="module")
def rule_contract() -> dict:
    return yaml.safe_load((_REPO_ROOT / RULE_PATH).read_text(encoding="utf-8"))


@pytest.fixture(scope="module")
def schema_contract() -> dict:
    return yaml.safe_load((_REPO_ROOT / SCHEMA_PATH).read_text(encoding="utf-8"))


@pytest.fixture(scope="module")
def contract(schema_contract: dict, rule_contract: dict) -> dict:
    return effective_progress_contract(schema_contract, rule_contract)


def _comparability(**overrides) -> dict:
    block = {
        "corpus_id": "m513-holdout-610",
        "profile": "structural",
        "oracle": "external_holdout_probe_semantics",
        "source_revision": REV,
    }
    block.update(overrides)
    return block


def _evidence(**overrides) -> dict:
    block = {
        "path": EVIDENCE,
        "sha256": SHA,
        "source_revision": REV,
        "tested_revision": REV,
        "kind": "executed_run",
    }
    block.update(overrides)
    return block


def _denominator(**overrides) -> dict:
    block = {
        "assigned": 610,
        "executed": 531,
        "refused": 0,
        "excluded": 79,
        "exclusion_reasons": {
            "header_date_absent": 72,
            "header_title_absent": 5,
            "header_author_absent": 2,
        },
    }
    block.update(overrides)
    return block


def _baseline_scorecard() -> dict:
    return {
        "schema_version": "absorption-progress.v1",
        "scorecard_id": "m514-absorption-progress-baseline",
        "census_id": "holdout-partition-610-531-79",
        "denominator": _denominator(),
        "measurements": [
            {
                "metric_id": "preservation.occurrence_conservation",
                "axis": "preservation",
                "state": "measured",
                "denominator_basis": "assigned_full",
                "score": {"numerator": 74910, "denominator": 79621},
                "strata": {
                    "micro_occurrence": {"numerator": 74910, "denominator": 79621},
                    "per_document_minimum": 0.492683,
                    "weakest_stratum": 0.492683,
                },
                "weakest_stratum_id": "per_document_minimum",
                "comparability": _comparability(),
                "evidence": _evidence(),
            },
            {
                "metric_id": "extraction.native_capability_recall",
                "axis": "extraction",
                "state": "measured",
                "denominator_basis": "assigned_full",
                "score": 0.945771,
                "strata": {
                    "micro_occurrence": 0.945771,
                    "affiliation": 0.4317,
                    "weakest_stratum": 0.4317,
                },
                "weakest_stratum_id": "affiliation",
                "comparability": _comparability(),
                "evidence": _evidence(),
            },
            {
                "metric_id": "java_independence.java_free_parse_route",
                "axis": "java_independence",
                "state": "not_measured",
                "reason": "java_production_route_active",
            },
            {
                "metric_id": "resources.native_savings",
                "axis": "resources",
                "state": "not_measured",
                "reason": "native_savings_not_measured",
            },
            {
                "metric_id": "reliability.rollback_rehearsal",
                "axis": "reliability",
                "state": "not_measured",
                "reason": "rehearsal_only_not_a_product_measurement",
            },
        ],
        "authorization": {
            "state": "owner_directive_research",
            "decision": "D491",
            "product_coverage_effect": "none",
        },
        "lineage": {},
        "threshold_freeze": {
            "state": "frozen",
            "revision": SCHEMA_REV,
            "frozen_at": "2026-09-29",
            "source": SCHEMA_PATH,
        },
        "observations": {"evidence_sha256": {EVIDENCE: SHA}},
    }


def _observations(scorecard: dict) -> dict:
    block = scorecard.get("observations")
    return block if isinstance(block, dict) else {}


def _mutated(mutate) -> dict:
    scorecard = copy.deepcopy(_baseline_scorecard())
    mutate(scorecard)
    return scorecard


# ── the contract itself ────────────────────────────────────────────────────


def test_progress_rule_loads_and_vocabulary_is_complete(contract: dict) -> None:
    assert missing_progress_vocabulary(contract) == []
    assert set(contract["violations"]) == ADAPTER_LABELS
    # The accepted scope, the expected axis states and the savings pairing are the
    # T04 additions; without them the oracle would fail open.
    assert contract["comparability_rules"]["oracle_pin"] == "external_holdout_probe_semantics"
    assert contract["comparability_rules"]["accepted"] == {
        "corpus_id": "m513-holdout-610",
        "profile": "structural",
    }
    assert contract["regression"]["expected_axis_states"]["extraction"] == "measured"
    assert contract["resources_claim"]["cpu_kind"] == "cpu"
    # S02 fixed the thresholds as a prospective regression floor and bound them
    # to the carrier revision; the floor is not the candidate scorecard.
    policy = contract["threshold_policy"]
    assert policy["state"] == "frozen"
    assert policy["tuned_after_scoring"] == "forbidden"
    assert policy["pass_requires_frozen_threshold"] is True
    assert policy["source"] == REFERENCE_PINS_PATH
    assert policy["reference_pins"]["path"] == REFERENCE_PINS_PATH
    assert {entry["threshold_id"] for entry in policy["thresholds"]} == {
        "preservation_micro_occurrence",
        "extraction_micro_occurrence",
        "extraction_weakest_family",
    }


def test_frozen_thresholds_resolve_from_the_committed_baseline(contract: dict) -> None:
    reader = FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, SCORECARD_PATH)
    resolved = reader.resolve_threshold_references(contract)
    policy = contract["threshold_policy"]
    assert set(resolved) == {entry["threshold_id"] for entry in policy["thresholds"]}
    assert all(isinstance(value, float) for value in resolved.values()), resolved
    assert resolved["extraction_micro_occurrence"] == pytest.approx(0.945771)
    assert resolved["preservation_micro_occurrence"] == pytest.approx(74910 / 79621)
    assert resolved["extraction_weakest_family"] == pytest.approx(3055 / 7077)


def test_pointer_resolution_handles_selectors_and_misses() -> None:
    document = {
        "measurements": [
            {"metric_id": "a", "score": {"numerator": 1, "denominator": 2}},
            {"metric_id": "b", "score": 0.5},
        ]
    }
    assert resolve_pointer(document, "measurements[metric_id=b]/score") == 0.5
    assert resolve_pointer(document, "measurements[metric_id=a]/score") == {
        "numerator": 1,
        "denominator": 2,
    }
    assert resolve_pointer(document, "measurements[metric_id=missing]/score") is _UNRESOLVED
    assert resolve_pointer(document, "measurements[metric_id=b]/nope") is _UNRESOLVED
    assert resolve_pointer(document, "nope/x") is _UNRESOLVED
    assert resolve_pointer(document, "") is _UNRESOLVED


def test_reader_annotates_missing_thresholds_but_keeps_declared_ones(contract: dict) -> None:
    reader = FilesystemAbsorptionProgressReader(_REPO_ROOT, SCHEMA_PATH, SCORECARD_PATH)
    resolved = reader.resolve_threshold_references(contract)
    annotated = reader.annotate_thresholds(_baseline_scorecard(), contract, resolved)
    for row in annotated["measurements"]:
        if row.get("state") == "measured":
            assert row["threshold"]["state"] == "frozen"
            assert row["threshold"]["threshold_id"] in resolved
            assert isinstance(row["threshold"]["value"], float)

    declared = _baseline_scorecard()
    declared["measurements"][0]["threshold"] = {
        "threshold_id": "preservation_micro_occurrence",
        "direction": "at_least",
        "margin": 0,
        "value": 0.0,
        "state": "frozen",
    }
    kept = reader.annotate_thresholds(declared, contract, resolved)
    assert kept["measurements"][0]["threshold"]["value"] == 0.0


def test_threshold_freeze_revision_mismatch_is_flagged(contract: dict) -> None:
    card = _mutated(
        lambda c: c["threshold_freeze"].__setitem__("revision", "sha256:" + "e" * 64)
    )
    labels = absorption_progress_violations(
        card, contract, {"evidence_sha256": {EVIDENCE: SHA}, "schema_sha256": SCHEMA_REV}
    )
    assert "threshold_freeze_revision_mismatch" in labels
    # Without a recomputed schema digest the freeze is unverified, not refuted.
    assert "threshold_freeze_revision_mismatch" not in absorption_progress_violations(
        card, contract, _observations(card)
    )
    honest = _baseline_scorecard()
    assert "threshold_freeze_revision_mismatch" not in absorption_progress_violations(
        honest, contract, {"evidence_sha256": {EVIDENCE: SHA}, "schema_sha256": SCHEMA_REV}
    )


def test_unresolved_threshold_reference_is_flagged(contract: dict) -> None:
    card = _baseline_scorecard()
    all_none = {entry["threshold_id"]: None for entry in contract["threshold_policy"]["thresholds"]}
    assert "threshold_reference_unbound" in absorption_progress_violations(
        card, contract, {"evidence_sha256": {EVIDENCE: SHA}, "threshold_values": all_none}
    )
    assert "threshold_reference_unbound" not in absorption_progress_violations(
        card, contract, _observations(card)
    )


def test_source_markers_exist_in_the_real_tree(rule_contract: dict) -> None:
    for entry in (rule_contract.get("source_markers") or {}).values():
        target = str(entry.get("path") or "")
        assert target and not target.startswith("/")
        text = (_REPO_ROOT / target).read_text(encoding="utf-8")
        for pattern in entry.get("required_patterns", []):
            assert pattern in text, (target, pattern)


def test_vocabulary_fails_closed_when_the_accepted_scope_is_dropped(contract: dict) -> None:
    stripped = dict(contract)
    stripped["comparability_rules"] = {"oracle_pin": contract["comparability_rules"]["oracle_pin"]}
    assert "comparability_rules.accepted" in missing_progress_vocabulary(stripped)

    no_pairing = dict(contract)
    no_pairing["resources_claim"] = {k: v for k, v in contract["resources_claim"].items() if k != "cpu_kind"}
    assert "resources_claim.cpu_kind" in missing_progress_vocabulary(no_pairing)


def test_positive_baseline_is_clean(contract: dict) -> None:
    scorecard = _baseline_scorecard()
    assert absorption_progress_violations(scorecard, contract, _observations(scorecard)) == set()


# ── the committed fixtures ─────────────────────────────────────────────────


def test_committed_positive_fixture_is_clean(contract: dict) -> None:
    records = json.loads((_REPO_ROOT / POSITIVE_FIXTURE).read_text(encoding="utf-8"))
    assert records
    for record in records:
        assert absorption_progress_violations(record, contract, _observations(record)) == set()


def test_committed_negative_fixture_triggers_its_labelled_violation(contract: dict) -> None:
    records = json.loads((_REPO_ROOT / NEGATIVE_FIXTURE).read_text(encoding="utf-8"))
    assert records
    observed: set[str] = set()
    for entry in records:
        label = entry["violation"]
        violations = absorption_progress_violations(
            entry["record"], contract, _observations(entry["record"])
        )
        observed |= violations
        assert label in violations, (label, sorted(violations))

    rule = yaml.safe_load((_REPO_ROOT / RULE_PATH).read_text(encoding="utf-8"))
    required = rule["fixtures"]["negative"]["expect"]["require_violations"]
    assert set(required) - observed == set()
    assert ADAPTER_LABELS <= set(required)


# ── identity: hash / path / revision ───────────────────────────────────────


def test_substituted_evidence_hash_is_flagged_and_resolves_when_restored(contract: dict) -> None:
    substituted = _mutated(
        lambda card: card["measurements"][1]["evidence"].__setitem__("sha256", "sha256:" + "b" * 64)
    )
    assert "evidence_hash_substituted" in absorption_progress_violations(
        substituted, contract, _observations(substituted)
    )

    honest = _baseline_scorecard()
    honest["observations"]["evidence_sha256"][EVIDENCE] = "sha256:" + "b" * 64
    assert "evidence_hash_substituted" in absorption_progress_violations(
        honest, contract, _observations(honest)
    )


def test_scalar_revision_mismatch_is_flagged(contract: dict) -> None:
    card = _mutated(
        lambda c: c["measurements"][1]["evidence"].__setitem__("source_revision", REV2)
    )
    assert "evidence_revision_mismatch" in absorption_progress_violations(
        card, contract, _observations(card)
    )


def test_evidence_path_must_be_repository_relative(contract: dict) -> None:
    card = _mutated(
        lambda c: c["measurements"][1]["evidence"].__setitem__("path", "/root/absolute.json")
    )
    assert "evidence_path_not_relative" in absorption_progress_violations(
        card, contract, _observations(card)
    )


def test_missing_evidence_artifact_is_flagged(contract: dict) -> None:
    card = _mutated(
        lambda c: c["measurements"][1]["evidence"].update(
            {"path": "artifacts/parse_compat/missing_probe.json", "sha256": "sha256:" + "c" * 64}
        )
    )
    card["observations"]["evidence_sha256"]["artifacts/parse_compat/missing_probe.json"] = None
    assert "evidence_artifact_missing" in absorption_progress_violations(
        card, contract, _observations(card)
    )


# ── the accepted series ────────────────────────────────────────────────────


def test_changed_oracle_is_flagged(contract: dict) -> None:
    card = _mutated(
        lambda c: c["measurements"][1].__setitem__(
            "comparability", _comparability(oracle="other_oracle")
        )
    )
    labels = absorption_progress_violations(card, contract, _observations(card))
    assert "oracle_changed" in labels


def test_scope_outside_the_accepted_series_is_not_comparable(contract: dict) -> None:
    card = _mutated(
        lambda c: c["measurements"][1].__setitem__(
            "comparability", _comparability(corpus_id="some-other-corpus")
        )
    )
    labels = absorption_progress_violations(card, contract, _observations(card))
    assert "scope_not_accepted" in labels
    # A different corpus is a new series, not a regression: the denominator and
    # coverage comparisons must not run against it.
    assert "denominator_shrink" not in labels
    assert "coverage_regression" not in labels


def test_two_series_inside_one_scorecard_are_refused(contract: dict) -> None:
    card = _mutated(
        lambda c: c["measurements"][1].__setitem__(
            "comparability", _comparability(profile="rich")
        )
    )
    assert "scope_mixed_in_scorecard" in absorption_progress_violations(
        card, contract, _observations(card)
    )


# ── direction: coverage and denominator ────────────────────────────────────


def test_coverage_regression_is_flagged(contract: dict) -> None:
    card = _mutated(
        lambda c: c["measurements"].__setitem__(
            1,
            {
                "metric_id": "extraction.native_capability_recall",
                "axis": "extraction",
                "state": "not_measured",
                "reason": "not_run",
            },
        )
    )
    assert "coverage_regression" in absorption_progress_violations(
        card, contract, _observations(card)
    )


def test_denominator_shrink_is_flagged(contract: dict) -> None:
    card = _mutated(
        lambda c: c.__setitem__(
            "denominator", _denominator(assigned=600, executed=521)
        )
    )
    assert "denominator_shrink" in absorption_progress_violations(
        card, contract, _observations(card)
    )


def test_dropped_exclusion_is_flagged_through_the_composed_oracle(contract: dict) -> None:
    dropped_reason = _mutated(
        lambda c: c["denominator"]["exclusion_reasons"].pop("header_title_absent")
    )
    assert "unnamed_exclusions" in absorption_progress_violations(
        dropped_reason, contract, _observations(dropped_reason)
    )

    recut = _mutated(
        lambda c: c.__setitem__(
            "denominator",
            _denominator(
                executed=530,
                excluded=80,
                exclusion_reasons={
                    "header_date_absent": 73,
                    "header_title_absent": 5,
                    "header_author_absent": 2,
                },
            ),
        )
    )
    assert "census_mismatch" in absorption_progress_violations(
        recut, contract, _observations(recut)
    )


def test_denominator_that_does_not_sum_is_flagged(contract: dict) -> None:
    card = _mutated(lambda c: c["denominator"].__setitem__("assigned", 611))
    assert "assigned_ne_total" in absorption_progress_violations(
        card, contract, _observations(card)
    )


# ── the per-document census (S02/T03) ──────────────────────────────────────

_CENSUS_PROBE = "artifacts/parse_compat/fixture_probe.json"
_CENSUS_MANIFEST = "data/parse/fixture_manifest.yaml"
_CENSUS_PROBE_SHA = "sha256:" + "b" * 64
_CENSUS_MANIFEST_SHA = "sha256:" + "c" * 64
_CENSUS_PARTITION = {"assigned": 3, "executed": 2, "refused": 0, "excluded": 1}
_CENSUS_PROJECTION = [
    ["fixture-a", "executed"],
    ["fixture-b", "executed"],
    ["fixture-c", "excluded:header_title_absent"],
]


def _census_sources() -> dict:
    return {
        "probe": {"path": _CENSUS_PROBE, "sha256": _CENSUS_PROBE_SHA},
        "manifest": {"path": _CENSUS_MANIFEST, "sha256": _CENSUS_MANIFEST_SHA},
    }


def _census_declared(projection=None, partition=None, refused=None, reasons=None) -> dict:
    projection = copy.deepcopy(projection if projection is not None else _CENSUS_PROJECTION)
    return {
        "census_id": "fixture-census-3-2-1",
        "identity": "assigned = executed + refused + excluded",
        "partition": dict(partition or _CENSUS_PARTITION),
        "refusal_semantics": {
            "closed": True,
            "refused_document_ids": list(refused if refused is not None else []),
        },
        "exclusion_reasons": {
            "closed": True,
            "values": dict(reasons or {"header_title_absent": 1}),
        },
        "authority": _census_sources(),
        "derivation": {"match_by": "document_id"},
        "projection": projection,
        "projection_sha256": projection_digest(projection),
    }


def _census_observation() -> dict:
    return {
        "available": True,
        "census_id": "fixture-census-3-2-1",
        "match_by": "document_id",
        "sources": _census_sources(),
        "partition": dict(_CENSUS_PARTITION),
        "refused_document_ids": [],
        "exclusion_reasons": {"header_title_absent": 1},
        "projection": copy.deepcopy(_CENSUS_PROJECTION),
        "projection_sha256": projection_digest(_CENSUS_PROJECTION),
        "list_counts": {
            "manifest_articles": 3,
            "probe_outcomes": 2,
            "probe_exceptions": 1,
        },
        "declared_counts": {
            "evaluated_documents": 2,
            "executed_documents": 2,
            "reference_documents": 3,
            "reference_measurable": 2,
            "reference_excluded": 1,
        },
    }


def _census_scorecard(**overrides) -> dict:
    return {
        "schema_version": "absorption-progress.v1",
        "scorecard_id": "m514-absorption-progress-census-fixture",
        "denominator": {
            "assigned": 3,
            "executed": 2,
            "refused": 0,
            "excluded": 1,
            "exclusion_reasons": {"header_title_absent": 1},
        },
        "population_census": _census_declared(**overrides),
        "measurements": [
            {"metric_id": "java_independence.java_free_parse_route", "axis": "java_independence", "state": "not_measured", "reason": "java_production_route_active"},
            {"metric_id": "extraction.native_capability_recall", "axis": "extraction", "state": "not_measured", "reason": "not_run"},
            {"metric_id": "preservation.occurrence_conservation", "axis": "preservation", "state": "not_measured", "reason": "not_run"},
            {"metric_id": "resources.native_savings", "axis": "resources", "state": "not_measured", "reason": "native_savings_not_measured"},
            {"metric_id": "reliability.rollback_rehearsal", "axis": "reliability", "state": "not_measured", "reason": "rehearsal_only_not_a_product_measurement"},
        ],
        "authorization": {
            "state": "owner_directive_research",
            "decision": "D491",
            "product_coverage_effect": "none",
        },
        "lineage": {},
        "threshold_freeze": {
            "state": "frozen",
            "revision": "sha256:" + "d" * 64,
            "frozen_at": "2026-09-29",
            "source": SCHEMA_PATH,
        },
    }


def _census_labels(card: dict, observation: dict, contract: dict) -> set[str]:
    return absorption_progress_violations(
        card, contract, {"population_census": observation}
    )


def test_clean_census_projection_is_not_flagged(contract: dict) -> None:
    assert _census_labels(_census_scorecard(), _census_observation(), contract) == set()


def test_dropped_census_document_is_flagged(contract: dict) -> None:
    dropped = [pair for pair in copy.deepcopy(_CENSUS_PROJECTION) if pair[0] != "fixture-c"]
    labels = _census_labels(
        _census_scorecard(projection=dropped), _census_observation(), contract
    )
    assert "census_document_dropped" in labels


def test_swapped_census_disposition_is_flagged(contract: dict) -> None:
    swapped = copy.deepcopy(_CENSUS_PROJECTION)
    swapped[0][1] = "refused"
    assert "census_projection_mismatch" in _census_labels(
        _census_scorecard(projection=swapped), _census_observation(), contract
    )


def test_untyped_census_reason_is_flagged(contract: dict) -> None:
    untyped = copy.deepcopy(_CENSUS_PROJECTION)
    untyped[2][1] = "excluded:weird_reason"
    labels = _census_labels(
        _census_scorecard(projection=untyped, reasons={"weird_reason": 1}),
        _census_observation(),
        contract,
    )
    assert "census_disposition_untyped" in labels


def test_refused_boundary_move_is_flagged(contract: dict) -> None:
    moved = copy.deepcopy(_CENSUS_PROJECTION)
    moved[0][1] = "refused"
    labels = _census_labels(
        _census_scorecard(
            projection=moved,
            partition={"assigned": 3, "executed": 1, "refused": 1, "excluded": 1},
            refused=["fixture-a"],
        ),
        _census_observation(),
        contract,
    )
    assert "refused_set_unbound" in labels


def test_census_is_unverified_without_a_recomputed_observation(contract: dict) -> None:
    """No derived observation → the per-document half is unverified, not assumed."""
    card = _census_scorecard()
    assert absorption_progress_violations(card, contract, {"evidence_sha256": {}}) == set()


def test_census_authority_must_name_the_recomputed_bytes(contract: dict) -> None:
    observation = _census_observation()
    observation["sources"] = {
        "probe": {"path": _CENSUS_PROBE, "sha256": "sha256:" + "f" * 64},
        "manifest": {"path": _CENSUS_MANIFEST, "sha256": _CENSUS_MANIFEST_SHA},
    }
    assert "census_projection_mismatch" in _census_labels(
        _census_scorecard(), observation, contract
    )


# ── resources: savings need a paired CPU measurement ───────────────────────


def _with_savings_claim(*, paired_row: dict | None) -> dict:
    def mutate(card: dict) -> None:
        card["measurements"].append(
            {
                "metric_id": "resources.native_savings",
                "axis": "resources",
                "state": "not_measured",
                "savings_claimed": True,
                "paired_cpu_metric_id": "resources.cpu_baseline",
            }
        )
        if paired_row is not None:
            card["measurements"].append(paired_row)

    return _mutated(mutate)


def test_savings_without_paired_cpu_is_flagged(contract: dict) -> None:
    card = _with_savings_claim(paired_row=None)
    assert "savings_without_paired_cpu" in absorption_progress_violations(
        card, contract, _observations(card)
    )


def test_savings_with_unmeasured_cpu_is_flagged(contract: dict) -> None:
    card = _with_savings_claim(
        paired_row={
            "metric_id": "resources.cpu_baseline",
            "axis": "resources",
            "kind": "cpu",
            "state": "not_measured",
        }
    )
    assert "savings_paired_cpu_not_measured" in absorption_progress_violations(
        card, contract, _observations(card)
    )


# ── lifecycle is not product ───────────────────────────────────────────────


def test_skipped_as_delivered_is_flagged(contract: dict) -> None:
    card = _mutated(
        lambda c: c["measurements"][1]["evidence"].__setitem__("kind", "lifecycle_state")
    )
    assert "skipped_as_delivered" in absorption_progress_violations(
        card, contract, _observations(card)
    )


# ── the reader port ────────────────────────────────────────────────────────


def test_reader_confines_paths_and_computes_hashes() -> None:
    root = _make_root()
    _write(root / "data" / "doc.json", '{"ok": true}\n')
    reader = FilesystemAbsorptionProgressReader(root, "data/schema.yaml", "data/doc.json")

    assert reader.path_exists("data/doc.json") is True
    assert reader.path_exists("/etc/passwd") is False
    assert reader.path_exists("../outside.json") is False
    assert reader.file_sha256("/etc/passwd") is None
    expected = "sha256:" + hashlib.sha256((root / "data" / "doc.json").read_bytes()).hexdigest()
    assert reader.file_sha256("data/doc.json") == expected
    assert reader.read_contract() is None
    assert reader.read_scorecard() == {"ok": True}


# ── end to end through the service ─────────────────────────────────────────


def _scratch_service(scorecard: dict | None = None) -> TrajectoryService:
    """A scratch tree holding exactly what the check reads."""
    root = _make_root()
    (root / "artifacts").mkdir(parents=True, exist_ok=True)
    for rel in (RULE_PATH, SCHEMA_PATH, POSITIVE_FIXTURE, NEGATIVE_FIXTURE, DOMAIN_MODULE, ADAPTER_MODULE, REFERENCE_PINS_PATH):
        _write(root / rel, (_REPO_ROOT / rel).read_text(encoding="utf-8"))
    # Historical carriers are only existence-checked here; placeholders keep the
    # scratch tree small while still exercising the guard.
    schema = yaml.safe_load((_REPO_ROOT / SCHEMA_PATH).read_text(encoding="utf-8"))
    for carrier in schema["historical_carriers"]["carriers"]:
        rel = str(carrier.get("path") or "").split("#", 1)[0]
        if rel:
            _write(root / rel, "placeholder\n")
    if scorecard is not None:
        _write(root / SCORECARD_PATH, json.dumps(scorecard, indent=2))
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = root
    return service


def test_absent_required_scorecard_is_a_finding() -> None:
    """`scorecard.required: true` since S02/T02: absence is a finding, not silence.

    Before the freeze, an absent scorecard was `not_measured`. Now that the
    governed baseline is committed and the freeze is bound to it, deleting the
    artifact must be drift rather than a way to silence the check.
    """
    service = _scratch_service(scorecard=None)
    assert [f.flag for f in service._check_absorption_progress()] == [
        "absorption_progress_scorecard_not_measured"
    ]


def test_clean_committed_scorecard_is_not_flagged() -> None:
    root = _make_root()
    evidence_bytes = b'{"probe": true}\n'
    rel = EVIDENCE
    (root / rel).parent.mkdir(parents=True, exist_ok=True)
    (root / rel).write_bytes(evidence_bytes)
    digest = "sha256:" + hashlib.sha256(evidence_bytes).hexdigest()

    scorecard = copy.deepcopy(_baseline_scorecard())
    for row in scorecard["measurements"]:
        if isinstance(row.get("evidence"), dict):
            row["evidence"]["sha256"] = digest
    scorecard.pop("observations")
    service = _scratch_service(scorecard=scorecard)
    # Move the evidence artifact into the scratch root the service reads.
    (service.root / rel).parent.mkdir(parents=True, exist_ok=True)
    (service.root / rel).write_bytes(evidence_bytes)
    assert service._check_absorption_progress() == []


def test_broken_committed_scorecard_is_flagged() -> None:
    root = _make_root()
    evidence_bytes = b'{"probe": true}\n'
    rel = EVIDENCE
    (root / rel).parent.mkdir(parents=True, exist_ok=True)
    (root / rel).write_bytes(evidence_bytes)
    digest = "sha256:" + hashlib.sha256(evidence_bytes).hexdigest()

    scorecard = copy.deepcopy(_baseline_scorecard())
    for row in scorecard["measurements"]:
        if isinstance(row.get("evidence"), dict):
            row["evidence"]["sha256"] = digest
    # Substitute the oracle: the scope is not the accepted series.
    scorecard["measurements"][1]["comparability"]["oracle"] = "other_oracle"
    scorecard.pop("observations")
    service = _scratch_service(scorecard=scorecard)
    (service.root / rel).parent.mkdir(parents=True, exist_ok=True)
    (service.root / rel).write_bytes(evidence_bytes)

    flags = service._check_absorption_progress()
    broken = [f for f in flags if f.flag == "absorption_progress_broken"]
    assert broken, [f.flag for f in flags]
    assert "oracle_changed" in broken[0].evidence


def test_edited_freeze_revision_in_a_scratch_tree_is_flagged() -> None:
    root = _make_root()
    evidence_bytes = b'{"probe": true}\n'
    rel = EVIDENCE
    (root / rel).parent.mkdir(parents=True, exist_ok=True)
    (root / rel).write_bytes(evidence_bytes)
    digest = "sha256:" + hashlib.sha256(evidence_bytes).hexdigest()

    scorecard = copy.deepcopy(_baseline_scorecard())
    for row in scorecard["measurements"]:
        if isinstance(row.get("evidence"), dict):
            row["evidence"]["sha256"] = digest
    # Re-tune the carrier: the declared revision no longer matches the schema bytes.
    scorecard["threshold_freeze"]["revision"] = "sha256:" + "e" * 64
    scorecard.pop("observations")
    service = _scratch_service(scorecard=scorecard)
    (service.root / rel).parent.mkdir(parents=True, exist_ok=True)
    (service.root / rel).write_bytes(evidence_bytes)

    flags = service._check_absorption_progress()
    broken = [f for f in flags if f.flag == "absorption_progress_broken"]
    assert broken, [f.flag for f in flags]
    assert "threshold_freeze_revision_mismatch" in broken[0].evidence


def test_corrupt_committed_scorecard_fails_closed() -> None:
    """A scorecard that exists but cannot be parsed must not read as absent."""
    service = _scratch_service(scorecard=None)
    _write(service.root / SCORECARD_PATH, "{not valid json")
    flags = service._check_absorption_progress()
    assert [f.flag for f in flags] == ["absorption_progress_scorecard_invalid"]


def test_missing_historical_carrier_is_flagged() -> None:
    service = _scratch_service(scorecard=None)
    victim = "doc/review/M513-S07-release-decision-package.md"
    assert (service.root / victim).is_file()
    (service.root / victim).unlink()
    flags = service._check_absorption_progress()
    missing = [f for f in flags if f.flag == "absorption_progress_historical_guard_missing"]
    assert missing, [f.flag for f in flags]
    assert victim in missing[0].evidence


def test_real_tree_has_no_absorption_progress_flag() -> None:
    """Positive control on the repository: contract, fixtures and scorecard are honest.

    Since S02 the governed scorecard is committed and `required`, so the real tree
    must validate clean rather than merely be absent.
    """
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    assert service._check_absorption_progress() == []


def test_absorption_progress_is_wired_into_build_report() -> None:
    import inspect

    source = inspect.getsource(TrajectoryService.build_report)
    assert "_check_absorption_progress()" in source
