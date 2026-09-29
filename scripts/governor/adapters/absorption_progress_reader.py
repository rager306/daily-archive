"""Adapter + independent oracle for the absorption progress scorecard (M514/S01/T04).

Two responsibilities live here on purpose:

* ``FilesystemAbsorptionProgressReader`` implements the port
  ``governor.ports.absorption_progress_reader`` — it owns every file read and
  every ``sha256`` computation, and it confines each path to the project root.
* ``absorption_progress_violations`` is the pure regression oracle the Governor
  check runs. It composes the T01 pure domain validator
  (``governor.domain.absorption_progress.scorecard_violations``) with the
  *identity and direction* checks the domain deliberately leaves out: a scalar
  must still be bound to the bytes that produced it, two rows may only be
  compared inside one accepted scope, a savings claim must name a paired CPU
  measurement, and a later scorecard may not shrink the denominator or drop an
  axis that the recorded census already measured.

Why the composition (M513 closeout): the recorded progress surface could publish
a number whose evidence artifact had been substituted, compare a new corpus
against the old one as if it were the same series, and claim a resource saving
measured only on the wall clock. Each shape is now a typed label:

* ``evidence_artifact_missing`` — a bound evidence path no longer resolves;
* ``evidence_hash_substituted`` — the declared ``sha256`` no longer matches the
  artifact bytes (the scorecard is stale; it is flagged, never rewritten);
* ``evidence_path_not_relative`` — a host or absolute path, which the evidence
  policy forbids;
* ``oracle_changed`` / ``scope_not_accepted`` / ``scope_mixed_in_scorecard`` —
  a value that is not comparable to the accepted series;
* ``savings_without_paired_cpu`` / ``savings_paired_cpu_not_measured`` — a
  resource saving claimed without the paired CPU measurement that would make it
  a product measurement;
* ``coverage_regression`` — an axis the recorded census measured is now silent;
* ``denominator_shrink`` — the full denominator got smaller than the pinned
  census. Direction only: S01 freezes no numeric threshold, so a regression is a
  state or ordering change, never a tuned margin;
* ``threshold_freeze_revision_mismatch`` (S02) — the scorecard's declared freeze
  revision no longer matches the schema bytes, i.e. the carrier was edited after
  the freeze. Re-tuning a threshold therefore invalidates the freeze instead of
  passing silently;
* ``threshold_reference_unbound`` (S02) — a frozen threshold could not be
  resolved from the artifact + pointer it names, so its number does not exist.

Frozen thresholds (M514/S02/T02): the contract fixes them before held-out
scoring and binds them to the carrier revision. This adapter resolves each
threshold's `reference` (path + pointer) to a number with the domain's own
``score_number`` rule, publishes the resolved values in observations, annotates
measured rows that publish no threshold of their own, and recomputes the schema
sha256 so the freeze is checked against bytes rather than trusted.

Boundary (stated, not implied — E3, carried from M509/S02 Q3 R2): this is a
consistency and fail-closed contract over what a scorecard declares about itself
plus a recomputed content hash. It is **not** a tamper-evident attestation: an
actor able to rewrite the scorecard, the evidence artifact and the DATA rule can
satisfy it. Its value is that absence, drift, scope mixing and self-contradiction
stop passing as compliance, and that a coverage number must name the bytes that
produced it.

The oracle is pure: it takes the scorecard, the merged contract and an
observations mapping, and never touches the filesystem. The reader is the only
filesystem code. Determinism: every returned set is a plain ``set[str]`` of
closed labels, and the caller sorts it, so the same tree yields the same report.
"""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path
from typing import Any, Optional

import yaml

from governor.domain.absorption_progress import (
    contract_thresholds,
    missing_contract_vocabulary,
    score_number,
    scorecard_violations,
)
from governor.ports.absorption_progress_reader import AbsorptionProgressReader

__all__ = [
    "FilesystemAbsorptionProgressReader",
    "absorption_progress_violations",
    "effective_progress_contract",
    "missing_progress_vocabulary",
]

#: A bound byte hash and a bound revision, as declared by the evidence contract.
_SHA256_RE = re.compile(r"^sha256:[0-9a-f]{64}$")

#: Rule keys the T04 DATA check contributes on top of the T01 vocabulary. Only
#: these are merged; the rest of the schema stays the single source of truth for
#: the closed vocabulary, and the check's own `schema_version` never overwrites
#: the scorecard schema version the domain validator compares against.
_RULE_KEYS = (
    "severity",
    "dimension",
    "anchor",
    "violations",
    "source_markers",
    "graph_task_suite",
    "fixtures",
    "regression",
    "resources_claim",
    "comparability_rules",
)


class FilesystemAbsorptionProgressReader(AbsorptionProgressReader):
    """Reads the scorecard, its vocabulary and content hashes from disk."""

    def __init__(
        self,
        project_root: Path,
        contract_path: str,
        scorecard_path: str,
    ):
        self.root = Path(project_root)
        self.contract_path = contract_path
        self.scorecard_path = scorecard_path

    # ── path confinement ───────────────────────────────────────────────────
    def _resolve(self, relative_path: str) -> Optional[Path]:
        """A repository-relative path inside the root, or None when it escapes."""
        if not isinstance(relative_path, str) or not relative_path.strip():
            return None
        candidate = Path(relative_path)
        if candidate.is_absolute() or candidate.drive:
            return None
        if ".." in candidate.parts:
            return None
        return self.root / candidate

    # ── reads ──────────────────────────────────────────────────────────────
    def read_contract(self) -> Optional[dict[str, Any]]:
        path = self._resolve(self.contract_path)
        if path is None or not path.is_file():
            return None
        try:
            document = yaml.safe_load(path.read_text(encoding="utf-8"))
        except (OSError, yaml.YAMLError, UnicodeDecodeError):
            return None
        return document if isinstance(document, dict) else None

    def read_scorecard(self) -> Optional[dict[str, Any]]:
        return self.read_json(self.scorecard_path)

    def read_json(self, relative_path: str) -> Optional[dict[str, Any]]:
        path = self._resolve(relative_path)
        if path is None or not path.is_file():
            return None
        try:
            document = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError, UnicodeDecodeError):
            return None
        return document if isinstance(document, dict) else None

    def file_sha256(self, relative_path: str) -> Optional[str]:
        path = self._resolve(relative_path)
        if path is None or not path.is_file():
            return None
        try:
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
        except OSError:
            return None
        return f"sha256:{digest}"

    def resolve_threshold_references(
        self, contract: Any
    ) -> dict[str, Optional[float]]:
        """Resolve every frozen threshold's declared reference to a number.

        The contract carries no numeric literal; each threshold names an artifact
        plus a pointer, and this is where that pointer becomes a value. ``None``
        means the reference did not resolve (missing artifact, escaped path, bad
        pointer, or a non-numeric target), which the oracle reports as
        ``threshold_reference_unbound`` rather than guessing.
        """
        resolved: dict[str, Optional[float]] = {}
        for entry in contract_thresholds(contract):
            threshold_id = _text(entry.get("threshold_id"))
            if threshold_id is None:
                continue
            reference = entry.get("reference")
            value: Optional[float] = None
            if isinstance(reference, dict):
                rel = _text(reference.get("path"))
                pointer = _text(reference.get("pointer"))
                if rel is not None and _is_relative_path(rel) and pointer is not None:
                    document = self.read_json(rel)
                    if isinstance(document, dict):
                        value = score_number(resolve_pointer(document, pointer))
            resolved[threshold_id] = value
        return resolved

    def annotate_thresholds(
        self,
        scorecard: Any,
        contract: Any,
        resolved: Optional[dict[str, Optional[float]]] = None,
    ) -> Any:
        """Return the scorecard with a frozen `threshold` on every measured row.

        Only rows that publish no threshold of their own are annotated, so the
        scorecard's own declaration is never silently overwritten by the reader:
        a wrong published value stays wrong and is caught, rather than repaired.
        The annotated block names the contract threshold, its direction, margin
        and resolved value, so the row carries what it is measured against.
        """
        if not isinstance(scorecard, dict):
            return scorecard
        measurements = scorecard.get("measurements")
        if not isinstance(measurements, list):
            return scorecard
        thresholds = contract_thresholds(contract)
        if not thresholds:
            return scorecard
        resolved = resolved if isinstance(resolved, dict) else {}
        by_metric: dict[str, dict[str, Any]] = {}
        for entry in thresholds:
            metric_id = _text(entry.get("metric_id"))
            if metric_id is not None and metric_id not in by_metric:
                by_metric[metric_id] = entry
        annotated_rows: list[Any] = []
        for row in measurements:
            if (
                isinstance(row, dict)
                and row.get("state") == "measured"
                and "threshold" not in row
            ):
                entry = by_metric.get(_text(row.get("metric_id")))
                if entry is not None:
                    threshold_id = _text(entry.get("threshold_id"))
                    row = dict(row)
                    row["threshold"] = {
                        "threshold_id": threshold_id,
                        "metric_id": _text(entry.get("metric_id")),
                        "stratum": _text(entry.get("stratum")),
                        "direction": _text(entry.get("direction")),
                        "margin": entry.get("margin"),
                        "value": resolved.get(threshold_id) if threshold_id else None,
                        "state": "frozen",
                        "reference": dict(entry.get("reference") or {}),
                    }
            annotated_rows.append(row)
        annotated = dict(scorecard)
        annotated["measurements"] = annotated_rows
        return annotated

    def path_exists(self, relative_path: str) -> bool:
        path = self._resolve(relative_path)
        return path is not None and path.exists()


# ── contract composition ───────────────────────────────────────────────────


def effective_progress_contract(
    schema_contract: Any, rule_contract: Any
) -> dict[str, Any]:
    """Merge the T04 rule DATA over the T01 closed vocabulary.

    The T01 schema owns the axes, states, denominator partition, census and
    evidence policy; the T04 check adds only the rule configuration the domain
    deliberately does not carry (accepted scope pin, expected axis states and the
    savings/CPU pairing). Merging the whole rule document would let its own
    ``schema_version`` shadow the scorecard schema version the domain validator
    compares against, so only ``_RULE_KEYS`` are copied.
    """
    merged: dict[str, Any] = dict(schema_contract) if isinstance(schema_contract, dict) else {}
    if isinstance(rule_contract, dict):
        for key in _RULE_KEYS:
            if key in rule_contract:
                merged[key] = rule_contract[key]
    for key in ("regression", "resources_claim", "comparability_rules"):
        if not isinstance(merged.get(key), dict):
            merged[key] = {}
    return merged


def missing_progress_vocabulary(contract: Any) -> list[str]:
    """Dotted names of required vocabulary keys that are absent or unusable.

    A contract missing any of these cannot tell an honest scorecard from a
    dishonest one: without the accepted scope every series looks comparable,
    without the expected axis states a silent axis looks like progress, and
    without the savings/CPU pairing a wall-clock claim looks like a product
    saving. The caller fails closed on a non-empty result.
    """
    missing = list(missing_contract_vocabulary(contract))
    if not isinstance(contract, dict):
        return missing

    regression = contract.get("regression")
    expected = regression.get("expected_axis_states") if isinstance(regression, dict) else None
    if not isinstance(expected, dict) or not expected:
        missing.append("regression.expected_axis_states")

    comparability = contract.get("comparability_rules")
    if not isinstance(comparability, dict):
        missing.append("comparability_rules")
    else:
        if not isinstance(comparability.get("accepted"), dict) or not comparability["accepted"]:
            missing.append("comparability_rules.accepted")
        if not _text(comparability.get("oracle_pin")):
            missing.append("comparability_rules.oracle_pin")

    resources = contract.get("resources_claim")
    if not isinstance(resources, dict):
        missing.append("resources_claim")
    else:
        for key in ("axis", "savings_flag", "paired_cpu_field", "kind_field", "cpu_kind"):
            if not _text(resources.get(key)):
                missing.append(f"resources_claim.{key}")
    return missing


# ── shared value helpers ───────────────────────────────────────────────────


#: Marker returned by :func:`resolve_pointer` when a pointer cannot be followed.
_UNRESOLVED = object()

#: One pointer segment: an object key, optionally selecting from a list of
#: mappings by ``field=value``. Enough to address a measurement by its metric_id
#: without inventing a JSON-pointer library (and its escape grammar) for it.
_POINTER_SELECTOR_RE = re.compile(r"^([A-Za-z0-9_]+)\[([A-Za-z0-9_]+)=([^\[\]]+)\]$")


def resolve_pointer(document: Any, pointer: Any) -> Any:
    """Follow a slash-separated ``pointer`` into ``document``.

    Grammar (deliberately small, so it is reviewable):

      * ``key`` addresses an object member;
      * ``key[field=value]`` selects, from the list at ``key``, the first mapping
        whose ``field`` equals ``value``.

    Returns :data:`_UNRESOLVED` when any segment does not resolve, so a caller
    can tell "the answer is None" from "the pointer did not resolve".
    """
    text = _text(pointer)
    if text is None or not isinstance(document, (dict, list)):
        return _UNRESOLVED
    node: Any = document
    for raw in text.split("/"):
        segment = raw.strip()
        if not segment:
            return _UNRESOLVED
        match = _POINTER_SELECTOR_RE.match(segment)
        if match is not None:
            key, field, wanted = match.groups()
            if not isinstance(node, dict) or key not in node:
                return _UNRESOLVED
            candidates = node[key]
            if not isinstance(candidates, list):
                return _UNRESOLVED
            found = next(
                (
                    item
                    for item in candidates
                    if isinstance(item, dict) and str(item.get(field)) == wanted
                ),
                _UNRESOLVED,
            )
            if found is _UNRESOLVED:
                return _UNRESOLVED
            node = found
        else:
            if not isinstance(node, dict) or segment not in node:
                return _UNRESOLVED
            node = node[segment]
    return node


def _text(value: Any) -> Optional[str]:
    if isinstance(value, str) and value.strip():
        return value
    return None


def _count(value: Any) -> Optional[int]:
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value
    if isinstance(value, float) and value.is_integer():
        return int(value)
    return None


def _is_relative_path(value: Any) -> bool:
    text = _text(value)
    if text is None:
        return False
    if text.startswith("/") or text.startswith("\\"):
        return False
    if ".." in text.split("/"):
        return False
    if re.match(r"^[A-Za-z]:", text):  # Windows drive letter is absolute
        return False
    return True


def _rows(scorecard: dict[str, Any]) -> list[dict[str, Any]]:
    measurements = scorecard.get("measurements")
    if not isinstance(measurements, list):
        return []
    return [row for row in measurements if isinstance(row, dict)]


def _measured_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [row for row in rows if row.get("state") == "measured"]


def _comparability_keys(contract: dict[str, Any]) -> tuple[str, ...]:
    keys = (contract.get("comparability") or {}).get("keys")
    if isinstance(keys, list):
        named = tuple(key for key in keys if isinstance(key, str) and key)
        if named:
            return named
    return ("corpus_id", "profile", "oracle", "source_revision")


def _scope_accepted(rows: list[dict[str, Any]], contract: dict[str, Any]) -> bool:
    """True when every measured row sits in the accepted series.

    A scorecard in a different corpus, profile or oracle is a *new series*, not a
    regression: it is reported as ``scope_not_accepted`` and no coverage or
    denominator comparison is made against it.
    """
    measured = _measured_rows(rows)
    if not measured:
        return False
    rules = contract.get("comparability_rules") or {}
    accepted = rules.get("accepted") or {}
    pin = _text(rules.get("oracle_pin"))
    for row in measured:
        block = row.get("comparability")
        if not isinstance(block, dict):
            return False
        if pin and _text(block.get("oracle")) != pin:
            return False
        for key, expected in accepted.items():
            if _text(block.get(key)) != _text(expected):
                return False
    return True


# ── the oracle ─────────────────────────────────────────────────────────────


def absorption_progress_violations(
    scorecard: Any,
    contract: Any,
    observations: Optional[dict[str, Any]] = None,
) -> set[str]:
    """Every violation of one scorecard against the progress contract.

    ``observations`` carries what the reader recomputed from disk:

    ``{"evidence_sha256": {relative_path: "sha256:<hex>" | None}}``.

    A path present with value ``None`` is a missing/unreadable artifact; a path
    absent from the mapping was never resolved, so it is left unverified rather
    than guessed. An empty set means the scorecard speaks the closed vocabulary,
    every scalar it publishes is still bound to the bytes that produced it, and
    it is either in the accepted series or explicitly marked not comparable.
    """
    if not isinstance(contract, dict):
        return {"contract_vocabulary_missing"}
    violations = set(scorecard_violations(scorecard, contract))
    if not isinstance(scorecard, dict):
        return violations

    observations = observations if isinstance(observations, dict) else {}
    rows = _rows(scorecard)

    _scope_violations(rows, contract, violations)
    _evidence_violations(rows, observations, violations)
    _resources_violations(rows, contract, violations)
    _threshold_violations(scorecard, contract, observations, violations)
    if _scope_accepted(rows, contract):
        _regression_violations(scorecard, rows, contract, violations)
    return violations


def _threshold_violations(
    scorecard: dict[str, Any],
    contract: dict[str, Any],
    observations: dict[str, Any],
    violations: set[str],
) -> None:
    """Frozen-threshold violations the domain cannot see (it cannot read files).

    ``threshold_freeze_revision_mismatch`` compares the scorecard's declared
    carrier revision with the schema sha256 the reader recomputed. This is the
    whole point of the freeze: editing the schema (re-tuning a threshold) changes
    the digest and invalidates the declaration instead of passing silently.

    ``threshold_reference_unbound`` fires only when resolutions were supplied and
    one came back unresolved; a scorecard validated without resolutions is left
    unverified rather than assumed bound.
    """
    block = scorecard.get("threshold_freeze")
    if isinstance(block, dict) and block.get("state") == "frozen":
        declared = _text(block.get("revision"))
        observed = _text(observations.get("schema_sha256"))
        if declared is not None and observed is not None and declared != observed:
            violations.add("threshold_freeze_revision_mismatch")
    resolved = observations.get("threshold_values")
    if isinstance(resolved, dict) and resolved:
        for entry in contract_thresholds(contract):
            threshold_id = _text(entry.get("threshold_id"))
            if threshold_id in resolved and resolved.get(threshold_id) is None:
                violations.add("threshold_reference_unbound")


def _scope_violations(
    rows: list[dict[str, Any]], contract: dict[str, Any], violations: set[str]
) -> None:
    rules = contract.get("comparability_rules") or {}
    accepted = rules.get("accepted") or {}
    pin = _text(rules.get("oracle_pin"))
    keys = _comparability_keys(contract)
    seen: set[tuple] = set()
    for row in _measured_rows(rows):
        block = row.get("comparability")
        if not isinstance(block, dict):
            continue
        oracle = _text(block.get("oracle"))
        if pin and oracle != pin:
            violations.add("oracle_changed")
        for key, expected in accepted.items():
            if _text(block.get(key)) != _text(expected):
                violations.add("scope_not_accepted")
        seen.add(tuple((key, _text(block.get(key))) for key in keys))
    # Two measured rows in different series are not comparable to each other even
    # when neither declares a `compared_to` target, so the mix is refused here
    # rather than left to the domain's compared_to-only path.
    if len(seen) > 1:
        violations.add("scope_mixed_in_scorecard")


def _evidence_violations(
    rows: list[dict[str, Any]], observations: dict[str, Any], violations: set[str]
) -> None:
    hashes = observations.get("evidence_sha256")
    if not isinstance(hashes, dict):
        hashes = {}
    for row in rows:
        evidence = row.get("evidence")
        if not isinstance(evidence, dict):
            continue
        path = _text(evidence.get("path"))
        if path is None:
            continue
        if not _is_relative_path(path):
            violations.add("evidence_path_not_relative")
        declared = _text(evidence.get("sha256"))
        if path not in hashes:
            continue
        recomputed = hashes.get(path)
        if recomputed is None:
            violations.add("evidence_artifact_missing")
        elif declared and _SHA256_RE.match(declared) and declared != recomputed:
            # The artifact bytes changed under a scorecard that still claims the
            # old digest. The scorecard is stale; regenerate it, never rewrite
            # the historical carrier or the digest in place.
            violations.add("evidence_hash_substituted")


def _resources_violations(
    rows: list[dict[str, Any]], contract: dict[str, Any], violations: set[str]
) -> None:
    rules = contract.get("resources_claim")
    if not isinstance(rules, dict):
        return
    axis = _text(rules.get("axis"))
    savings_flag = _text(rules.get("savings_flag"))
    paired_field = _text(rules.get("paired_cpu_field"))
    kind_field = _text(rules.get("kind_field"))
    cpu_kind = _text(rules.get("cpu_kind"))
    if not all((axis, savings_flag, paired_field, kind_field, cpu_kind)):
        return
    by_id = {
        _text(row.get("metric_id")): row
        for row in rows
        if _text(row.get("metric_id")) is not None
    }
    for row in rows:
        if row.get("axis") != axis or row.get(savings_flag) is not True:
            continue
        reference = row.get(paired_field)
        target_id = None
        if isinstance(reference, dict):
            target_id = _text(reference.get("metric_id"))
        elif isinstance(reference, str):
            target_id = _text(reference)
        target = by_id.get(target_id) if target_id is not None else None
        if (
            target is None
            or target.get("axis") != axis
            or target.get(kind_field) != cpu_kind
        ):
            violations.add("savings_without_paired_cpu")
        elif target.get("state") != "measured":
            violations.add("savings_paired_cpu_not_measured")


def _regression_violations(
    scorecard: dict[str, Any],
    rows: list[dict[str, Any]],
    contract: dict[str, Any],
    violations: set[str],
) -> None:
    census = ((contract.get("denominator") or {}).get("initial_census")) or {}
    denominator = scorecard.get("denominator")
    if isinstance(denominator, dict):
        for observed_key, pinned_key in (
            ("assigned", "declared"),
            ("executed", "measurable"),
            ("excluded", "excluded"),
        ):
            observed = _count(denominator.get(observed_key))
            expected = _count(census.get(pinned_key))
            if observed is not None and expected is not None and observed < expected:
                violations.add("denominator_shrink")

    # Expected states are a floor only where the census already measured an axis:
    # an axis that moved from not_measured to measured is progress, not drift.
    expected_states = (contract.get("regression") or {}).get("expected_axis_states") or {}
    if isinstance(expected_states, dict):
        for axis, state in expected_states.items():
            if state != "measured":
                continue
            if not any(
                row.get("axis") == axis and row.get("state") == "measured"
                for row in rows
            ):
                violations.add("coverage_regression")
