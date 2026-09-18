"""Independent oracle for the executed population binding DATA rule (GAR-006).

GAR-006 recorded that a shadow report could publish a corpus-scale identity
number beside a small executed population and still read as a corpus-scale
result: the reference, executed and evaluated populations were either one
undifferentiated object or a presence-only split, so the number describing the
*referenced* population could be consumed as if it described what was
*executed*. M509-yzblqx/S02 splits the three populations in every shadow
producer (T01) and this module is the control-plane half: it refuses a
corpus-scale claim that is not bound to execution evidence for that population.

It is an INDEPENDENT oracle (ADR-051): it shares no code with the Rust shadow
producers or with the data contract in ``crates/da-parse/tests/data_contracts.rs``
and does not import them. It re-derives the relation from the fields a report
declares about itself, which is what makes it falsifiable against the DATA
fixtures rather than a restatement of the producer.

Semantics
---------
A report *claims* corpus execution when any of the following holds:

  * ``executed_population.corpus_execution`` is literally ``True``;
  * ``evaluated_population.status`` is a measured status (a non-empty string
    outside the declared ``not_evaluated_statuses`` set);
  * ``executed_population.documents`` reaches ``corpus_scale_documents``.

A claim is accepted only when the report carries execution evidence for the
population it claims:

  * evidence is absent/empty -> ``corpus_claim_without_execution_evidence``
    (a corpus number without evidence is the GAR-006 fabrication shape);
  * evidence is a string whose token before ``:`` is not a declared
    ``evidence_kinds`` member -> ``unknown_execution_evidence_kind`` (the
    pre-fix ``"n/a"`` shape);
  * evidence is a string and the executed count is corpus-scale ->
    ``execution_evidence_unbound``: a bare token cannot bind a corpus-scale
    number, so replaying a valid token from another report is rejected;
  * evidence is a mapping whose ``population_identity``/``documents`` disagree
    with the executed population (or cannot be compared at all) ->
    ``execution_receipt_mismatch``: a spliced receipt is rejected.

Relations are checked in the same pass, because a number is only honest
relative to the population it came from:

  * ``executed.documents > reference.documents`` (with a positive reference)
    -> ``executed_population_exceeds_reference``;
  * ``evaluated.documents > executed.documents`` ->
    ``evaluated_population_exceeds_executed`` (a corpus-scale reference count
    presented as an evaluation outcome of a small execution);
  * a measured status with ``corpus_execution`` not literally true ->
    ``measured_status_without_corpus_execution``.

Absence is never compliance: a missing reference/executed/evaluated population,
a population without a numeric ``documents`` count, or a record that is not an
object all fail closed with their own label. Identity and evidence strings are
bounded by ``max_value_length`` (M509/S02 Q3 R4 transfer) and an oversized value
is ``oversized_population_value``.

Boundary (M509/S02 Q3 R2, stated rather than implied): this is a consistency and
fail-closed contract over the populations an artifact declares about itself, not
a tamper-evident attestation. A writer able to rewrite both the report and this
DATA rule can satisfy it. Its value is that absence, drift and self-contradiction
stop passing as compliance, and that a corpus-scale number must name the evidence
that produced it.

The module is pure: no filesystem, network, clock or RNG access.
"""

from __future__ import annotations

from typing import Any, Iterator

__all__ = [
    "declared_report_paths",
    "executed_population_violations",
    "missing_binding_vocabulary",
]

#: Population contract keys the oracle cannot work without. Without them the
#: oracle cannot locate the populations and would accept every record (fail
#: open), so a missing/empty value is reported to the caller instead.
_REQUIRED_POPULATION_KEYS = (
    "reference_key",
    "executed_key",
    "evaluated_key",
    "documents_key",
    "execution_flag",
    "evidence_key",
    "status_key",
)

#: Collections that must be non-empty, and scalars that must be positive ints.
_REQUIRED_POPULATION_COLLECTIONS = ("not_evaluated_statuses", "evidence_kinds")
_REQUIRED_POPULATION_SCALARS = ("max_value_length", "corpus_scale_documents")

#: Keys of the shared report-contract locator inside the DATA rule.
_REQUIRED_REPORT_CONTRACT_KEYS = ("path", "key", "paths_key")

_DEFAULT_POPULATION_KEYS = {
    "reference_key": "reference_population",
    "executed_key": "executed_population",
    "evaluated_key": "evaluated_population",
    "documents_key": "documents",
    "identity_key": "identity",
    "execution_flag": "corpus_execution",
    "evidence_key": "execution_evidence",
    "status_key": "status",
}

_DEFAULT_RECEIPT_KEYS = {
    "receipt_documents_key": "documents",
    "receipt_identity_key": "population_identity",
    "receipt_kind_key": "kind",
}


def _string_set(value: Any) -> set[str]:
    """The non-empty strings inside a contract collection, or an empty set."""
    if not isinstance(value, (list, tuple, set)):
        return set()
    return {item for item in value if isinstance(item, str) and item}


def missing_binding_vocabulary(contract: Any) -> list[str]:
    """Names of required binding keys that are absent or empty.

    A contract whose population keys, evidence vocabulary or corpus-scale
    threshold is empty cannot distinguish a bound claim from an unbound one, so
    it would accept every record. Returning the names lets the check flag the
    contract instead of failing open.
    """
    populations = (contract or {}).get("populations")
    if not isinstance(populations, dict):
        return ["populations"]
    missing: list[str] = []
    for key in _REQUIRED_POPULATION_KEYS:
        value = populations.get(key)
        if not isinstance(value, str) or not value:
            missing.append(key)
    for key in _REQUIRED_POPULATION_COLLECTIONS:
        if not _string_set(populations.get(key)):
            missing.append(key)
    for key in _REQUIRED_POPULATION_SCALARS:
        value = populations.get(key)
        if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
            missing.append(key)
    report_contract = (contract or {}).get("report_contract")
    if not isinstance(report_contract, dict):
        missing.append("report_contract")
    else:
        for key in _REQUIRED_REPORT_CONTRACT_KEYS:
            value = report_contract.get(key)
            if not isinstance(value, str) or not value:
                missing.append(f"report_contract.{key}")
    return missing


def declared_report_paths(manifest: Any, contract: Any) -> list[str]:
    """The report paths the shared population contract enumerates.

    The governed reports come from `data/parse/acceptance_manifest.yaml` so the
    binding rule and the population-split contract cannot disagree about which
    shadow reports exist. Pure: the caller owns the file I/O.
    """
    report_contract = (contract or {}).get("report_contract") or {}
    section_key = str(report_contract.get("key") or "population_contract")
    paths_key = str(report_contract.get("paths_key") or "report_paths")
    section = manifest.get(section_key) if isinstance(manifest, dict) else None
    if not isinstance(section, dict):
        return []
    paths = section.get(paths_key)
    if not isinstance(paths, list):
        return []
    return [path for path in paths if isinstance(path, str) and path]


def _count(value: Any) -> int | None:
    """A non-negative population count, or None when the value is not one."""
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value
    if isinstance(value, float) and value.is_integer():
        return int(value)
    return None


def _strings(node: Any) -> Iterator[str]:
    """Every string reachable from a nested dict/list structure."""
    if isinstance(node, str):
        yield node
    elif isinstance(node, dict):
        for value in node.values():
            yield from _strings(value)
    elif isinstance(node, list):
        yield from (item for value in node for item in _strings(value))


def _evidence_kind(evidence: str) -> str:
    """The declared kind of a string evidence token (`kind:target` -> kind)."""
    return evidence.split(":", 1)[0].strip()


def executed_population_violations(record: Any, contract: Any) -> set[str]:
    """Violations of one shadow report document against the binding contract.

    Matches the shared fixture-oracle signature `violations_fn(record,
    contract)`. An empty set means every population the report publishes is
    consistent, and every corpus-scale claim it makes names the execution
    evidence that produced it.
    """
    populations = (contract or {}).get("populations") or {}
    keys = {
        name: str(populations.get(name) or default)
        for name, default in _DEFAULT_POPULATION_KEYS.items()
    }
    receipt_keys = {
        name: str(populations.get(name) or default)
        for name, default in _DEFAULT_RECEIPT_KEYS.items()
    }
    not_evaluated = _string_set(populations.get("not_evaluated_statuses"))
    evidence_kinds = _string_set(populations.get("evidence_kinds"))
    threshold = populations.get("corpus_scale_documents")
    threshold = threshold if isinstance(threshold, int) and not isinstance(threshold, bool) else 0
    limit = populations.get("max_value_length")
    limit = limit if isinstance(limit, int) and not isinstance(limit, bool) else 0

    if not isinstance(record, dict):
        return {"missing_record_object"}

    violations: set[str] = set()
    blocks: dict[str, dict] = {}
    for name in ("reference", "executed", "evaluated"):
        block = record.get(keys[f"{name}_key"])
        if not isinstance(block, dict):
            violations.add(f"{name}_population_absent")
            continue
        blocks[name] = block
    if len(blocks) != 3:
        # A missing population is its own finding; relations cannot be decided
        # on a partial report, and inventing them would hide the absence.
        return violations

    reference, executed, evaluated = (
        blocks["reference"], blocks["executed"], blocks["evaluated"]
    )
    counts = {
        name: _count(block.get(keys["documents_key"]))
        for name, block in blocks.items()
    }
    if any(count is None for count in counts.values()):
        violations.add("population_count_absent")

    if limit > 0 and any(
        len(text) > limit
        for block in (reference, executed, evaluated)
        for text in _strings(block)
    ):
        violations.add("oversized_population_value")

    reference_documents = counts["reference"]
    executed_documents = counts["executed"]
    evaluated_documents = counts["evaluated"]

    # ── relations ────────────────────────────────────────────────────────
    if (
        reference_documents is not None
        and reference_documents > 0
        and executed_documents is not None
        and executed_documents > reference_documents
    ):
        violations.add("executed_population_exceeds_reference")
    if (
        executed_documents is not None
        and evaluated_documents is not None
        and evaluated_documents > executed_documents
    ):
        violations.add("evaluated_population_exceeds_executed")

    status = evaluated.get(keys["status_key"])
    measured = (
        isinstance(status, str)
        and status != ""
        and status not in not_evaluated
    )
    corpus_execution = executed.get(keys["execution_flag"])
    corpus_scale = (
        executed_documents is not None
        and threshold > 0
        and executed_documents >= threshold
    )

    if measured and corpus_execution is not True:
        violations.add("measured_status_without_corpus_execution")

    # ── the claim and its evidence ───────────────────────────────────────
    claims = corpus_execution is True or measured or corpus_scale
    if not claims:
        return violations

    evidence = executed.get(keys["evidence_key"])
    if evidence is None or (isinstance(evidence, str) and not evidence.strip()) or isinstance(evidence, bool) or isinstance(evidence, (int, float)):
        violations.add("corpus_claim_without_execution_evidence")
        return violations

    if isinstance(evidence, str):
        if evidence_kinds and _evidence_kind(evidence) not in evidence_kinds:
            violations.add("unknown_execution_evidence_kind")
        if corpus_scale:
            # A bare token cannot bind a corpus-scale number: replaying a valid
            # token from a different report is indistinguishable, so it is not
            # accepted as execution evidence for THIS population.
            violations.add("execution_evidence_unbound")
        return violations

    if isinstance(evidence, dict):
        receipt_documents = _count(evidence.get(receipt_keys["receipt_documents_key"]))
        if receipt_documents is None or receipt_documents != executed_documents:
            violations.add("execution_receipt_mismatch")
        identity = evidence.get(receipt_keys["receipt_identity_key"])
        target = executed.get(keys["identity_key"]) or reference.get(keys["identity_key"])
        if not isinstance(target, str) or not target:
            # Nothing in the report declares which population was executed, so
            # the receipt cannot be bound to it.
            violations.add("execution_receipt_mismatch")
        elif not isinstance(identity, str) or not identity or identity != target:
            violations.add("execution_receipt_mismatch")
        kind = evidence.get(receipt_keys["receipt_kind_key"])
        if isinstance(kind, str) and kind and evidence_kinds and kind not in evidence_kinds:
            violations.add("unknown_execution_evidence_kind")
        return violations

    violations.add("corpus_claim_without_execution_evidence")
    return violations
