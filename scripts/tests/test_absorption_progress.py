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
import hashlib
import json
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

#: The frozen freeze declaration the inline scorecards carry by default. S02 made
#: the declaration mandatory under a frozen contract, so every fixture that is
#: meant to be otherwise clean must carry a bound one.
THRESHOLD_FREEZE = {
    "state": "frozen",
    "revision": "sha256:" + "d" * 64,
    "frozen_at": "2026-09-29",
    "source": "data/parse/absorption_progress.schema.yaml",
}


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
        "threshold_freeze": dict(THRESHOLD_FREEZE),
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
    assert contract["threshold_policy"]["state"] == "frozen"
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


# ── thresholds (S02: frozen, reference-derived, revision-bound) ─────────────


def test_real_contract_freezes_thresholds_before_scoring(contract: dict) -> None:
    policy = contract["threshold_policy"]
    assert policy["state"] == "frozen"
    assert policy["tuned_after_scoring"] == "forbidden"
    assert policy["pass_requires_frozen_threshold"] is True
    assert policy["numeric_thresholds_present"] is True
    assert policy["numeric_before_freeze"] == "forbidden"
    assert policy["source"] == "artifacts/parse_compat/absorption_progress_baseline.json"
    # `freeze_revision` is the frozen evidence run, recomputed here rather than
    # trusted: a hand-typed revision would fail this assertion.
    probe = ROOT / "artifacts/parse_compat/external_holdout_release_probe.json"
    assert policy["freeze_revision"] == "sha256:" + hashlib.sha256(probe.read_bytes()).hexdigest()
    # No numeric value is typed into the contract: every threshold is a reference
    # to the artifact that produced it, so re-tuning requires editing the carrier
    # (which invalidates the freeze) rather than editing a literal.
    for entry in policy["thresholds"]:
        assert "value" not in entry, entry
        assert entry["margin"] == 0
        assert entry["direction"] in policy["direction_vocabulary"]
        assert entry["basis"] in policy["basis_vocabulary"]
        assert entry["reference"]["path"] == policy["source"]
        assert not str(entry["reference"]["path"]).startswith("/")
        assert entry["reference"]["pointer"]


def test_frozen_thresholds_cover_exactly_the_required_families(contract: dict) -> None:
    policy = contract["threshold_policy"]
    covered = {entry["family_id"] for entry in policy["thresholds"]}
    required = {f["family_id"] for f in _family_rows(contract) if f["state"] == "required"}
    pending = {f["family_id"] for f in _family_rows(contract) if f["state"] == "threshold_pending"}
    assert covered == required
    assert covered & pending == set()

    # A required family without a threshold means the freeze governs a denominator
    # it does not gate: a partial freeze, not a cheaper one.
    partial = copy.deepcopy(contract)
    partial["threshold_policy"]["thresholds"] = partial["threshold_policy"]["thresholds"][:-1]
    assert missing_contract_vocabulary(partial) == []
    assert "threshold_missing_for_required_metric" in scorecard_violations(
        _baseline(partial), partial
    )

    # A threshold on a family the corpus cannot measure would read as a pass the
    # measurement cannot earn (D496).
    pending_claimed = copy.deepcopy(contract)
    victim = sorted(pending)[0]
    pending_claimed["threshold_policy"]["thresholds"][0]["family_id"] = victim
    violations = scorecard_violations(_baseline(pending_claimed), pending_claimed)
    assert "family_claimed_without_required_scope" in violations
    assert "threshold_missing_for_required_metric" in violations

    # A freeze with no entry at all is an incomplete vocabulary, not a valid one.
    empty = copy.deepcopy(contract)
    empty["threshold_policy"]["thresholds"] = []
    assert "threshold_policy.thresholds" in missing_contract_vocabulary(empty)


def test_thresholds_are_refused_before_the_freeze_and_bound_after(contract: dict) -> None:
    # Before the freeze a numeric threshold is invented, not measured.
    unfrozen = copy.deepcopy(contract)
    unfrozen["threshold_policy"]["state"] = "not_frozen"
    assert "threshold_invented_before_freeze" in scorecard_violations(
        _scorecard(unfrozen, [_measured_row(threshold=0.9)]), unfrozen
    )
    assert "threshold_invented_before_freeze" in scorecard_violations(
        _scorecard(unfrozen, [_measured_row(margin=0.05)]), unfrozen
    )
    claimed = _scorecard(unfrozen, [_measured_row()], threshold_freeze=dict(THRESHOLD_FREEZE))
    assert "threshold_invented_before_freeze" in scorecard_violations(claimed, unfrozen)

    # Frozen: the freeze declaration is mandatory and must be bound.
    missing = _scorecard(contract, [_measured_row()])
    missing.pop("threshold_freeze")
    assert "threshold_freeze_unbound" in scorecard_violations(missing, contract)
    for block in ({"state": "frozen"}, {"state": "pending"}, {"state": "frozen", "revision": "x"}):
        card = _scorecard(contract, [_measured_row()], threshold_freeze=block)
        assert "threshold_freeze_unbound" in scorecard_violations(card, contract), block

    # Frozen: a measured row may only publish a threshold the contract declares.
    published = _measured_row()
    published["threshold"] = {
        "threshold_id": "extraction_micro_occurrence",
        "direction": "at_least",
        "margin": 0,
        "value": 0.945771,
        "state": "frozen",
    }
    assert "threshold_missing_for_required_metric" not in scorecard_violations(
        _scorecard(contract, [published]), contract
    )
    invented = _measured_row()
    invented["threshold"] = {
        "threshold_id": "not_declared_anywhere",
        "direction": "at_least",
        "margin": 0,
        "value": 0.9,
        "state": "frozen",
    }
    assert "threshold_missing_for_required_metric" in scorecard_violations(
        _scorecard(contract, [invented]), contract
    )
    bounded = _measured_row()
    bounded["threshold"] = {
        "threshold_id": "extraction_micro_occurrence",
        "direction": "at_least",
        "margin": 0,
        "value": None,
        "state": "frozen",
    }
    assert "threshold_missing_for_required_metric" in scorecard_violations(
        _scorecard(contract, [bounded]), contract
    )


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


# ── S02: required scope, assigned population, independent corpus ────────────


def _family_rows(contract: dict) -> list[dict]:
    return contract["required_scope"]["families"]


def _family_ids(contract: dict) -> list[str]:
    return [family["family_id"] for family in _family_rows(contract)]


def _required_measured_ids(contract: dict) -> list[str]:
    return [f["family_id"] for f in _family_rows(contract) if f["state"] == "required"]


def _live_parity_required_rows() -> list[dict]:
    carrier = yaml.safe_load(
        (ROOT / "data/parse/grobid_compatibility.yaml").read_text(encoding="utf-8")
    )
    return [
        row
        for row in carrier["parity_ledger"]["rows"]
        if row.get("scope") == "required"
    ]


def _live_holdout_articles() -> list[dict]:
    manifest = yaml.safe_load(
        (ROOT / "data/parse/holdout_manifests.yaml").read_text(encoding="utf-8")
    )
    return manifest["articles"]


def test_required_scope_reuses_the_parity_ledger_required_rows(contract: dict) -> None:
    """The family list is a projection of the carrier, not a second authority."""
    scope = contract["required_scope"]
    assert scope["source"]["path"] == "data/parse/grobid_compatibility.yaml"
    assert scope["source"]["pointer"] == "parity_ledger.rows"

    projected = [
        (
            family["family_id"],
            family["measurement_identity"],
            family["state"],
            family.get("reason"),
        )
        for family in _family_rows(contract)
    ]
    derived = [
        (
            row["capability"],
            row["measurement"]["state"],
            "required"
            if row["measurement"]["state"] == scope["measured_identity"]
            else "threshold_pending",
            row["measurement"].get("reason"),
        )
        for row in _live_parity_required_rows()
    ]
    assert projected == derived
    # A family the corpus does not measure is required but cannot pass.
    pending = [f for f in _family_rows(contract) if f["state"] == "threshold_pending"]
    assert pending
    for family in pending:
        assert family["reason"] in scope["pending_reason_vocabulary"]


def test_assigned_population_binds_the_live_manifest(contract: dict) -> None:
    population = contract["assigned_population"]
    assert population["census_id"] == contract["denominator"]["initial_census"]["census_id"]
    assert population["manifest"]["path"] == "data/parse/holdout_manifests.yaml"

    articles = _live_holdout_articles()
    ids = [article["article_id"] for article in articles]
    assert len(ids) == population["declared"] == 610
    assert len(set(ids)) == len(ids)  # a duplicated identity would inflate the census
    assert all(article["status"] == population["manifest"]["present_value"] for article in articles)

    probe = json.loads(
        (ROOT / "artifacts/parse_compat/external_holdout_release_probe.json").read_text(
            encoding="utf-8"
        )
    )
    reference = probe["reference_population"]
    assert population["measurable"] == reference["measurable_documents"] == 531
    assert population["excluded"] == reference["excluded_documents"] == 79
    assert population["refused"] == 0


def test_evaluation_and_tuning_corpora_are_disjoint_on_the_live_tree(contract: dict) -> None:
    corpus = contract["independent_corpus"]
    split = contract["split_contract"]

    gold_paths = sorted((ROOT / "data/gold_standard").glob("*.gold.json"))
    gold_ids = set()
    for path in gold_paths:
        identity = path.name[: -len(".gold.json")]
        record = json.loads(path.read_text(encoding="utf-8"))
        assert record["paper_id"] == identity  # file identity is the document identity
        gold_ids.add(identity)
    assert len(gold_ids) == 200

    holdout_ids = {article["article_id"] for article in _live_holdout_articles()}
    assert gold_ids & holdout_ids == set()

    assert corpus["evaluation"]["manifest"] == split["evaluation_source"]
    assert corpus["evaluation"]["bytes_state"] == "bytes_present"
    assert corpus["tuning"]["glob"] == split["tuning_source"]
    assert corpus["disjointness"]["by"] == "document_identity"
    assert corpus["disjointness"]["prompt_hash_forbidden"] is True
    assert split["disjointness_identity"] == corpus["disjointness"]["identity_field"]
    assert corpus["disjointness"]["production_catalog_overlap"] == 0
    assert corpus["external"]["band"] == "bytes_absent"
    assert corpus["external"]["download_forbidden"] is True


def test_contract_without_the_scope_or_split_blocks_fails_closed(contract: dict) -> None:
    no_scope = copy.deepcopy(contract)
    del no_scope["required_scope"]
    assert "required_scope" in missing_contract_vocabulary(no_scope)
    violations = scorecard_violations(_baseline(contract), no_scope)
    assert {"contract_vocabulary_missing", "required_scope_missing"} <= violations

    no_split = copy.deepcopy(contract)
    del no_split["split_contract"]
    assert "split_contract" in missing_contract_vocabulary(no_split)
    violations = scorecard_violations(_baseline(contract), no_split)
    assert {"contract_vocabulary_missing", "split_contract_missing"} <= violations

    no_population = copy.deepcopy(contract)
    del no_population["assigned_population"]
    assert "assigned_population" in missing_contract_vocabulary(no_population)

    assert missing_contract_vocabulary(contract) == []


def test_scope_coverage_must_be_complete_and_inside_the_required_scope(contract: dict) -> None:
    families = _family_ids(contract)
    measured = _required_measured_ids(contract)

    clean = _baseline(contract)
    clean["scope_coverage"] = {"families": families, "measured": measured}
    assert scorecard_violations(clean, contract) == set()

    shrunk = _baseline(contract)
    shrunk["scope_coverage"] = {"families": families[:-1], "measured": measured}
    assert "required_scope_shrunk" in scorecard_violations(shrunk, contract)

    extended = _baseline(contract)
    extended["scope_coverage"] = {
        "families": [*families, "stage.invented_family"],
        "measured": measured,
    }
    assert "family_outside_required_scope" in scorecard_violations(extended, contract)

    pending_claimed = _baseline(contract)
    claimed = next(f for f in _family_rows(contract) if f["state"] == "threshold_pending")
    pending_claimed["scope_coverage"] = {
        "families": families,
        "measured": [*measured, claimed["family_id"]],
    }
    assert "family_claimed_without_required_scope" in scorecard_violations(
        pending_claimed, contract
    )

    malformed = _baseline(contract)
    malformed["scope_coverage"] = {"families": []}
    assert "required_scope_missing" in scorecard_violations(malformed, contract)

    # A scorecard that declares no coverage claims nothing and is not refused.
    assert "required_scope_missing" not in scorecard_violations(_baseline(contract), contract)


def test_split_must_be_disjoint_by_document_identity(contract: dict) -> None:
    clean = _baseline(contract)
    clean["document_split"] = {
        "identity_field": "arxiv_id",
        "evaluation_document_ids": ["1203.2295"],
        "tuning_document_ids": ["1412.6980"],
    }
    assert scorecard_violations(clean, contract) == set()

    reused = _baseline(contract)
    reused["document_split"] = {
        "identity_field": "arxiv_id",
        "evaluation_document_ids": ["1203.2295"],
        "tuning_document_ids": ["1203.2295"],
    }
    assert "eval_split_reused_for_tuning" in scorecard_violations(reused, contract)

    prompt_hash = _baseline(contract)
    prompt_hash["document_split"] = {
        "identity_field": "prompt_sha256",
        "evaluation_document_ids": ["1203.2295"],
        "tuning_document_ids": ["1412.6980"],
    }
    assert "split_contract_missing" in scorecard_violations(prompt_hash, contract)

    missing_side = _baseline(contract)
    missing_side["document_split"] = {
        "identity_field": "arxiv_id",
        "evaluation_document_ids": ["1203.2295"],
        "tuning_document_ids": [],
    }
    assert "split_contract_missing" in scorecard_violations(missing_side, contract)


def test_external_holdout_bytes_cannot_be_claimed(contract: dict) -> None:
    honest = _baseline(contract)
    honest["external_holdout"] = {"band": "bytes_absent"}
    assert scorecard_violations(honest, contract) == set()

    claimed_band = _baseline(contract)
    claimed_band["external_holdout"] = {"band": "bytes_present"}
    assert "external_bytes_claimed" in scorecard_violations(claimed_band, contract)

    claimed_bytes = _baseline(contract)
    claimed_bytes["external_holdout"] = {"band": "bytes_absent", "bytes_present": True}
    assert "external_bytes_claimed" in scorecard_violations(claimed_bytes, contract)

    claimed_docs = _baseline(contract)
    claimed_docs["external_holdout"] = {
        "band": "bytes_absent",
        "document_ids": ["9999.00001"],
    }
    assert "external_bytes_claimed" in scorecard_violations(claimed_docs, contract)
