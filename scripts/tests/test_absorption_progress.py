"""M514-1lss2s/S01/T01 tests for the absorption progress scorecard contract.

The closed vocabulary is DATA (``data/parse/absorption_progress.schema.yaml``)
and the validator is pure (``scripts/governor/domain/absorption_progress.py``).
These tests prove the vocabulary is closed, that the recorded M514 initial
census validates as a positive baseline, and that each named false-progress
shape is refused:

* denominator mutation — ``assigned != executed + refused + excluded``;
* zero population — an empty population is never read as one (perfect) or zero
  (failure);
* mixed scopes — two series are not comparable;
* skipped-as-delivered — a terminal GSD state never becomes product population;
* dropped exclusion — the 79 typed exclusions stay inside the pinned census.

Tests read the tracked DATA contract and use inline fixtures only.
"""

from __future__ import annotations

import copy
from pathlib import Path

import pytest
import yaml

from governor.domain.absorption_progress import (
    comparability_key,
    denominator_violations,
    measurement_violations,
    missing_contract_vocabulary,
    scorecard_violations,
)

ROOT = Path(__file__).resolve().parents[2]
CONTRACT_PATH = ROOT / "data/parse/absorption_progress.schema.yaml"

REVISION = "git:" + "52bb08a4a016e69e3224ae79f02c2f3e53897e72"
SHA = "sha256:" + "a" * 64


@pytest.fixture(scope="module")
def contract() -> dict:
    return yaml.safe_load(CONTRACT_PATH.read_text(encoding="utf-8"))


# ── fixtures ────────────────────────────────────────────────────────────────


def _comparability(**overrides) -> dict:
    block = {
        "corpus_id": "m513-holdout-610",
        "profile": "structural",
        "oracle": "external_holdout_probe_semantics",
        "source_revision": REVISION,
    }
    block.update(overrides)
    return block


def _evidence(**overrides) -> dict:
    block = {
        "path": "artifacts/parse_compat/external_holdout_release_probe.json",
        "sha256": SHA,
        "source_revision": REVISION,
        "tested_revision": REVISION,
        "kind": "executed_run",
    }
    block.update(overrides)
    return block


def _measured_row(metric_id: str = "extraction.native_capability_recall", **overrides) -> dict:
    row = {
        "metric_id": metric_id,
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
    }
    row.update(overrides)
    return row


def _denominator() -> dict:
    return {
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


def _scorecard(contract: dict, measurements: list, **overrides) -> dict:
    """A structurally valid scorecard; ``census_id`` is opt-in per test."""
    card = {
        "schema_version": contract["schema_version"],
        "scorecard_id": "test-scorecard",
        "denominator": _denominator(),
        "measurements": measurements,
        "authorization": {
            "state": "owner_directive_research",
            "decision": "D491",
            "product_coverage_effect": "none",
        },
        "lineage": {},
    }
    card.update(overrides)
    return card


def _baseline(contract: dict) -> dict:
    """The recorded initial census (исходная перепись): measured where measured,
    silent where the value does not exist, with the 79 typed exclusions kept."""
    return _scorecard(
        contract,
        [
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
            _measured_row("extraction.native_capability_recall"),
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
        census_id="holdout-partition-610-531-79",
    )


# ── the contract itself ─────────────────────────────────────────────────────


def test_real_contract_is_closed_and_inventory_paths_exist(contract: dict) -> None:
    assert missing_contract_vocabulary(contract) == []

    census = contract["denominator"]["initial_census"]
    assert census["census_id"] == "holdout-partition-610-531-79"
    assert (census["declared"], census["measurable"], census["excluded"]) == (610, 531, 79)
    assert census["reason_counts"] == {
        "header_date_absent": 72,
        "header_title_absent": 5,
        "header_author_absent": 2,
    }
    assert contract["threshold_policy"]["state"] == "not_frozen"
    assert contract["product_axes"]["authorization_not_product"] is True
    assert contract["preservation"]["separate_from_extraction"] is True

    # The inventory is an inventory: every referenced rule/authority exists.
    for rule in contract["existing_rules"]:
        target = str(rule["path"]).split("#", 1)[0]
        assert not target.startswith("/"), rule
        assert (ROOT / target).is_file(), rule

    for carrier in contract["historical_carriers"]["carriers"]:
        path = carrier.get("path")
        if path:
            target = str(path).split("#", 1)[0]
            assert not target.startswith("/"), carrier
            assert (ROOT / target).is_file(), carrier


def test_positive_baseline_census_is_clean(contract: dict) -> None:
    card = _baseline(contract)
    assert scorecard_violations(card, contract) == set()
    # The full denominator is the point: the 79 exclusions stay inside 610.
    assert card["denominator"]["assigned"] == 531 + 0 + 79


# ── denominator mutations ───────────────────────────────────────────────────


@pytest.mark.parametrize(
    "mutate,expected",
    [
        (lambda card: card["denominator"].__setitem__("assigned", 611), "assigned_ne_total"),
        (lambda card: card["denominator"].__setitem__("executed", 530), "assigned_ne_total"),
        (lambda card: card["denominator"].__setitem__("excluded", 78), "assigned_ne_total"),
        (
            lambda card: card["denominator"].__setitem__("refused", True),
            "denominator_component_invalid",
        ),
        (
            lambda card: card["denominator"]["exclusion_reasons"].__setitem__(
                "header_missing", 1
            ),
            "exclusion_reason_untyped",
        ),
        (
            lambda card: card["denominator"]["exclusion_reasons"].pop("header_title_absent"),
            "unnamed_exclusions",
        ),
    ],
)
def test_denominator_mutation_is_refused(contract, mutate, expected) -> None:
    card = _baseline(contract)
    mutate(card)
    assert expected in scorecard_violations(card, contract)


def test_denominator_violation_oracle_is_reusable(contract: dict) -> None:
    broken = _denominator()
    broken["assigned"] = 500
    assert "assigned_ne_total" in denominator_violations(broken, contract)
    assert denominator_violations(_denominator(), contract) == set()


# ── zero population ─────────────────────────────────────────────────────────


def _zero_population(contract: dict, record: dict) -> dict:
    card = _scorecard(
        contract,
        [record],
        denominator={
            "assigned": 0,
            "executed": 0,
            "refused": 0,
            "excluded": 0,
            "exclusion_reasons": {},
        },
    )
    return card


def test_zero_population_is_no_denominator_not_a_number(contract: dict) -> None:
    silent = {
        "metric_id": "extraction.native_capability_recall",
        "axis": "extraction",
        "state": "no_denominator",
        "reason": "no_documents_assigned_in_this_scope",
    }
    assert scorecard_violations(_zero_population(contract, silent), contract) == set()

    claimed = _measured_row(
        strata={"micro_occurrence": 1.0, "affiliation": 0.5, "weakest_stratum": 0.5}
    )
    claimed["score"] = {"numerator": 1, "denominator": 1}
    assert "empty_population_given_value" in scorecard_violations(
        _zero_population(contract, claimed), contract
    )

    zero_fraction = {
        "metric_id": "extraction.native_capability_recall",
        "axis": "extraction",
        "state": "no_denominator",
        "score": {"numerator": 1, "denominator": 0},
        "strata": {"micro_occurrence": 1.0, "affiliation": 0.5, "weakest_stratum": 0.5},
    }
    violations = scorecard_violations(_zero_population(contract, zero_fraction), contract)
    assert {"no_denominator_treated_as_number", "value_with_no_denominator"} <= violations


# ── comparability / mixed scopes ────────────────────────────────────────────


def test_mixed_scope_comparison_is_not_comparable(contract: dict) -> None:
    left = _measured_row("extraction.native_capability_recall")
    right = _measured_row("extraction.native_capability_recall_new")
    right["comparability"] = _comparability(profile="rich")
    right["evidence"] = _evidence()
    right["compared_to"] = "extraction.native_capability_recall"

    assert "mixed_scope_comparison" in scorecard_violations(
        _scorecard(contract, [left, right]), contract
    )

    right["comparability"] = _comparability()
    assert "mixed_scope_comparison" not in scorecard_violations(
        _scorecard(contract, [left, right]), contract
    )

    right["compared_to"] = "extraction.does_not_exist"
    assert "comparison_target_missing" in scorecard_violations(
        _scorecard(contract, [left, right]), contract
    )


def test_comparability_key_requires_the_full_identity(contract: dict) -> None:
    assert comparability_key(_measured_row(), contract) is not None
    assert comparability_key(_measured_row(comparability={"profile": "structural"}), contract) is None
    assert comparability_key(_measured_row(comparability=None), contract) is None


# ── skipped-as-delivered ────────────────────────────────────────────────────


def test_skipped_as_delivered_is_refused(contract: dict) -> None:
    lifecycle_evidence = _measured_row(evidence=_evidence(kind="lifecycle_state"))
    assert "skipped_as_delivered" in scorecard_violations(
        _scorecard(contract, [lifecycle_evidence]), contract
    )

    delivered_by = _measured_row(delivered_by="skipped")
    assert "skipped_as_delivered" in scorecard_violations(
        _scorecard(contract, [delivered_by]), contract
    )

    card = _scorecard(contract, [_measured_row()])
    card["denominator"]["executed_evidence_kinds"] = ["complete"]
    assert "skipped_as_delivered" in scorecard_violations(card, contract)

    assert "skipped_as_delivered" not in scorecard_violations(
        _scorecard(contract, [_measured_row()]), contract
    )


# ── the pinned census ───────────────────────────────────────────────────────


def test_census_cannot_be_recut(contract: dict) -> None:
    unknown = _baseline(contract)
    unknown["census_id"] = "some-other-census"
    assert "census_identity_unknown" in scorecard_violations(unknown, contract)

    dropped = _baseline(contract)
    dropped["denominator"]["executed"] = 532
    dropped["denominator"]["excluded"] = 78
    dropped["denominator"]["exclusion_reasons"] = {
        "header_date_absent": 71,
        "header_title_absent": 5,
        "header_author_absent": 2,
    }
    assert dropped["denominator"]["assigned"] == 532 + 0 + 78
    assert "census_mismatch" in scorecard_violations(dropped, contract)

    assert scorecard_violations(_baseline(contract), contract) == set()


# ── thresholds ──────────────────────────────────────────────────────────────


def test_thresholds_cannot_be_invented_before_s02(contract: dict) -> None:
    assert "threshold_invented_before_freeze" in scorecard_violations(
        _scorecard(contract, [_measured_row(threshold=0.9)]), contract
    )
    assert "threshold_invented_before_freeze" in scorecard_violations(
        _scorecard(contract, [_measured_row(margin=0.05)]), contract
    )

    frozen = _scorecard(
        contract,
        [_measured_row()],
        threshold_freeze={
            "state": "frozen",
            "revision": REVISION,
            "frozen_at": "2026-09-29",
            "source": "data/parse/absorption_progress.schema.yaml",
        },
    )
    assert "threshold_invented_before_freeze" in scorecard_violations(frozen, contract)

    for block in ({"state": "frozen"}, {"state": "pending"}, {"state": "frozen", "revision": "x"}):
        card = _scorecard(contract, [_measured_row()], threshold_freeze=block)
        violations = scorecard_violations(card, contract)
        assert {"threshold_freeze_unbound", "threshold_invented_before_freeze"} & violations, block


# ── preservation is not extraction ──────────────────────────────────────────


def test_preservation_is_never_pooled_into_extraction(contract: dict) -> None:
    pooled = _measured_row(includes_preservation=True)
    assert "preservation_pooled_with_extraction" in scorecard_violations(
        _scorecard(contract, [pooled]), contract
    )

    preservation_row = _measured_row(
        "preservation.occurrence_conservation", axis="preservation", pooled_with=["extraction"]
    )
    assert "preservation_pooled_with_extraction" in scorecard_violations(
        _scorecard(contract, [preservation_row]), contract
    )

    both = _measured_row(pooled_with=["extraction", "preservation"])
    assert "preservation_pooled_with_extraction" in scorecard_violations(
        _scorecard(contract, [both]), contract
    )

    assert "preservation_pooled_with_extraction" not in scorecard_violations(
        _baseline(contract), contract
    )


# ── authorization separation ────────────────────────────────────────────────


def test_authorization_is_separate_from_product_coverage(contract: dict) -> None:
    missing = _scorecard(contract, [_measured_row()])
    del missing["authorization"]
    assert "authorization_block_missing" in scorecard_violations(missing, contract)

    unknown = _scorecard(
        contract, [_measured_row()], authorization={"state": "granted_forever"}
    )
    assert "unknown_authorization_state" in scorecard_violations(unknown, contract)

    inflated = _scorecard(
        contract,
        [_measured_row()],
        authorization={
            "state": "owner_directive_research",
            "product_coverage_effect": "increases_coverage",
        },
    )
    assert "authorization_used_as_product_evidence" in scorecard_violations(inflated, contract)

    authorized_evidence = _measured_row(evidence=_evidence(kind="authorization"))
    assert "authorization_used_as_product_evidence" in scorecard_violations(
        _scorecard(contract, [authorized_evidence]), contract
    )

    unbound_release = _scorecard(
        contract,
        [_measured_row()],
        authorization={"state": "human_release_recorded", "product_coverage_effect": "none"},
    )
    assert "authorization_release_unbound" in scorecard_violations(unbound_release, contract)


# ── historical carriers ─────────────────────────────────────────────────────


def test_historical_carriers_are_read_only(contract: dict) -> None:
    replaced = _scorecard(contract, [_measured_row()], lineage={"replaces": "m513-release-pin"})
    assert "historical_carrier_rewritten" in scorecard_violations(replaced, contract)

    superseded = _scorecard(
        contract,
        [_measured_row()],
        lineage={"supersedes": ["m513-holdout-manifest-pin"]},
    )
    assert "historical_carrier_rewritten" in scorecard_violations(superseded, contract)

    malformed = _scorecard(contract, [_measured_row()], lineage="not-a-mapping")
    assert "lineage_invalid" in scorecard_violations(malformed, contract)

    assert "historical_carrier_rewritten" not in scorecard_violations(
        _scorecard(contract, [_measured_row()]), contract
    )


# ── evidence binding ────────────────────────────────────────────────────────


def test_evidence_binding_shape_is_enforced(contract: dict) -> None:
    without_evidence = _measured_row()
    del without_evidence["evidence"]
    assert "evidence_binding_missing" in scorecard_violations(
        _scorecard(contract, [without_evidence]), contract
    )

    bad_hash = _measured_row(evidence=_evidence(sha256="sha256:not-hex"))
    assert "evidence_binding_malformed" in scorecard_violations(
        _scorecard(contract, [bad_hash]), contract
    )

    bad_revision = _measured_row(evidence=_evidence(source_revision="main"))
    assert "evidence_binding_malformed" in scorecard_violations(
        _scorecard(contract, [bad_revision]), contract
    )

    mismatched = _measured_row(
        evidence=_evidence(source_revision="git:" + "b" * 40)
    )
    assert "evidence_revision_mismatch" in scorecard_violations(
        _scorecard(contract, [mismatched]), contract
    )

    no_identity = _measured_row(comparability=None)
    assert "comparability_fields_missing" in scorecard_violations(
        _scorecard(contract, [no_identity]), contract
    )


# ── strata ──────────────────────────────────────────────────────────────────


def test_strata_must_name_the_weakest(contract: dict) -> None:
    micro_only = _measured_row(
        strata={"micro_occurrence": 0.9, "weakest_stratum": 0.9}
    )
    assert "stratum_micro_only" in scorecard_violations(
        _scorecard(contract, [micro_only]), contract
    )

    missing_weakest = _measured_row(strata={"micro_occurrence": 0.9})
    assert "weakest_stratum_absent" in scorecard_violations(
        _scorecard(contract, [missing_weakest]), contract
    )

    lying = _measured_row(
        strata={
            "micro_occurrence": 0.9,
            "affiliation": 0.2,
            "weakest_stratum": 0.5,
        }
    )
    assert "weakest_stratum_not_weakest" in scorecard_violations(
        _scorecard(contract, [lying]), contract
    )


# ── closed vocabulary / fail-closed ─────────────────────────────────────────


def test_unknown_vocabulary_fails_closed(contract: dict) -> None:
    assert "unknown_axis" in measurement_violations(
        _measured_row(axis="vibes"), contract
    )
    assert "unknown_measurement_state" in measurement_violations(
        _measured_row(state="mostly_measured"), contract
    )
    assert "value_with_not_measured" in measurement_violations(
        _measured_row(state="not_measured"), contract
    )
    assert "measured_without_score" in measurement_violations(
        _measured_row(score=None), contract
    )

    stripped = copy.deepcopy(contract)
    del stripped["product_axes"]
    assert "product_axes" in missing_contract_vocabulary(stripped)
    assert scorecard_violations(_baseline(contract), stripped) == {"contract_vocabulary_missing"}

    assert scorecard_violations("not-a-mapping", contract) == {"scorecard_not_an_object"}


def test_scorecard_schema_version_must_match_its_contract(contract: dict) -> None:
    stale = _baseline(contract)
    stale["schema_version"] = "absorption-progress.v0"
    assert "unsupported_scorecard_schema" in scorecard_violations(stale, contract)
