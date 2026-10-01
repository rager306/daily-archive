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
  ``threshold_invented_before_freeze``; once the contract is frozen a required
  family with no threshold, or a measured row publishing a threshold the frozen
  contract does not declare, is ``threshold_missing_for_required_metric`` (a
  threshold the corpus cannot measure is never created, D496);
* a frozen threshold that resolves against the candidate scorecard instead of a
  separately pinned reference is ``threshold_reference_not_independently_pinned``:
  a candidate compared to its own restated score would move its own floor;
* a frozen threshold that resolves against the candidate scorecard instead of a
  separately pinned reference is ``threshold_reference_not_independently_pinned``:
  a candidate compared to its own restated score would move its own floor;
* a coverage report that shrinks, extends or silently claims the required
  product scope is ``required_scope_shrunk`` / ``family_outside_required_scope``
  / ``family_claimed_without_required_scope`` (and ``required_scope_missing``
  when the contract has lost the scope block entirely);
* a split that reuses the evaluation corpus for tuning, or that declares its
  disjointness by prompt hash instead of document identity, is
  ``eval_split_reused_for_tuning`` / ``split_contract_missing``, and external
  holdout bytes offered as local evidence are ``external_bytes_claimed``;
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
    "JAVA_FREE_SLICE_VIOLATION_LABELS",
    "NATIVE_CONTENT_VIOLATION_LABELS",
    "NATIVE_FAMILY_VIOLATION_LABELS",
    "NATIVE_ROUTE_VIOLATION_LABELS",
    "SCOPE_VIOLATION_LABELS",
    "THRESHOLD_DIRECTIONS",
    "comparability_key",
    "contract_thresholds",
    "denominator_violations",
    "measurement_violations",
    "missing_contract_vocabulary",
    "score_number",
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

#: The two states a required scope family may carry. ``threshold_pending`` is a
#: required family the frozen corpus does not measure (D496): it must still be
#: reported, but it can never be marked measured and can never pass.
_SCOPE_FAMILY_STATES = ("required", "threshold_pending")

#: The directions a frozen threshold may carry. Every frozen threshold is an
#: absolute noninferiority floor against the declared reference, so ``at_least``
#: is the expected value; ``at_most`` exists for a bounded-interval metric.
THRESHOLD_DIRECTIONS = ("at_least", "at_most")

#: The closed labels the scope/corpus/split half contributes. Pinned as a tuple
#: so the vocabulary is inspectable without parsing the docstring.
SCOPE_VIOLATION_LABELS = (
    "required_scope_missing",
    "required_scope_shrunk",
    "family_outside_required_scope",
    "family_claimed_without_required_scope",
    "split_contract_missing",
    "eval_split_reused_for_tuning",
    "external_bytes_claimed",
)

#: The closed adapter-level vocabulary of the Java-free vertical slice half
#: (M514/S03/T06). Literals on purpose (D492): the closed vocabulary is owned
#: here, and the rule DATA restates it so a label the oracle can emit but that is
#: not registered — or a registered label the oracle cannot emit — is a
#: data/implementation mismatch rather than a silent finding. The adapter is the
#: only layer that reads the receipt; it emits exactly these labels and nothing
#: else, and none of them moves the product `java_independence` axis (D499).
JAVA_FREE_SLICE_VIOLATION_LABELS = frozenset(
    {
        "java_free_slice_claim_without_receipt",
        "java_free_slice_receipt_slice_mismatch",
        "java_free_slice_unmeasured_stage",
        "java_free_slice_guard_not_installed",
        "java_free_slice_product_coverage_claimed",
        "java_free_slice_stage_outside_slice_counted",
        "java_free_slice_receipt_stale",
        "java_free_slice_java_calls_nonzero",
    }
)

#: The closed adapter-level vocabulary of the native family receipts half
#: (M514/S04/T06). Literals on purpose (D492): the closed vocabulary is owned
#: here, and the rule DATA restates it so a label the oracle can emit but that is
#: not registered — or a registered label the oracle cannot emit — is a
#: data/implementation mismatch rather than a silent finding. The adapter is the
#: only layer that reads the receipts; it emits exactly these labels and nothing
#: else, and none of them moves the product `java_independence` axis (D499).
#:
#: Why this half exists (R141): the S04 native family receipts are the first
#: source-bound evidence that the real native parsers — not the Java carriers —
#: produced header and bibliography atoms on the declared 3-document slice, and
#: that the 79 typed baseline exclusions keep their denominator. A receipt's mere
#: existence must not read as product success, so every claim shape has a typed
#: refusal: a claim with no receipt, a receipt whose population does not match the
#: frozen carriers, a receipt bound to older bytes, a diagnostic slice re-labelled
#: as product coverage, an observed Java call on a Java-free plane, an observed
#: zero with no guard provenance, an oracle that is not independent of the thing
#: it scores, a claimed threshold the freeze does not declare, and a shrunken
#: exclusion denominator.
NATIVE_FAMILY_VIOLATION_LABELS = frozenset(
    {
        "native_family_claim_without_receipt",
        "native_family_receipt_population_mismatch",
        "native_family_receipt_stale",
        "native_family_product_coverage_claimed",
        "native_family_java_calls_nonzero",
        "native_family_guard_not_installed",
        "native_family_oracle_not_independent",
        "native_family_threshold_claimed",
        "native_family_exclusion_denominator_dropped",
    }
)

#: The closed adapter-level vocabulary of the S05 content receipt half
#: (M514/S05/T07). Literals on purpose (D492): the closed vocabulary is owned
#: here, and the rule DATA restates it so a label the oracle can emit but that is
#: not registered — or a registered label the oracle cannot emit — is a
#: data/implementation mismatch rather than a silent finding. The adapter is the
#: only layer that reads the receipt; it emits exactly these labels and nothing
#: else, and none of them moves the product `java_independence` axis (D499).
#:
#: Why this half exists (R141): the S05 receipt is the first source-bound evidence
#: that the native body/section/reading-order/callout/visual bands measured the
#: declared 3-document slice against an independent oracle, with a total group
#: partition and a typed per-family matrix. A receipt's mere existence must not
#: read as product success, so every claim shape has a typed refusal: a claim with
#: no receipt, a receipt whose population or group partition the frozen carriers
#: do not derive, a receipt bound to older bytes, a diagnostic slice re-labelled as
#: product coverage (including a claimed threshold or a leaked redaction flag), an
#: observed Java call on an otherwise Java-free plane, an observed zero with no
#: guard provenance, an oracle that is not independent, an oracle that carries
#: only absences, a required family silently moved to `out_of_scope`, an untyped
#: group partition, and a body gap whose D498 obligation is unresolved.
NATIVE_CONTENT_VIOLATION_LABELS = frozenset(
    {
        "native_content_claim_without_receipt",
        "native_content_receipt_population_mismatch",
        "native_content_receipt_stale",
        "native_content_product_coverage_claimed",
        "native_content_java_calls_nonzero",
        "native_content_guard_not_installed",
        "native_content_oracle_not_independent",
        "native_content_oracle_absence_only",
        "native_content_required_family_out_of_scope",
        "native_content_group_partition_untyped",
        "native_content_body_gap_unresolved",
    }
)

#: The closed adapter-level vocabulary of the S06 route receipt-claim half
#: (M514/S06/T07). Literals on purpose (D492): the closed vocabulary is owned
#: here, and the rule DATA restates it so a label the oracle can emit but that is
#: not registered — or a registered label the oracle cannot emit — is a
#: data/implementation mismatch rather than a silent finding. The adapter is the
#: only layer that reads the two S06 receipts; it emits exactly these labels and
#: nothing else, and none of them moves the product `java_independence` axis
#: (D499).
#:
#: Why this half exists (R141): the S06 route is the first PDF->native path that
#: ends in one typed document result on the declared 3-document slice with no Java
#: and no prior TEI, and the S06/T06 same-n receipt pairs that route's resource
#: measurement with a pinned reference on the same documents. A receipt's mere
#: existence must not read as product success, so every claim shape has a typed
#: refusal: a claim with no receipt, a receipt whose population or document set
#: the frozen carriers do not derive, a receipt bound to older bytes or to a
#: freeze revision that no longer recomputes, a diagnostic slice re-labelled as
#: product coverage (including a claimed threshold or a leaked redaction flag), an
#: observed Java call on an otherwise Java-free plane, an observed zero with no
#: guard provenance, an unfrozen or post-scoring limit dictionary, a stage counted
#: where the boundary declares it out of slice, a required family moved to the
#: forbidden `out_of_scope` token, an untyped body/P1 residual, a same-n reference
#: row with no pinned measured source, and a metric row that cannot be read as a
#: typed observation.
NATIVE_ROUTE_VIOLATION_LABELS = frozenset(
    {
        "native_route_claim_without_receipt",
        "native_route_receipt_population_mismatch",
        "native_route_receipt_stale",
        "native_route_product_coverage_claimed",
        "native_route_java_calls_nonzero",
        "native_route_guard_not_installed",
        "native_route_limit_not_frozen",
        "native_route_limit_tuned_after_scoring",
        "native_route_stage_outside_slice_counted",
        "native_route_family_out_of_scope",
        "native_route_body_gap_untyped",
        "native_route_resource_pair_unbound",
        "native_route_metric_untyped",
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


def _required_scope_vocabulary(block: Any) -> list[str]:
    """Names of ``required_scope`` fields that are absent or malformed."""
    if not isinstance(block, dict):
        return ["required_scope"]
    missing: list[str] = []
    if block.get("closed") is not True:
        missing.append("required_scope.closed")
    source = block.get("source")
    if (
        not isinstance(source, dict)
        or _text(source.get("path")) is None
        or _text(source.get("pointer")) is None
    ):
        missing.append("required_scope.source")
    elif not _is_relative_path(source.get("path")):
        missing.append("required_scope.source.path")
    states = set(_text_list(block.get("family_state_vocabulary")))
    if not set(_SCOPE_FAMILY_STATES) <= states:
        missing.append("required_scope.family_state_vocabulary")
    families = block.get("families")
    if not isinstance(families, list) or not families:
        missing.append("required_scope.families")
    else:
        reasons = set(_text_list(block.get("pending_reason_vocabulary")))
        for index, family in enumerate(families):
            if not isinstance(family, dict) or _text(family.get("family_id")) is None:
                missing.append(f"required_scope.families[{index}].family_id")
                continue
            state = _text(family.get("state"))
            if state not in states:
                missing.append(f"required_scope.families[{index}].state")
            elif state == "threshold_pending":
                reason = _text(family.get("reason"))
                if reason is None or (reasons and reason not in reasons):
                    missing.append(f"required_scope.families[{index}].reason")
    return missing


def _assigned_population_vocabulary(block: Any) -> list[str]:
    """Names of ``assigned_population`` fields that are absent or malformed."""
    if not isinstance(block, dict):
        return ["assigned_population"]
    missing: list[str] = []
    if block.get("closed") is not True:
        missing.append("assigned_population.closed")
    for key in ("census_id", "identity"):
        if _text(block.get(key)) is None:
            missing.append(f"assigned_population.{key}")
    manifest = block.get("manifest")
    if (
        not isinstance(manifest, dict)
        or _text(manifest.get("path")) is None
        or _text(manifest.get("pointer")) is None
        or _text(manifest.get("identity_field")) is None
    ):
        missing.append("assigned_population.manifest")
    elif not _is_relative_path(manifest.get("path")):
        missing.append("assigned_population.manifest.path")
    for key in ("declared", "measurable", "excluded", "refused"):
        if _count(block.get(key)) is None:
            missing.append(f"assigned_population.{key}")
    if block.get("bytes_present_required") is not True:
        missing.append("assigned_population.bytes_present_required")
    return missing


def _independent_corpus_vocabulary(block: Any) -> list[str]:
    """Names of ``independent_corpus`` fields that are absent or malformed."""
    if not isinstance(block, dict):
        return ["independent_corpus"]
    missing: list[str] = []
    if block.get("closed") is not True:
        missing.append("independent_corpus.closed")
    evaluation = block.get("evaluation")
    if (
        not isinstance(evaluation, dict)
        or _text(evaluation.get("corpus_id")) is None
        or _text(evaluation.get("bytes_state")) is None
    ):
        missing.append("independent_corpus.evaluation")
    tuning = block.get("tuning")
    if (
        not isinstance(tuning, dict)
        or _text(tuning.get("corpus_id")) is None
        or _text(tuning.get("glob")) is None
    ):
        missing.append("independent_corpus.tuning")
    disjoint = block.get("disjointness")
    if not isinstance(disjoint, dict) or _text(disjoint.get("by")) is None:
        missing.append("independent_corpus.disjointness")
    elif disjoint.get("prompt_hash_forbidden") is not True:
        missing.append("independent_corpus.disjointness.prompt_hash_forbidden")
    external = block.get("external")
    if not isinstance(external, dict) or _text(external.get("band")) is None:
        missing.append("independent_corpus.external")
    elif external.get("download_forbidden") is not True:
        missing.append("independent_corpus.external.download_forbidden")
    return missing


def _split_contract_vocabulary(block: Any) -> list[str]:
    """Names of ``split_contract`` fields that are absent or malformed."""
    if not isinstance(block, dict):
        return ["split_contract"]
    missing: list[str] = []
    if block.get("closed") is not True:
        missing.append("split_contract.closed")
    for key in (
        "evaluation_never_tuning_input",
        "reuse_forbidden",
        "prompt_hash_disjointness_forbidden",
    ):
        if block.get(key) is not True:
            missing.append(f"split_contract.{key}")
    for key in ("disjointness_identity", "evaluation_source", "tuning_source"):
        if _text(block.get(key)) is None:
            missing.append(f"split_contract.{key}")
    return missing


def contract_thresholds(contract: Any) -> list[dict[str, Any]]:
    """The frozen threshold entries the contract declares (possibly empty)."""
    block = (contract or {}).get("threshold_policy") if isinstance(contract, dict) else None
    if not isinstance(block, dict):
        return []
    entries = block.get("thresholds")
    if not isinstance(entries, list):
        return []
    return [entry for entry in entries if isinstance(entry, dict)]


def _thresholds_vocabulary(block: dict[str, Any]) -> list[str]:
    """Names of ``threshold_policy`` fields absent or malformed while frozen.

    Frozen is a stronger contract than not-frozen: the carrier revision, the
    freeze moment, the numeric-source binding and the reference-derived entries
    all become required, and an entry is only usable when it names a relative
    artifact plus a pointer. A partial freeze (state flipped, entries missing)
    would otherwise read as a frozen contract that governs nothing.
    """
    missing: list[str] = []
    if block.get("state") != "frozen":
        return missing
    if _text(block.get("tuned_after_scoring")) != "forbidden":
        missing.append("threshold_policy.tuned_after_scoring")
    if block.get("pass_requires_frozen_threshold") is not True:
        missing.append("threshold_policy.pass_requires_frozen_threshold")
    if block.get("numeric_thresholds_present") is not True:
        missing.append("threshold_policy.numeric_thresholds_present")
    if _text(block.get("frozen_at")) is None:
        missing.append("threshold_policy.frozen_at")
    revision = block.get("freeze_revision")
    if _text(revision) is None or not _REVISION_RE.match(str(revision)):
        missing.append("threshold_policy.freeze_revision")
    if _text(block.get("freeze_scope")) is None:
        missing.append("threshold_policy.freeze_scope")
    source = block.get("source")
    if _text(source) is None:
        missing.append("threshold_policy.source")
    elif not _is_relative_path(source):
        missing.append("threshold_policy.source.path")
    pins = block.get("reference_pins")
    if not isinstance(pins, dict):
        missing.append("threshold_policy.reference_pins")
    else:
        pins_path = _text(pins.get("path"))
        if pins_path is None or not _is_relative_path(pins_path):
            missing.append("threshold_policy.reference_pins.path")
        pins_sha = pins.get("sha256")
        if _text(pins_sha) is None or not _SHA256_RE.match(str(pins_sha)):
            missing.append("threshold_policy.reference_pins.sha256")
    entries = block.get("thresholds")
    if not isinstance(entries, list) or not entries:
        missing.append("threshold_policy.thresholds")
        return missing
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            missing.append(f"threshold_policy.thresholds[{index}]")
            continue
        for key in ("threshold_id", "family_id", "metric_id", "stratum", "basis"):
            if _text(entry.get(key)) is None:
                missing.append(f"threshold_policy.thresholds[{index}].{key}")
        if _text(entry.get("direction")) not in THRESHOLD_DIRECTIONS:
            missing.append(f"threshold_policy.thresholds[{index}].direction")
        margin = entry.get("margin")
        if isinstance(margin, bool) or not isinstance(margin, (int, float)) or margin < 0:
            missing.append(f"threshold_policy.thresholds[{index}].margin")
        reference = entry.get("reference")
        if (
            not isinstance(reference, dict)
            or _text(reference.get("path")) is None
            or _text(reference.get("pointer")) is None
        ):
            missing.append(f"threshold_policy.thresholds[{index}].reference")
        elif not _is_relative_path(reference.get("path")):
            missing.append(f"threshold_policy.thresholds[{index}].reference.path")
    return missing


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

    missing.extend(_required_scope_vocabulary(contract.get("required_scope")))
    missing.extend(_assigned_population_vocabulary(contract.get("assigned_population")))
    missing.extend(_independent_corpus_vocabulary(contract.get("independent_corpus")))
    missing.extend(_split_contract_vocabulary(contract.get("split_contract")))

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
        missing.extend(_thresholds_vocabulary(thresholds))

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


def score_number(value: Any) -> Optional[float]:
    """A numeric score from either a scalar or a ``{numerator, denominator}`` map.

    A fraction is read as a number only when its denominator is positive; a zero
    denominator is "nothing to divide", not a value, so the caller can refuse it.

    Public because the adapter resolves a frozen threshold reference to a number
    with the same rule the domain uses to read a score; a second arithmetic
    implementation would let the two disagree.
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


def _score_number(value: Any) -> Optional[float]:
    """Backward-compatible private alias for :func:`score_number`."""
    return score_number(value)


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


def _threshold_family_violations(contract: dict, violations: set[str]) -> None:
    """A frozen threshold may only exist for a required, measurable family.

    Every required family must carry one (otherwise the freeze governs a
    denominator it does not gate), and a family the corpus does not measure
    (``threshold_pending``) must carry none: creating a threshold for it would
    read as a pass the measurement cannot earn (D496).
    """
    states = _required_scope_family_states(contract)
    if not states:
        return
    covered: set[str] = set()
    for entry in contract_thresholds(contract):
        family_id = _text(entry.get("family_id"))
        if family_id is None:
            continue
        covered.add(family_id)
        if family_id not in states:
            violations.add("family_outside_required_scope")
        elif states[family_id] != "required":
            violations.add("family_claimed_without_required_scope")
    for family_id, state in states.items():
        if state == "required" and family_id not in covered:
            violations.add("threshold_missing_for_required_metric")


def _row_threshold_violations(measurements: Any, contract: dict) -> set[str]:
    """A published row threshold must be one the frozen contract declares.

    The comparison runs only when the contract is frozen: before the freeze the
    domain already refuses any threshold at all (``threshold_invented_before_
    freeze``), so a second label here would blur the two defects.
    """
    if not _thresholds_frozen(contract) or not isinstance(measurements, list):
        return set()
    declared = {
        _text(entry.get("threshold_id")): entry
        for entry in contract_thresholds(contract)
        if _text(entry.get("threshold_id")) is not None
    }
    violations: set[str] = set()
    for record in measurements:
        if not isinstance(record, dict) or "threshold" not in record:
            continue
        block = record.get("threshold")
        if not isinstance(block, dict):
            violations.add("threshold_missing_for_required_metric")
            continue
        threshold_id = _text(block.get("threshold_id"))
        entry = declared.get(threshold_id) if threshold_id is not None else None
        if entry is None:
            violations.add("threshold_missing_for_required_metric")
            continue
        if block.get("state") != "frozen":
            violations.add("threshold_missing_for_required_metric")
        if _text(block.get("direction")) != _text(entry.get("direction")):
            violations.add("threshold_missing_for_required_metric")
        value = block.get("value")
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            violations.add("threshold_missing_for_required_metric")
    return violations


def _threshold_reference_violations(contract: dict, violations: set[str]) -> None:
    """A frozen threshold may not resolve against the candidate scorecard.

    The floor must be a separately pinned artifact; a threshold whose reference
    path is not the pinned reference path would compare the candidate to itself,
    so a restated candidate score would move its own threshold. The domain refuses
    the shape rather than leaving it to convention.
    """
    policy = contract.get("threshold_policy")
    pins = policy.get("reference_pins") if isinstance(policy, dict) else None
    pins_path = _text(pins.get("path")) if isinstance(pins, dict) else None
    for entry in contract_thresholds(contract):
        reference = entry.get("reference")
        if not isinstance(reference, dict):
            continue
        if pins_path is None or _text(reference.get("path")) != pins_path:
            violations.add("threshold_reference_not_independently_pinned")


def _threshold_reference_violations(contract: dict, violations: set[str]) -> None:
    """A frozen threshold may not resolve against the candidate scorecard.

    The floor must be a separately pinned artifact; a threshold whose reference
    path is not the pinned reference path would compare the candidate to itself,
    so a restated candidate score would move its own threshold. The domain refuses
    the shape rather than leaving it to convention.
    """
    policy = contract.get("threshold_policy")
    pins = policy.get("reference_pins") if isinstance(policy, dict) else None
    pins_path = _text(pins.get("path")) if isinstance(pins, dict) else None
    for entry in contract_thresholds(contract):
        reference = entry.get("reference")
        if not isinstance(reference, dict):
            continue
        if pins_path is None or _text(reference.get("path")) != pins_path:
            violations.add("threshold_reference_not_independently_pinned")


def _threshold_freeze_violations(scorecard: dict, contract: dict) -> set[str]:
    """Threshold-freeze violations: the binding, the coverage and the row claim.

    When the contract is frozen the scorecard must declare a bound freeze (state,
    carrier revision, moment, source) — the freeze is what makes a later score a
    noninferiority claim rather than a self-chosen target. A threshold block under
    a not-frozen contract is ``threshold_invented_before_freeze``.
    """
    block = scorecard.get("threshold_freeze")
    frozen = _thresholds_frozen(contract)
    if block is None:
        return {"threshold_freeze_unbound"} if frozen else set()
    if not isinstance(block, dict) or block.get("state") != "frozen":
        return {"threshold_freeze_unbound"}
    if not frozen:
        return {"threshold_invented_before_freeze"}
    if any(_text(block.get(key)) is None for key in ("revision", "frozen_at", "source")):
        return {"threshold_freeze_unbound"}
    violations: set[str] = set()
    _threshold_family_violations(contract, violations)
    _threshold_reference_violations(contract, violations)
    return violations


def _required_scope_family_states(contract: dict) -> dict[str, str]:
    """family_id -> family state, from the contract's required scope."""
    families = (contract.get("required_scope") or {}).get("families") or []
    states: dict[str, str] = {}
    for family in families:
        if not isinstance(family, dict):
            continue
        family_id = _text(family.get("family_id"))
        state = _text(family.get("state"))
        if family_id is not None and state is not None:
            states[family_id] = state
    return states


def _contract_external_band(contract: dict) -> Optional[str]:
    external = (contract.get("independent_corpus") or {}).get("external")
    if isinstance(external, dict):
        return _text(external.get("band"))
    return None


def _split_disjointness_identity(contract: dict) -> Optional[str]:
    return _text((contract.get("split_contract") or {}).get("disjointness_identity"))


def _required_scope_violations(scorecard: dict, contract: dict) -> set[str]:
    """Scope violations, only where a scorecard declares a coverage report.

    Absence of a `scope_coverage` block is not a violation: a scorecard that
    reports no family-level coverage makes no scope claim. Once it does, every
    required family must be present (``required_scope_shrunk``), no family
    outside the required scope may be declared
    (``family_outside_required_scope``), and a family the frozen corpus does not
    measure may not be claimed as measured
    (``family_claimed_without_required_scope``). A malformed block is
    ``required_scope_missing``.
    """
    block = scorecard.get("scope_coverage")
    if block is None:
        return set()
    if not isinstance(block, dict) or not _text_list(block.get("families")):
        return {"required_scope_missing"}
    required = _required_scope_family_states(contract)
    if not required:
        return {"required_scope_missing"}
    violations: set[str] = set()
    declared = list(dict.fromkeys(_text_list(block.get("families"))))
    for family_id in declared:
        if family_id not in required:
            violations.add("family_outside_required_scope")
    if not set(required) <= set(declared):
        violations.add("required_scope_shrunk")
    for family_id in _text_list(block.get("measured")):
        if family_id not in required:
            violations.add("family_outside_required_scope")
        elif required[family_id] != "required":
            violations.add("family_claimed_without_required_scope")
    return violations


def _split_violations(scorecard: dict, contract: dict) -> set[str]:
    """Split-discipline violations, only where a scorecard declares a split.

    A `document_split` block must name both sides by the contract's disjointness
    identity (document identity, never a prompt hash); a missing side or a
    different identity is ``split_contract_missing``, and an identity appearing
    on both sides is ``eval_split_reused_for_tuning``.
    """
    block = scorecard.get("document_split")
    if block is None:
        return set()
    if not isinstance(block, dict):
        return {"split_contract_missing"}
    violations: set[str] = set()
    evaluation = _text_list(block.get("evaluation_document_ids"))
    tuning = _text_list(block.get("tuning_document_ids"))
    if not evaluation or not tuning:
        violations.add("split_contract_missing")
    elif set(evaluation) & set(tuning):
        violations.add("eval_split_reused_for_tuning")
    identity = _text(block.get("identity_field"))
    expected = _split_disjointness_identity(contract)
    if identity is None or expected is None or identity != expected:
        violations.add("split_contract_missing")
    return violations


def _external_violations(scorecard: dict, contract: dict) -> set[str]:
    """External-holdout claims, only where a scorecard declares an external band.

    The external band is `bytes_absent`: a scorecard that declares any other
    band, or any external document identity, is claiming bytes it does not have
    to hold (``external_bytes_claimed``).
    """
    block = scorecard.get("external_holdout")
    if block is None:
        return set()
    if not isinstance(block, dict):
        return {"external_bytes_claimed"}
    allowed = _contract_external_band(contract)
    if allowed is None or _text(block.get("band")) != allowed:
        return {"external_bytes_claimed"}
    if block.get("bytes_present") is True or _text_list(block.get("document_ids")):
        return {"external_bytes_claimed"}
    return set()


def scorecard_violations(scorecard: Any, contract: Any) -> set[str]:
    """Every violation of one scorecard against the closed progress contract.

    An empty set means the scorecard speaks the vocabulary honestly. The
    function is pure and total: a missing or malformed contract fails closed
    with ``contract_vocabulary_missing`` rather than accepting everything.
    """
    missing_names = missing_contract_vocabulary(contract)
    if missing_names:
        violations = {"contract_vocabulary_missing"}
        if any(
            name == "required_scope" or name.startswith("required_scope.")
            for name in missing_names
        ):
            violations.add("required_scope_missing")
        if any(
            name == "split_contract" or name.startswith("split_contract.")
            for name in missing_names
        ):
            violations.add("split_contract_missing")
        return violations
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
    violations |= _row_threshold_violations(measurements, contract)
    violations |= _required_scope_violations(scorecard, contract)
    violations |= _split_violations(scorecard, contract)
    violations |= _external_violations(scorecard, contract)
    return violations
