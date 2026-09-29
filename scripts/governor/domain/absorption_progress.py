"""Absorption progress domain validator — the product-progress scorecard contract.

Pure value logic: no filesystem, network, clock or RNG access, matching
``governor.domain``'s contract. The closed vocabulary lives in DATA
(``data/parse/absorption_progress.schema.yaml``); this module only decides
whether a scorecard speaks that vocabulary honestly, so a DATA edit changes the
rules without changing code.

Why the shape matters (M513 closeout): the recorded progress surface mixed
preservation with extraction, published ratios over shrunken denominators,
inferred independence from display labels, and treated terminal GSD state as
delivered capability. Each of those is now a typed, fail-closed violation rather
than a convention:

* a denominator that does not sum (``assigned != executed + refused + excluded``)
  is ``assigned_ne_total``; exclusions without a closed typed reason are
  ``unnamed_exclusions`` / ``exclusion_reason_untyped``;
* a pinned census that has been re-cut is ``census_mismatch`` — the 79 typed
  exclusions stay in the denominator;
* a value under a measured state with no evidence, no comparability identity or
  a zero denominator is refused (``evidence_binding_missing``,
  ``comparability_fields_missing``, ``no_denominator_treated_as_number``);
* ``not_measured`` may not carry a value (``value_with_not_measured``), an empty
  population may not be given a number (``empty_population_given_value``), and a
  measurement on nothing is not ``measured``;
* two measurements may only be compared inside one series; a differing corpus,
  profile, oracle or source revision is ``mixed_scope_comparison``;
* a micro score with no weakest stratum, or a weakest stratum that is not the
  weakest, is refused (``weakest_stratum_absent``, ``stratum_micro_only``,
  ``weakest_stratum_not_weakest``);
* a numeric threshold before S02 freezes one is
  ``threshold_invented_before_freeze``;
* preservation pooled into extraction is
  ``preservation_pooled_with_extraction``;
* a lifecycle state offered as executed product evidence is
  ``skipped_as_delivered``;
* an authorization state offered as product coverage is
  ``authorization_used_as_product_evidence``;
* rewriting a historical M513 carrier is ``historical_carrier_rewritten``.

Boundary (stated, not implied — M509/S02 Q3 R2): this is a consistency and
fail-closed contract over what a scorecard declares about itself and its
evidence identity. It is **not** a tamper-evident attestation: an actor able to
rewrite the scorecard, the evidence artifact and this DATA rule can satisfy it.
Its value is that absence, drift, mixed scope and self-contradiction stop passing
as compliance, and that a coverage number must name the evidence that produced
it.

Violations are plain labels (never exception tuples), so the T04 Governor check
can format them into evidence without parsing prose. The label vocabulary is the
list embedded in this docstring; ``scorecard_violations`` returns a subset of it.
"""

from __future__ import annotations

import re
from typing import Any, Optional

__all__ = [
    "comparability_key",
    "denominator_violations",
    "measurement_violations",
    "missing_contract_vocabulary",
    "scorecard_violations",
]

#: The honest measurement states. A state outside this set fails closed.
MEASUREMENT_STATES = ("measured", "not_measured", "no_denominator", "not_comparable")

#: The full-denominator partition. Excluded and refused stay inside it.
DENOMINATOR_COMPONENTS = ("assigned", "executed", "refused", "excluded")

#: Fallback comparability identity when the contract omits ``comparability.keys``.
DEFAULT_COMPARABILITY_KEYS = ("corpus_id", "profile", "oracle", "source_revision")

#: A bound byte hash and a bound revision, as declared by the evidence contract.
_SHA256_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
_REVISION_RE = re.compile(r"^(git:[0-9a-f]{40}|sha256:[0-9a-f]{64})$")

#: Words that mean "the harness finished", never "the product delivered".
_LIFECYCLE_WORDS = frozenset(
    {"complete", "completed", "skipped", "waiver", "owner_accepted", "cancelled", "canceled"}
)

#: Evidence kinds that may never stand in for an executed product measurement.
_FORBIDDEN_EVIDENCE_KINDS = frozenset({"lifecycle_state", "lifecycle", "authorization"})

_REQUIRED_AXES = frozenset(
    {
        "java_independence",
        "extraction",
        "preservation",
        "reliability",
        "resources",
        "authorization",
    }
)


def _count(value: Any) -> Optional[int]:
    """A non-negative integer count, or None when the value is not one."""
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value if value >= 0 else None
    if isinstance(value, float) and value.is_integer():
        return int(value)
    return None


def _text(value: Any) -> Optional[str]:
    """A non-empty string, or None."""
    if isinstance(value, str) and value.strip():
        return value
    return None


def _text_list(value: Any) -> list[str]:
    """The non-empty strings inside a list-like value (possibly empty)."""
    if not isinstance(value, (list, tuple, set)):
        return []
    return [item for item in value if isinstance(item, str) and item.strip()]


def _is_relative_path(value: Any) -> bool:
    """A repository-relative path: non-empty, not absolute, no parent escape."""
    text = _text(value)
    if text is None:
        return False
    if text.startswith("/") or text.startswith("\\"):
        return False
    if ".." in text.split("/"):
        return False
    if re.match(r"^[A-Za-z]:", text):  # a Windows drive letter is absolute
        return False
    return True


def missing_contract_vocabulary(contract: Any) -> list[str]:
    """Names of required contract keys that are absent, empty or malformed.

    A contract that has lost a closed vocabulary cannot distinguish an honest
    scorecard from a dishonest one, so the caller must fail closed on a non-empty
    result instead of validating against a partial contract. The returned names
    are dotted key paths, so a DATA regression is reported precisely.
    """
    if not isinstance(contract, dict):
        return ["contract"]

    missing: list[str] = []
    for key in ("schema_version", "contract_id", "adr_ref", "boundary"):
        if _text(contract.get(key)) is None:
            missing.append(key)

    axes = contract.get("product_axes")
    if not isinstance(axes, dict):
        missing.append("product_axes")
    else:
        if axes.get("closed") is not True:
            missing.append("product_axes.closed")
        ids = set(_text_list(axes.get("ids")))
        if not _REQUIRED_AXES <= ids:
            missing.append("product_axes.ids")
        if axes.get("authorization_not_product") is not True:
            missing.append("product_axes.authorization_not_product")

    states = contract.get("measurement_states")
    if not isinstance(states, dict):
        missing.append("measurement_states")
    else:
        if states.get("closed") is not True:
            missing.append("measurement_states.closed")
        if not set(MEASUREMENT_STATES) <= set(_text_list(states.get("values"))):
            missing.append("measurement_states.values")

    denominator = contract.get("denominator")
    if not isinstance(denominator, dict):
        missing.append("denominator")
    else:
        if denominator.get("partition_closed") is not True:
            missing.append("denominator.partition_closed")
        if list(_text_list(denominator.get("components"))) != list(DENOMINATOR_COMPONENTS):
            missing.append("denominator.components")
        if _text(denominator.get("identity")) is None:
            missing.append("denominator.identity")
        reasons = denominator.get("exclusion_reasons")
        if not isinstance(reasons, dict) or reasons.get("closed") is not True or not _text_list(
            reasons.get("values")
        ):
            missing.append("denominator.exclusion_reasons")
        census = denominator.get("initial_census")
        if not isinstance(census, dict):
            missing.append("denominator.initial_census")
        else:
            if _text(census.get("census_id")) is None:
                missing.append("denominator.initial_census.census_id")
            if _text(census.get("authority")) is None:
                missing.append("denominator.initial_census.authority")
            for key in ("declared", "measurable", "excluded"):
                if _count(census.get(key)) is None:
                    missing.append(f"denominator.initial_census.{key}")
            if not isinstance(census.get("reason_counts"), dict) or not census.get(
                "reason_counts"
            ):
                missing.append("denominator.initial_census.reason_counts")

    comparability = contract.get("comparability")
    if not isinstance(comparability, dict) or not _text_list(comparability.get("keys")):
        missing.append("comparability")
    elif comparability.get("same_series_required") is not True:
        missing.append("comparability.same_series_required")

    strata = contract.get("quality_strata")
    if not isinstance(strata, dict):
        missing.append("quality_strata")
    else:
        if strata.get("closed") is not True:
            missing.append("quality_strata.closed")
        if not _text_list(strata.get("values")):
            missing.append("quality_strata.values")
        if strata.get("weakest_required") is not True:
            missing.append("quality_strata.weakest_required")
        if strata.get("micro_alone_forbidden") is not True:
            missing.append("quality_strata.micro_alone_forbidden")
        if strata.get("direction") not in {"higher_is_better", "lower_is_better"}:
            missing.append("quality_strata.direction")

    thresholds = contract.get("threshold_policy")
    if not isinstance(thresholds, dict):
        missing.append("threshold_policy")
    else:
        if thresholds.get("state") not in {"not_frozen", "frozen"}:
            missing.append("threshold_policy.state")
        if _text(thresholds.get("freeze_owner")) is None:
            missing.append("threshold_policy.freeze_owner")
        if thresholds.get("numeric_before_freeze") != "forbidden":
            missing.append("threshold_policy.numeric_before_freeze")

    authorization = contract.get("authorization")
    if not isinstance(authorization, dict):
        missing.append("authorization")
    else:
        if authorization.get("separate_from_product") is not True:
            missing.append("authorization.separate_from_product")
        if not _text_list(authorization.get("states")):
            missing.append("authorization.states")
        if _text(authorization.get("product_coverage_effect")) is None:
            missing.append("authorization.product_coverage_effect")

    lifecycle = contract.get("lifecycle")
    if not isinstance(lifecycle, dict) or lifecycle.get("product_evidence_forbidden") is not True:
        missing.append("lifecycle.product_evidence_forbidden")
    if not isinstance(lifecycle, dict) or not _text_list(lifecycle.get("states")):
        missing.append("lifecycle.states")

    preservation = contract.get("preservation")
    if not isinstance(preservation, dict):
        missing.append("preservation")
    elif (
        preservation.get("separate_from_extraction") is not True
        or preservation.get("pooling_forbidden") is not True
    ):
        missing.append("preservation.separate_from_extraction")

    binding = contract.get("evidence_binding")
    if not isinstance(binding, dict):
        missing.append("evidence_binding")
    else:
        if binding.get("path_policy") != "repository_relative":
            missing.append("evidence_binding.path_policy")
        if not _text_list(binding.get("required_fields")):
            missing.append("evidence_binding.required_fields")
        if _text(binding.get("stale_evidence_behavior")) is None:
            missing.append("evidence_binding.stale_evidence_behavior")

    carriers = contract.get("historical_carriers")
    if not isinstance(carriers, dict):
        missing.append("historical_carriers")
    else:
        if carriers.get("mode") != "read_only":
            missing.append("historical_carriers.mode")
        if carriers.get("rewriting_forbidden") is not True:
            missing.append("historical_carriers.rewriting_forbidden")
        entries = carriers.get("carriers")
        if not isinstance(entries, list) or not entries:
            missing.append("historical_carriers.carriers")
        else:
            for index, entry in enumerate(entries):
                if not isinstance(entry, dict) or _text(entry.get("id")) is None or _text(
                    entry.get("kind")
                ) is None:
                    missing.append(f"historical_carriers.carriers[{index}]")

    rules = contract.get("existing_rules")
    if not isinstance(rules, list) or not rules:
        missing.append("existing_rules")
    else:
        for index, rule in enumerate(rules):
            if not isinstance(rule, dict):
                missing.append(f"existing_rules[{index}]")
                continue
            for key in ("id", "kind", "path", "covers", "reuse"):
                if _text(rule.get(key)) is None:
                    missing.append(f"existing_rules[{index}].{key}")
            if _text(rule.get("path")) is not None and not _is_relative_path(rule.get("path")):
                missing.append(f"existing_rules[{index}].path")

    return missing


def _contract_axes(contract: dict) -> set[str]:
    return set(_text_list((contract.get("product_axes") or {}).get("ids")))


def _contract_states(contract: dict) -> set[str]:
    return set(_text_list((contract.get("measurement_states") or {}).get("values")))


def _contract_comparability_keys(contract: dict) -> tuple[str, ...]:
    keys = _text_list((contract.get("comparability") or {}).get("keys"))
    return tuple(keys) if keys else DEFAULT_COMPARABILITY_KEYS


def _exclusion_vocabulary(contract: dict) -> set[str]:
    reasons = (contract.get("denominator") or {}).get("exclusion_reasons") or {}
    return set(_text_list(reasons.get("values")))


def _historical_carrier_ids(contract: dict) -> set[str]:
    carriers = (contract.get("historical_carriers") or {}).get("carriers") or []
    ids: set[str] = set()
    for entry in carriers:
        if isinstance(entry, dict) and _text(entry.get("id")):
            ids.add(str(entry["id"]))
    return ids


def _score_number(value: Any) -> Optional[float]:
    """A numeric score from either a scalar or a ``{numerator, denominator}`` map.

    A fraction is read as a number only when its denominator is positive; a zero
    denominator is "nothing to divide", not a value, so the caller can refuse it.
    """
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return float(value)
    if isinstance(value, dict):
        numerator = value.get("numerator")
        denominator = value.get("denominator")
        if isinstance(numerator, bool) or isinstance(denominator, bool):
            return None
        if isinstance(numerator, (int, float)) and isinstance(denominator, (int, float)):
            if denominator <= 0:
                return None
            return float(numerator) / float(denominator)
    return None


def _fraction_denominator(value: Any) -> Optional[int]:
    if isinstance(value, dict):
        denominator = value.get("denominator")
        if isinstance(denominator, bool):
            return None
        if isinstance(denominator, int):
            return denominator
        if isinstance(denominator, float) and denominator.is_integer():
            return int(denominator)
    return None


def _has_score(record: dict) -> bool:
    return record.get("score") is not None


def comparability_key(record: Any, contract: Any) -> Optional[tuple]:
    """The series identity of a measurement, or None when it is incomplete.

    Two measurements may only be compared when their keys are equal. None means
    the record does not declare a full identity, which the caller reports as
    ``comparability_fields_missing`` rather than silently treating it as
    comparable to anything.
    """
    if not isinstance(record, dict):
        return None
    block = record.get("comparability")
    if not isinstance(block, dict):
        return None
    values: list[str] = []
    for key in _contract_comparability_keys(contract if isinstance(contract, dict) else {}):
        text = _text(block.get(key))
        if text is None:
            return None
        values.append(text)
    return tuple(values)


def denominator_violations(denominator: Any, contract: Any) -> set[str]:
    """Violations of the full-denominator partition and its typed exclusions."""
    if not isinstance(denominator, dict):
        return {"denominator_missing"}
    violations: set[str] = set()
    values: dict[str, int] = {}
    for component in DENOMINATOR_COMPONENTS:
        count = _count(denominator.get(component))
        if count is None:
            violations.add("denominator_component_invalid")
        else:
            values[component] = count
    if len(values) == len(DENOMINATOR_COMPONENTS):
        total = values["executed"] + values["refused"] + values["excluded"]
        if values["assigned"] != total:
            violations.add("assigned_ne_total")

    vocabulary = _exclusion_vocabulary(contract if isinstance(contract, dict) else {})
    reasons = denominator.get("exclusion_reasons")
    if not isinstance(reasons, dict):
        violations.add("unnamed_exclusions")
        return violations
    total_reasons = 0
    for label, count in reasons.items():
        if not isinstance(label, str) or label not in vocabulary:
            violations.add("exclusion_reason_untyped")
        reason_count = _count(count)
        if reason_count is None:
            violations.add("unnamed_exclusions")
        else:
            total_reasons += reason_count
    excluded = values.get("excluded")
    if excluded is None or total_reasons != excluded:
        violations.add("unnamed_exclusions")
    return violations


def _thresholds_frozen(contract: dict) -> bool:
    return (contract.get("threshold_policy") or {}).get("state") == "frozen"


def measurement_violations(
    record: Any, contract: dict, population: Optional[int] = None
) -> set[str]:
    """Violations of one measurement row against the closed scorecard vocabulary.

    ``population`` is the scorecard's assigned population when known. A zero
    population turns any value or a measured state into
    ``empty_population_given_value``: there is nothing to divide, so a number
    here reads as invented.
    """
    if not isinstance(record, dict):
        return {"measurement_not_an_object"}

    violations: set[str] = set()
    if _text(record.get("metric_id")) is None:
        violations.add("metric_id_missing")

    axis = record.get("axis")
    if axis not in _contract_axes(contract):
        violations.add("unknown_axis")

    state = record.get("state")
    if state not in _contract_states(contract):
        violations.add("unknown_measurement_state")

    if state == "measured":
        if comparability_key(record, contract) is None:
            violations.add("comparability_fields_missing")
        if not _has_score(record):
            violations.add("measured_without_score")
    if state != "measured" and _has_score(record):
        violations.add(f"value_with_{state}")
    if state == "measured" and population == 0:
        violations.add("empty_population_given_value")

    score_denominator = _fraction_denominator(record.get("score"))
    if score_denominator == 0:
        violations.add("no_denominator_treated_as_number")

    if _has_score(record):
        strata = record.get("strata")
        if not isinstance(strata, dict) or "weakest_stratum" not in strata:
            violations.add("weakest_stratum_absent")
        else:
            # ``weakest_stratum`` is the bookkeeping key naming the weakest
            # value, not a stratum of its own: excluding it is what makes a
            # micro-only report detectable.
            names = {key for key in strata if isinstance(key, str) and key != "weakest_stratum"}
            if not names or names <= {"micro_occurrence"}:
                violations.add("stratum_micro_only")
            present = {_score_number(value) for value in strata.values()}
            present.discard(None)
            weakest = _score_number(strata.get("weakest_stratum"))
            if weakest is not None and len(present) >= 2:
                direction = (contract.get("quality_strata") or {}).get("direction")
                expected = min(present) if direction == "higher_is_better" else max(present)
                if weakest != expected:
                    violations.add("weakest_stratum_not_weakest")

    # Evidence identity is required only where a value is claimed: a
    # ``not_measured`` axis has nothing to bind, and demanding a receipt there
    # would force a fabricated one. If evidence IS supplied, its shape is
    # validated regardless of state.
    evidence = record.get("evidence")
    binding = contract.get("evidence_binding") or {}
    if not isinstance(evidence, dict):
        if state == "measured":
            violations.add("evidence_binding_missing")
    else:
        for field in _text_list(binding.get("required_fields")):
            if _text(evidence.get(field)) is None:
                violations.add("evidence_binding_missing")
                break
        sha = evidence.get("sha256")
        if _text(sha) is not None and not _SHA256_RE.match(str(sha)):
            violations.add("evidence_binding_malformed")
        for field in ("source_revision", "tested_revision"):
            revision = evidence.get(field)
            if _text(revision) is not None and not _REVISION_RE.match(str(revision)):
                violations.add("evidence_binding_malformed")
        compare = record.get("comparability")
        if isinstance(compare, dict):
            declared = _text(compare.get("source_revision"))
            bound = _text(evidence.get("source_revision"))
            if declared is not None and bound is not None and declared != bound:
                violations.add("evidence_revision_mismatch")

    evidence_kind = _text(evidence.get("kind")) if isinstance(evidence, dict) else None
    if evidence_kind in _FORBIDDEN_EVIDENCE_KINDS:
        if evidence_kind == "authorization":
            violations.add("authorization_used_as_product_evidence")
        else:
            violations.add("skipped_as_delivered")
    delivered_by = record.get("delivered_by")
    if isinstance(delivered_by, str) and delivered_by in _LIFECYCLE_WORDS:
        violations.add("skipped_as_delivered")
    if axis == "extraction" and record.get("includes_preservation") is True:
        violations.add("preservation_pooled_with_extraction")
    # Pooling is the join of the two axes, declared from either side: a
    # preservation row pooled with extraction is the same defect as an
    # extraction row carrying preservation inside it.
    pooled = set(_text_list(record.get("pooled_with")))
    if axis in {"extraction", "preservation"}:
        other = "extraction" if axis == "preservation" else "preservation"
        if other in pooled:
            violations.add("preservation_pooled_with_extraction")
    if {"extraction", "preservation"} <= pooled:
        violations.add("preservation_pooled_with_extraction")

    if ("threshold" in record or "margin" in record) and not _thresholds_frozen(contract):
        violations.add("threshold_invented_before_freeze")

    return violations


def _authorization_violations(authorization: Any, contract: dict) -> set[str]:
    if not isinstance(authorization, dict):
        return {"authorization_block_missing"}
    violations: set[str] = set()
    states = set(_text_list((contract.get("authorization") or {}).get("states")))
    state = _text(authorization.get("state"))
    if state is None or state not in states:
        violations.add("unknown_authorization_state")
    effect = _text(authorization.get("product_coverage_effect"))
    if effect is None:
        violations.add("authorization_used_as_product_evidence")
    elif state != "human_release_recorded" and effect != "none":
        violations.add("authorization_used_as_product_evidence")
    if state == "human_release_recorded" and _text(authorization.get("release_ref")) is None:
        violations.add("authorization_release_unbound")
    return violations


def _census_violations(scorecard: dict, contract: dict) -> set[str]:
    census_id = scorecard.get("census_id")
    if census_id is None:
        return set()
    census = (contract.get("denominator") or {}).get("initial_census") or {}
    pinned = _text(census.get("census_id"))
    if _text(census_id) is None or pinned is None or str(census_id) != pinned:
        return {"census_identity_unknown"}
    violations: set[str] = set()
    denominator = scorecard.get("denominator")
    if not isinstance(denominator, dict):
        return set()
    pairs = (
        ("assigned", "declared"),
        ("executed", "measurable"),
        ("excluded", "excluded"),
    )
    for observed_key, pinned_key in pairs:
        observed = _count(denominator.get(observed_key))
        expected = _count(census.get(pinned_key))
        if observed is not None and expected is not None and observed != expected:
            violations.add("census_mismatch")
    observed_reasons = denominator.get("exclusion_reasons")
    expected_reasons = census.get("reason_counts")
    if isinstance(observed_reasons, dict) and isinstance(expected_reasons, dict):
        for label, expected in expected_reasons.items():
            observed = _count(observed_reasons.get(label))
            expected_count = _count(expected)
            if observed is not None and expected_count is not None and observed != expected_count:
                violations.add("census_mismatch")
    return violations


def _lifecycle_denominator_violations(denominator: Any) -> set[str]:
    if not isinstance(denominator, dict):
        return set()
    kinds = set(_text_list(denominator.get("executed_evidence_kinds")))
    if kinds & _FORBIDDEN_EVIDENCE_KINDS or kinds & _LIFECYCLE_WORDS:
        return {"skipped_as_delivered"}
    return set()


def _lineage_violations(scorecard: dict, contract: dict) -> set[str]:
    lineage = scorecard.get("lineage")
    if not isinstance(lineage, dict):
        return {"lineage_invalid"}
    carriers = _historical_carrier_ids(contract)
    if not carriers:
        return set()
    referenced: set[str] = set()
    for value in lineage.values():
        if isinstance(value, str):
            referenced.add(value)
        elif isinstance(value, (list, tuple, set)):
            referenced.update(item for item in value if isinstance(item, str))
    if referenced & carriers:
        return {"historical_carrier_rewritten"}
    return set()


def _comparison_violations(scorecard: dict, contract: dict) -> set[str]:
    measurements = scorecard.get("measurements")
    if not isinstance(measurements, list):
        return set()
    by_id: dict[str, dict] = {}
    for record in measurements:
        if isinstance(record, dict):
            metric_id = _text(record.get("metric_id"))
            if metric_id is not None:
                by_id[metric_id] = record
    violations: set[str] = set()
    for record in measurements:
        if not isinstance(record, dict):
            continue
        target_id = _text(record.get("compared_to"))
        if target_id is None:
            continue
        target = by_id.get(target_id)
        if target is None:
            violations.add("comparison_target_missing")
            continue
        left = comparability_key(record, contract)
        right = comparability_key(target, contract)
        if left is None or right is None or left != right:
            violations.add("mixed_scope_comparison")
    return violations


def _threshold_freeze_violations(scorecard: dict, contract: dict) -> set[str]:
    block = scorecard.get("threshold_freeze")
    if block is None:
        return set()
    if not isinstance(block, dict) or block.get("state") != "frozen":
        return {"threshold_freeze_unbound"}
    if not _thresholds_frozen(contract):
        return {"threshold_invented_before_freeze"}
    if any(_text(block.get(key)) is None for key in ("revision", "frozen_at", "source")):
        return {"threshold_freeze_unbound"}
    return set()


def scorecard_violations(scorecard: Any, contract: Any) -> set[str]:
    """Every violation of one scorecard against the closed progress contract.

    An empty set means the scorecard speaks the vocabulary honestly. The
    function is pure and total: a missing or malformed contract fails closed
    with ``contract_vocabulary_missing`` rather than accepting everything.
    """
    if missing_contract_vocabulary(contract):
        return {"contract_vocabulary_missing"}
    contract = dict(contract)
    if not isinstance(scorecard, dict):
        return {"scorecard_not_an_object"}

    violations: set[str] = set()
    if scorecard.get("schema_version") != contract.get("schema_version"):
        violations.add("unsupported_scorecard_schema")
    if _text(scorecard.get("scorecard_id")) is None:
        violations.add("scorecard_id_missing")

    denominator = scorecard.get("denominator")
    if not isinstance(denominator, dict):
        violations.add("denominator_missing")
    else:
        violations |= denominator_violations(denominator, contract)
        violations |= _lifecycle_denominator_violations(denominator)
    violations |= _census_violations(scorecard, contract)

    measurements = scorecard.get("measurements")
    population = _count((denominator or {}).get("assigned")) if isinstance(denominator, dict) else None
    if not isinstance(measurements, list):
        violations.add("measurements_missing")
    else:
        for record in measurements:
            violations |= measurement_violations(record, contract, population)

    violations |= _authorization_violations(scorecard.get("authorization"), contract)
    violations |= _lineage_violations(scorecard, contract)
    violations |= _comparison_violations(scorecard, contract)
    violations |= _threshold_freeze_violations(scorecard, contract)
    return violations
