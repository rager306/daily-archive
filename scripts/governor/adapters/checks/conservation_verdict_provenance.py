"""Independent oracle for the conservation verdict provenance DATA rule (GAR-005).

GAR-005 recorded that the P9 conservation stamp could carry a historic
`conserved` verdict beside `engine_called=false` / `computed_this_run=false`:
the honesty flags were truthful, yet the object combined a past result with a
current-run stamp, so downstream aggregation could read a not-computed verdict
as successful current-document conservation. M509-yzblqx/S01 splits the two
namespaces in the shipped Rust surface
(`crates/da-cli/src/source_cli.rs`: `current_run.verdict = NotEvaluated` plus a
relocated `reference_evidence` carrying `verdict_namespace:
historical_reference` / `evidence_state: historical`). This module is the
control-plane half of that split.

It is an INDEPENDENT oracle (ADR-051): it shares no code with the Rust
classifier `classify_conservation_verdict_provenance` and does not import it. It
re-derives currency from the fields an artifact declares about itself, which is
what makes it falsifiable against the DATA fixtures rather than a restatement of
the Rust function.

Semantics
---------
A "measured" conservation verdict (the engine's closed vocabulary) is accepted
only when ALL of the following hold:

  * it lives under the current-run namespace key, and
  * that mapping declares the current-run namespace, and
  * the observation asserts the measurement: every honesty flag is literally
    `True` and `evaluation_state` is a non-empty string other than the shipped
    `unmeasured` value.

Anything else fails closed:

  * no observation at all -> ``provenance_absent`` (absence is not compliance);
  * a measured verdict outside both declared namespaces (the pre-fix
    ``tracked_verdict: conserved`` shape) ->
    ``historic_verdict_presented_as_current``;
  * a measured verdict inside the current-run namespace without the assertion
    -> ``honesty_flags_disagree`` AND ``historic_verdict_presented_as_current``;
  * a historic claim whose relocation is not declared ->
    ``historic_reference_mislabelled``;
  * a token outside the closed vocabularies -> ``unknown_verdict_token``.

Boundary (M509/S01 Q3 R2): this is a consistency and fail-closed contract over
declared provenance, not a tamper-evident attestation. A writer able to rewrite
both the artifact and this rule can satisfy it.

The module is pure: no filesystem, network, clock or RNG access.
"""

from __future__ import annotations

from typing import Any, Iterator

__all__ = [
    "conservation_provenance_violations",
    "locate_p9_observation",
    "missing_provenance_vocabulary",
]

#: Provenance keys the oracle cannot work without. An empty/missing measured
#: vocabulary would make every record clean (fail open), so the check refuses
#: to evaluate that contract silently.
_REQUIRED_VOCABULARY = (
    "namespaces",
    "current_run_namespace",
    "historic_namespace",
    "not_evaluated_verdict",
    "measured_verdicts",
)

_VOCABULARY_COLLECTIONS = ("namespaces", "measured_verdicts")

#: Keys whose value, when it names a measured verdict, is a conservation claim.
_DEFAULT_CLAIM_KEYS = ("verdict", "tracked_verdict", "tracked_on_preserved_tei")


def _string_set(value: Any) -> set[str]:
    """The non-empty strings inside a contract collection, or an empty set."""
    if not isinstance(value, (list, tuple, set)):
        return set()
    return {item for item in value if isinstance(item, str) and item}


def missing_provenance_vocabulary(contract: Any) -> list[str]:
    """Names of required provenance keys that are absent or empty.

    A contract whose measured vocabulary is empty cannot distinguish a measured
    verdict from an unknown one, so it would accept every record. Returning the
    names lets the check flag the contract instead of failing open.
    """
    provenance = (contract or {}).get("provenance")
    if not isinstance(provenance, dict):
        return ["provenance"]
    missing: list[str] = []
    for key in _REQUIRED_VOCABULARY:
        value = provenance.get(key)
        if key in _VOCABULARY_COLLECTIONS:
            if not _string_set(value):
                missing.append(key)
        elif not isinstance(value, str) or not value:
            missing.append(key)
    return missing


def locate_p9_observation(record: Any, contract: Any) -> dict | None:
    """The P9 observation row inside one report record, or None when absent.

    The shipped document merges the P9 row into ``cascade_observe``; the
    container and stage keys are contract DATA so a rename is an edit there.
    """
    provenance = (contract or {}).get("provenance") or {}
    container_key = str(provenance.get("container_key", "cascade_observe"))
    stage_key = str(provenance.get("stage_key", "P9"))
    container = record.get(container_key) if isinstance(record, dict) else None
    if not isinstance(container, dict):
        return None
    row = container.get(stage_key)
    return row if isinstance(row, dict) else None


def _walks_outside_namespaces(node: dict, excluded: set[str]) -> Iterator[dict]:
    """Yield every mapping of the observation except the two namespaces.

    The declared namespaces are handled explicitly by the caller; everything
    else is "unscoped" and a measured token there is the GAR-005 failure shape.
    """
    stack = [node]
    while stack:
        mapping = stack.pop()
        yield mapping
        for key, value in mapping.items():
            if key in excluded:
                continue
            if isinstance(value, dict):
                stack.append(value)


def _has_oversized_value(node: Any, limit: int) -> bool:
    """True when any string in the observation exceeds the bounded identity."""
    if isinstance(node, str):
        return len(node) > limit
    if isinstance(node, dict):
        return any(_has_oversized_value(value, limit) for value in node.values())
    if isinstance(node, list):
        return any(_has_oversized_value(item, limit) for item in node)
    return False


def _oversized(node: Any, contract_provenance: dict) -> bool:
    limit = contract_provenance.get("max_value_length")
    return isinstance(limit, int) and limit > 0 and _has_oversized_value(node, limit)


def conservation_provenance_violations(record: Any, contract: Any) -> set[str]:
    """Violations of one report record against the provenance contract.

    Matches the shared fixture-oracle signature `violations_fn(record,
    contract)`. An empty set means every conservation claim the record carries
    is attributed to the run that declared it.
    """
    provenance = (contract or {}).get("provenance") or {}
    claim_keys = (
        tuple(_string_set(provenance.get("claim_keys"))) or _DEFAULT_CLAIM_KEYS
    )
    measured = _string_set(provenance.get("measured_verdicts"))
    namespaces = _string_set(provenance.get("namespaces"))
    current_namespace = str(provenance.get("current_run_namespace", "current_run"))
    historic_namespace = str(
        provenance.get("historic_namespace", "historical_reference")
    )
    historic_state = str(provenance.get("historic_evidence_state", "historical"))
    unmeasured_state = str(provenance.get("unmeasured_state", "unmeasured"))
    not_evaluated = str(provenance.get("not_evaluated_verdict", "NotEvaluated"))
    honesty_flags = tuple(
        _string_set(provenance.get("honesty_flags")) or ("engine_called",)
    )
    current_key = str(provenance.get("current_run_key", "current_run"))
    historic_key = str(provenance.get("historic_key", "reference_evidence"))

    violations: set[str] = set()
    if not isinstance(record, dict):
        return {"missing_record_object"}

    observation = locate_p9_observation(record, contract)
    if observation is None:
        # Absence is not compliance: a report with no provenance publishes no
        # currency, and a reader must be told that rather than shown a pass.
        return {"provenance_absent"}

    if _oversized(observation, provenance):
        violations.add("oversized_provenance_value")

    engine_measured = all(observation.get(flag) is True for flag in honesty_flags)

    # ── current-run namespace ────────────────────────────────────────────
    current = observation.get(current_key)
    if not isinstance(current, dict):
        violations.add("missing_current_run")
    else:
        declared = current.get("verdict_namespace")
        if not isinstance(declared, str) or not declared:
            violations.add("namespace_not_declared")
        elif declared not in namespaces or declared != current_namespace:
            violations.add("namespace_contradicts_key")

        verdict = current.get("verdict")
        if not isinstance(verdict, str) or not verdict:
            violations.add("missing_current_verdict")
        elif verdict == not_evaluated:
            # The run explicitly reports that it did not measure conservation.
            # That is the honest state, not a claim to accept or reject.
            pass
        elif verdict in measured:
            evaluation_state = current.get("evaluation_state")
            asserted = (
                engine_measured
                and isinstance(evaluation_state, str)
                and evaluation_state != ""
                and evaluation_state != unmeasured_state
            )
            if not asserted:
                # A past result wearing a current-run stamp.
                violations.add("honesty_flags_disagree")
                violations.add("historic_verdict_presented_as_current")
        else:
            violations.add("unknown_verdict_token")

    # ── relocated historic namespace ─────────────────────────────────────
    reference = observation.get(historic_key)
    if reference is not None:
        if not isinstance(reference, dict):
            violations.add("historic_reference_mislabelled")
        else:
            verdict = reference.get("verdict")
            if isinstance(verdict, str) and verdict and verdict != not_evaluated:
                if verdict not in measured:
                    violations.add("unknown_verdict_token")
                elif (
                    reference.get("verdict_namespace") != historic_namespace
                    or reference.get("evidence_state") != historic_state
                ):
                    violations.add("historic_reference_mislabelled")

    # ── unscoped claims ─────────────────────────────────────────────────
    # Any measured verdict that is not inside one of the two declared
    # namespaces has no currency declaration at all. This is the pre-fix
    # `tracked_verdict: conserved` shape, and rejecting it is the point of the
    # rule: a bare token must not read as a current measurement.
    for mapping in _walks_outside_namespaces(observation, {current_key, historic_key}):
        for key in claim_keys:
            value = mapping.get(key)
            if isinstance(value, str) and value in measured:
                violations.add("historic_verdict_presented_as_current")

    return violations
