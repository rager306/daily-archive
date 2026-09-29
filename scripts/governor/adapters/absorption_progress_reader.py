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
  resolved from the artifact + pointer it names, so its number does not exist;
* ``census_projection_mismatch`` (S02/T03) — the declared per-document
  disposition projection, its digest, its partition or its source authority is
  not what the two frozen census sources derive, so the counts may look right
  while the population they describe is not the one on disk;
* ``census_document_dropped`` (S02/T03) — an identity the frozen sources assign
  to the population is absent from the declared projection;
* ``census_disposition_untyped`` (S02/T03) — a declared or derived exclusion
  reason is outside the contract's closed reason vocabulary;
* ``refused_set_unbound`` (S02/T03) — the declared refusal is not the explicitly
  enumerated set the frozen sources derive (or names an identity outside the
  assigned population), so a document silently crossed the refusal boundary.

Frozen thresholds (M514/S02/T02): the contract fixes them as a prospective
regression floor over the frozen corpus (S01 already measured it) and binds them
to the carrier revision. This adapter resolves each threshold's `reference` (path
+ pointer) to a number with the domain's own ``score_number`` rule; the reference
is a separately pinned artifact derived from the frozen probe, never the candidate
scorecard, so resolution does not take the candidate as an input and a restated
candidate score cannot move its own floor. It publishes the resolved values in
observations, annotates measured rows that publish no threshold of their own, and
recomputes the schema sha256 so the freeze is checked against bytes rather than
trusted.

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
    "CENSUS_OBSERVATION_KEY",
    "FilesystemAbsorptionProgressReader",
    "absorption_progress_violations",
    "effective_progress_contract",
    "missing_progress_vocabulary",
    "projection_digest",
]

#: A bound byte hash and a bound revision, as declared by the evidence contract.
_SHA256_RE = re.compile(r"^sha256:[0-9a-f]{64}$")

#: Observation key under which the reader publishes the per-document census it
#: recomputed from the two frozen sources, and the oracle consumes it.
CENSUS_OBSERVATION_KEY = "population_census"

#: The disposition strings of the census projection. An exclusion carries its
#: typed reason as a suffix, so an untyped reason is visible in the digest and
#: comparable to the contract's closed reason vocabulary.
_EXECUTED_DISPOSITION = "executed"
_REFUSED_DISPOSITION = "refused"
_EXCLUDED_DISPOSITION_PREFIX = "excluded:"

#: The census partition components. The refused and excluded halves stay inside
#: the denominator, so the four counts are the whole assigned population.
_CENSUS_COMPONENTS = ("assigned", "executed", "refused", "excluded")

#: Census source keys, in the order the contract names them.
_CENSUS_SOURCE_KEYS = ("probe", "manifest")

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


def _census_projection_pairs(value: Any) -> Optional[tuple[tuple[str, str], ...]]:
    """The sorted ``(document_id, disposition)`` pairs of a census projection.

    ``None`` means the value is not a non-empty list of two-string pairs, so a
    malformed projection is reported rather than silently read as empty (an empty
    projection would make every document look dropped).
    """
    if not isinstance(value, (list, tuple)) or not value:
        return None
    pairs: list[tuple[str, str]] = []
    for entry in value:
        if not isinstance(entry, (list, tuple)) or len(entry) != 2:
            return None
        document_id = _text(entry[0])
        disposition = _text(entry[1])
        if document_id is None or disposition is None:
            return None
        pairs.append((document_id, disposition))
    return tuple(pairs)


def projection_digest(projection: Any) -> Optional[str]:
    """``sha256:<64 hex>`` over the canonical encoding of a census projection.

    Canonical means: the pairs as two-element arrays, in the order given (the
    projection is already sorted by document id), encoded as compact UTF-8 JSON
    with ``,``/``:`` separators. A reviewer can reproduce the digest with
    ``sha256(json.dumps(projection, separators=(",", ":")).encode())``, and the
    artifact stores the same rule so the two cannot drift silently.
    """
    pairs = _census_projection_pairs(projection)
    if pairs is None:
        return None
    blob = json.dumps(
        [list(pair) for pair in pairs], ensure_ascii=False, separators=(",", ":")
    ).encode("utf-8")
    return f"sha256:{hashlib.sha256(blob).hexdigest()}"


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

    def read_yaml(self, relative_path: str) -> Optional[dict[str, Any]]:
        """A repository-relative YAML mapping, or ``None`` when unreadable."""
        path = self._resolve(relative_path)
        if path is None or not path.is_file():
            return None
        try:
            document = yaml.safe_load(path.read_text(encoding="utf-8"))
        except (OSError, yaml.YAMLError, UnicodeDecodeError):
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

    # ── the per-document census (S02/T03) ──────────────────────────────────
    def _census_source_paths(self, contract: Any) -> tuple[Optional[str], Optional[str]]:
        """The two frozen census sources, named by the contract.

        Deliberately read from the contract (DATA) and never from the scorecard:
        a scorecard that could point its own census check at a convenient artifact
        would make the derivation trust the thing it is supposed to bind.
        """
        if not isinstance(contract, dict):
            return None, None
        manifest = contract.get("assigned_population")
        manifest_path = None
        if isinstance(manifest, dict) and isinstance(manifest.get("manifest"), dict):
            manifest_path = _text(manifest["manifest"].get("path"))
        census = contract.get("denominator")
        probe_path = None
        if isinstance(census, dict) and isinstance(census.get("initial_census"), dict):
            probe_path = _text(census["initial_census"].get("authority"))
        return probe_path, manifest_path

    def recompute_population_census(self, contract: Any) -> Optional[dict[str, Any]]:
        """Recompute the 610-document disposition projection from the sources.

        The assigned identities come from the holdout manifest, the executed ones
        from the probe's per-document evaluated population (`outcomes`, matched by
        ``document_id``), the excluded ones from the probe's typed exceptions, and
        a refused document is one that is assigned but neither executed nor
        excluded. The returned mapping is a *derived* observation: nothing here
        trusts the scorecard, and the caller publishes it for the oracle to compare
        against what the scorecard declares.

        ``None`` means a source is missing, unparseable, or does not carry the
        list-shaped identities the derivation needs. An unverified census stays
        absent rather than becoming a fabricated one; the oracle then leaves the
        per-document half unverified instead of assuming it.
        """
        probe_path, manifest_path = self._census_source_paths(contract)
        if probe_path is None or manifest_path is None:
            return None
        manifest = self.read_yaml(manifest_path)
        probe = self.read_json(probe_path)
        if not isinstance(manifest, dict) or not isinstance(probe, dict):
            return None

        articles = manifest.get("articles")
        outcomes = probe.get("outcomes")
        reference = probe.get("reference_population")
        exceptions = reference.get("exceptions") if isinstance(reference, dict) else None
        if not isinstance(articles, list) or not isinstance(outcomes, list):
            return None
        if not isinstance(exceptions, list):
            return None

        assigned_ids = [
            _text(entry.get("article_id"))
            for entry in articles
            if isinstance(entry, dict)
        ]
        assigned_ids = [value for value in assigned_ids if value is not None]
        executed_ids = [
            _text(entry.get("document_id"))
            for entry in outcomes
            if isinstance(entry, dict)
        ]
        executed_ids = [value for value in executed_ids if value is not None]
        excluded_pairs: list[tuple[str, str]] = []
        for entry in exceptions:
            if not isinstance(entry, dict):
                continue
            document_id = _text(entry.get("document_id"))
            reason = _text(entry.get("reason"))
            if document_id is not None and reason is not None:
                excluded_pairs.append((document_id, reason))

        assigned_set = set(assigned_ids)
        executed_set = set(executed_ids)
        excluded_map = dict(excluded_pairs)
        refused_set = assigned_set - executed_set - set(excluded_map)

        projection = (
            [[document_id, _EXECUTED_DISPOSITION] for document_id in executed_set]
            + [
                [document_id, _EXCLUDED_DISPOSITION_PREFIX + excluded_map[document_id]]
                for document_id in excluded_map
            ]
            + [[document_id, _REFUSED_DISPOSITION] for document_id in refused_set]
        )
        projection.sort(key=lambda pair: pair[0])
        digest = projection_digest(projection)
        if digest is None:  # pragma: no cover - the projection is non-empty by construction
            return None

        reason_counts: dict[str, int] = {}
        for reason in excluded_map.values():
            reason_counts[reason] = reason_counts.get(reason, 0) + 1

        evaluated = probe.get("evaluated_population")
        executed_population = probe.get("executed_population")
        return {
            "available": True,
            "census_id": _text(
                (contract.get("denominator") or {}).get("initial_census", {}).get("census_id")
                if isinstance(contract.get("denominator"), dict)
                else None
            ),
            "match_by": "document_id",
            "sources": {
                "probe": {"path": probe_path, "sha256": self.file_sha256(probe_path)},
                "manifest": {"path": manifest_path, "sha256": self.file_sha256(manifest_path)},
            },
            "partition": {
                "assigned": len(assigned_set),
                "executed": len(executed_set),
                "refused": len(refused_set),
                "excluded": len(excluded_map),
            },
            "refused_document_ids": sorted(refused_set),
            "exclusion_reasons": reason_counts,
            "projection": projection,
            "projection_sha256": digest,
            "list_counts": {
                "manifest_articles": len(assigned_ids),
                "probe_outcomes": len(executed_ids),
                "probe_exceptions": len(excluded_pairs),
            },
            "declared_counts": {
                "evaluated_documents": _count(
                    evaluated.get("documents") if isinstance(evaluated, dict) else None
                ),
                "executed_documents": _count(
                    executed_population.get("documents")
                    if isinstance(executed_population, dict)
                    else None
                ),
                "reference_documents": _count(
                    reference.get("documents") if isinstance(reference, dict) else None
                ),
                "reference_measurable": _count(
                    reference.get("measurable_documents") if isinstance(reference, dict) else None
                ),
                "reference_excluded": _count(
                    reference.get("excluded_documents") if isinstance(reference, dict) else None
                ),
            },
        }

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


def _text_list(value: Any) -> list[str]:
    """The non-empty strings inside a list-like value (possibly empty)."""
    if not isinstance(value, (list, tuple, set)):
        return []
    return [item for item in value if isinstance(item, str) and item.strip()]


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
    _population_census_violations(scorecard, contract, observations, violations)
    if _scope_accepted(rows, contract):
        _regression_violations(scorecard, rows, contract, violations)
    return violations


def _census_reason_vocabulary(contract: Any) -> set[str]:
    """The closed exclusion-reason vocabulary the contract pins."""
    if not isinstance(contract, dict):
        return set()
    reasons = (contract.get("denominator") or {}).get("exclusion_reasons") or {}
    values = reasons.get("values") if isinstance(reasons, dict) else None
    if not isinstance(values, (list, tuple, set)):
        return set()
    return {value for value in values if isinstance(value, str) and value.strip()}


def _population_census_violations(
    scorecard: dict[str, Any],
    contract: dict[str, Any],
    observations: dict[str, Any],
    violations: set[str],
) -> None:
    """Per-document census violations the domain cannot see (it cannot read files).

    The reader recomputes the disposition projection from the two frozen sources
    (the holdout manifest and the release probe) and publishes it under
    ``observations["population_census"]``. This compares that derivation with what
    the scorecard declares, so a dropped document or a swapped disposition cannot
    leave the aggregate counts looking correct:

    * ``census_projection_mismatch`` — no declared block or projection, a digest
      that is not the recomputed digest, a partition that is not the recomputed
      partition, an authority that does not name the recomputed source bytes, a
      declared identity the sources do not assign, a declared census id other than
      the pinned one, or a probe/manifest whose own aggregate counts disagree with
      its lists;
    * ``census_document_dropped`` — an identity the sources assign is absent from
      the declared projection;
    * ``census_disposition_untyped`` — an exclusion reason (declared or derived)
      outside the contract's closed vocabulary;
    * ``refused_set_unbound`` — the refused set is not explicitly enumerated, is
      not the set the sources derive, or names an identity outside the assigned
      population.

    When the reader supplied no observation the half is left unverified rather
    than assumed: the aggregate partition stays governed by the domain's
    denominator and census checks.
    """
    observed = observations.get(CENSUS_OBSERVATION_KEY)
    if not isinstance(observed, dict) or observed.get("available") is not True:
        return
    declared = scorecard.get("population_census")
    if not isinstance(declared, dict):
        violations.add("census_projection_mismatch")
        return

    projection_mismatch = False
    derived_projection = _census_projection_pairs(observed.get("projection"))
    declared_projection = _census_projection_pairs(declared.get("projection"))
    declared_digest = _text(declared.get("projection_sha256"))
    derived_digest = _text(observed.get("projection_sha256"))
    if (
        derived_projection is None
        or declared_projection is None
        or declared_digest is None
        or derived_digest is None
        or declared_digest != derived_digest
    ):
        projection_mismatch = True
    if derived_projection is not None and declared_projection is not None:
        derived_map = dict(derived_projection)
        declared_map = dict(declared_projection)
        if derived_map != declared_map:
            projection_mismatch = True
        if set(derived_map) - set(declared_map):
            violations.add("census_document_dropped")
        if set(declared_map) - set(derived_map):
            projection_mismatch = True

    derived_partition = observed.get("partition")
    declared_partition = declared.get("partition")
    if not isinstance(derived_partition, dict) or not isinstance(declared_partition, dict):
        projection_mismatch = True
    else:
        for component in _CENSUS_COMPONENTS:
            if _count(declared_partition.get(component)) != _count(
                derived_partition.get(component)
            ):
                projection_mismatch = True
        if _text(declared.get("identity")) is None:
            projection_mismatch = True

    # The declared census must be the one the contract pins and the derivation
    # reports, so a block copied from another census is refused.
    observed_census_id = _text(observed.get("census_id"))
    declared_census_id = _text(declared.get("census_id"))
    if observed_census_id is None or declared_census_id != observed_census_id:
        projection_mismatch = True
    scorecard_census_id = _text(scorecard.get("census_id"))
    if scorecard_census_id is not None and declared_census_id != scorecard_census_id:
        projection_mismatch = True

    # The authority block must name the bytes the reader actually derived from.
    authority = declared.get("authority")
    sources = observed.get("sources")
    for source_key in _CENSUS_SOURCE_KEYS:
        declared_source = authority.get(source_key) if isinstance(authority, dict) else None
        derived_source = sources.get(source_key) if isinstance(sources, dict) else None
        if not isinstance(declared_source, dict) or not isinstance(derived_source, dict):
            projection_mismatch = True
            continue
        if _text(declared_source.get("path")) != _text(derived_source.get("path")):
            projection_mismatch = True
        declared_sha = _text(declared_source.get("sha256"))
        derived_sha = _text(derived_source.get("sha256"))
        if declared_sha is None or derived_sha is None or declared_sha != derived_sha:
            projection_mismatch = True

    # The probe's own aggregate counts must agree with the lists they summarize,
    # or the derivation would be choosing one of two disagreeing sources.
    list_counts = observed.get("list_counts")
    declared_counts = observed.get("declared_counts")
    if not isinstance(list_counts, dict) or not isinstance(declared_counts, dict):
        projection_mismatch = True
    elif isinstance(derived_partition, dict):
        for key, component in (
            ("manifest_articles", "assigned"),
            ("probe_outcomes", "executed"),
            ("probe_exceptions", "excluded"),
        ):
            if _count(list_counts.get(key)) != _count(derived_partition.get(component)):
                projection_mismatch = True
        for key in (
            "evaluated_documents",
            "executed_documents",
            "reference_documents",
            "reference_measurable",
            "reference_excluded",
        ):
            expected = _count(declared_counts.get(key))
            component = {
                "evaluated_documents": "executed",
                "executed_documents": "executed",
                "reference_documents": "assigned",
                "reference_measurable": "executed",
                "reference_excluded": "excluded",
            }[key]
            if expected is None or expected != _count(derived_partition.get(component)):
                projection_mismatch = True

    derivation = declared.get("derivation")
    if not isinstance(derivation, dict) or _text(derivation.get("match_by")) is None:
        projection_mismatch = True

    if projection_mismatch:
        violations.add("census_projection_mismatch")

    # The refusal boundary: an explicitly enumerated set, not a residual count.
    derived_refused = sorted(_text_list(observed.get("refused_document_ids")))
    semantics = declared.get("refusal_semantics")
    if not isinstance(semantics, dict) or semantics.get("closed") is not True:
        violations.add("refused_set_unbound")
    else:
        declared_refused = sorted(_text_list(semantics.get("refused_document_ids")))
        if declared_refused != derived_refused:
            violations.add("refused_set_unbound")
        assigned_ids = set(dict(derived_projection or ()))
        if assigned_ids and any(
            document_id not in assigned_ids for document_id in declared_refused
        ):
            violations.add("refused_set_unbound")
        declared_refused_count = (
            _count(declared_partition.get("refused"))
            if isinstance(declared_partition, dict)
            else None
        )
        if declared_refused_count != len(derived_refused):
            violations.add("refused_set_unbound")

    # Typed exclusions: every reason (declared and derived) must be in the closed
    # vocabulary, and the declared counts must be the derived counts.
    vocabulary = _census_reason_vocabulary(contract)
    derived_reasons = observed.get("exclusion_reasons")
    reason_block = declared.get("exclusion_reasons")
    declared_reasons = reason_block.get("values") if isinstance(reason_block, dict) else None
    if (
        not isinstance(reason_block, dict)
        or reason_block.get("closed") is not True
        or not isinstance(declared_reasons, dict)
        or not isinstance(derived_reasons, dict)
    ):
        violations.add("census_projection_mismatch")
    else:
        for reason in set(declared_reasons) | set(derived_reasons):
            if vocabulary and reason not in vocabulary:
                violations.add("census_disposition_untyped")
            if _count(declared_reasons.get(reason)) != _count(derived_reasons.get(reason)):
                violations.add("census_projection_mismatch")


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
