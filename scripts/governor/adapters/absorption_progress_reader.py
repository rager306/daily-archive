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
    ACCEPTANCE_PACKAGE_VIOLATION_LABELS,
    JAVA_FREE_SLICE_VIOLATION_LABELS,
    NATIVE_CONTENT_VIOLATION_LABELS,
    NATIVE_FAMILY_VIOLATION_LABELS,
    NATIVE_ROUTE_VIOLATION_LABELS,
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
    "java_free_slice",
    "native_family_receipt",
    "native_content_receipt",
    "native_route_e2e",
    "acceptance_package",
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

    # ── the Java-free vertical slice claim (S03/T06) ───────────────────────
    def read_java_free_slice(self, contract: Any) -> Optional[dict[str, Any]]:
        """Resolve the Java-free slice claim, its receipt and its pinned sources.

        The claim is not this rule: it is the ``java_free_vertical_slice`` block in
        the layout-bakeoff DATA (ADR-077 §3 makes that block the sole authority),
        and this rule only names where to find it and what a receipt for it must
        look like. Nothing here restates the stage ids or the document identities —
        both are followed by pointer, so the rule cannot drift from the thing it
        governs.

        ``None`` means the rule does not govern a Java-free slice half. Otherwise a
        bundle carrying the resolved declaration, the receipt bytes and both
        recomputed source hashes, the resolved cascade stage ids and the pinned
        corpus documents. ``available`` is True only when the declaration manifest
        is readable and the declared pointer resolves to a mapping: a tree that
        simply does not carry the claim is *absence*, not a violation, so the check
        stays silent there and the layout-bakeoff contract keeps owning that
        artifact. An unreadable *receipt* under a resolvable declaration is not
        absence — it is a claim with no evidence — and the oracle reports it.
        """
        if not isinstance(contract, dict):
            return None
        rule = contract.get("java_free_slice")
        if not isinstance(rule, dict):
            return None

        declaration_rule = rule.get("declaration")
        declaration_rule = declaration_rule if isinstance(declaration_rule, dict) else {}
        declaration_manifest = _text(declaration_rule.get("manifest"))
        declaration_pointer = _text(declaration_rule.get("pointer"))

        receipt_rule = rule.get("receipt")
        receipt_rule = receipt_rule if isinstance(receipt_rule, dict) else {}
        receipt_path = _text(receipt_rule.get("path"))

        stage_rule = rule.get("stage_id_source")
        stage_rule = stage_rule if isinstance(stage_rule, dict) else {}
        stage_manifest = _text(stage_rule.get("manifest"))
        stage_pointer = _text(stage_rule.get("pointer"))

        corpus_rule = rule.get("corpus")
        corpus_rule = corpus_rule if isinstance(corpus_rule, dict) else {}
        corpus_manifest = _text(corpus_rule.get("manifest"))
        corpus_pointer = _text(corpus_rule.get("pointer"))

        bundle: dict[str, Any] = {
            "available": False,
            "declaration_manifest": declaration_manifest,
            "declaration_pointer": declaration_pointer,
            "declaration_manifest_present": bool(
                declaration_manifest and self.path_exists(declaration_manifest)
            ),
            "declaration_sha256": (
                self.file_sha256(declaration_manifest) if declaration_manifest else None
            ),
            "declaration": None,
            "receipt_path": receipt_path,
            "receipt_exists": bool(receipt_path and self.path_exists(receipt_path)),
            "receipt_sha256": self.file_sha256(receipt_path) if receipt_path else None,
            "receipt": None,
            "stage_id_source_manifest": stage_manifest,
            "stage_id_source_pointer": stage_pointer,
            "stage_id_source_sha256": (
                self.file_sha256(stage_manifest) if stage_manifest else None
            ),
            "cascade_stage_ids": [],
            "corpus_manifest": corpus_manifest,
            "corpus_pointer": corpus_pointer,
            "corpus_documents": [],
            "errors": [],
        }

        manifest = self.read_yaml(declaration_manifest) if declaration_manifest else None
        if not isinstance(manifest, dict):
            bundle["errors"].append("declaration_manifest_unreadable")
        else:
            block = _resolve_declared_pointer(manifest, declaration_pointer)
            if not isinstance(block, dict):
                bundle["errors"].append("declaration_pointer_unresolved")
            else:
                bundle["declaration"] = block
                bundle["available"] = True

        bundle["receipt"] = self.read_json(receipt_path) if receipt_path else None
        if bundle["receipt"] is None and bundle["receipt_exists"]:
            bundle["errors"].append("receipt_unreadable")

        if stage_manifest:
            stage_document = self.read_yaml(stage_manifest)
            if not isinstance(stage_document, dict):
                bundle["errors"].append("stage_id_source_unreadable")
            else:
                stages = _resolve_declared_pointer(stage_document, stage_pointer)
                if isinstance(stages, dict):
                    bundle["cascade_stage_ids"] = sorted(
                        key for key in stages if isinstance(key, str) and key.strip()
                    )
                elif isinstance(stages, list):
                    bundle["cascade_stage_ids"] = sorted(
                        stage_id
                        for stage_id in (
                            _text(entry.get("stage_id"))
                            for entry in stages
                            if isinstance(entry, dict)
                        )
                        if stage_id is not None
                    )
                else:
                    bundle["errors"].append("stage_id_source_pointer_unresolved")

        if corpus_manifest:
            corpus_document = self.read_yaml(corpus_manifest)
            if not isinstance(corpus_document, dict):
                bundle["errors"].append("corpus_source_unreadable")
            else:
                documents = _resolve_declared_pointer(corpus_document, corpus_pointer)
                if isinstance(documents, list):
                    bundle["corpus_documents"] = [
                        {
                            "id": _text(entry.get("id")),
                            "sha256": _text(entry.get("sha256")),
                        }
                        for entry in documents
                        if isinstance(entry, dict)
                    ]
                else:
                    bundle["errors"].append("corpus_source_pointer_unresolved")

        return bundle

    # ── the native family receipts (S04/T06) ───────────────────────────────
    def read_native_family_receipt(self, contract: Any) -> Optional[dict[str, Any]]:
        """Resolve the native family receipts, their carriers and the corpus pin.

        The half this governs is the S04 evidence that the real native parsers —
        not the Java carriers — produced header and bibliography atoms on the
        declared 3-document slice, and that the 79 typed baseline exclusions keep
        their denominator. The rule names *where* the two receipts live, where
        each frozen carrier lives and which pointer holds the corpus / denominator
        / preserved-TEI pins; nothing about the receipt contents is restated in
        the rule, so the rule cannot drift from the artifacts it governs.

        ``None`` means the rule does not govern a native family half. Otherwise a
        bundle carrying the two receipt documents with their recomputed
        ``sha256``, the frozen carriers with their recomputed ``sha256``, and the
        derived views the oracle compares against (the corpus document pin, the
        frozen baseline denominator and the preserved-TEI pins). ``available`` is
        True only when the S03 declaration carrier resolves — the same authority
        the Java-free slice half reads — so a tree that simply does not carry that
        authority is *absence*, not a violation. An unreadable *receipt* under a
        resolvable declaration is not absence — it is a claim with no evidence —
        and the oracle reports it.
        """
        if not isinstance(contract, dict):
            return None
        rule = contract.get("native_family_receipt")
        if not isinstance(rule, dict):
            return None

        declaration_rule = rule.get("declaration")
        declaration_rule = declaration_rule if isinstance(declaration_rule, dict) else {}
        declaration_manifest = _text(declaration_rule.get("manifest"))
        declaration_pointer = _text(declaration_rule.get("pointer"))

        bundle: dict[str, Any] = {
            "available": False,
            "declaration_manifest": declaration_manifest,
            "declaration_pointer": declaration_pointer,
            "declaration_manifest_present": bool(
                declaration_manifest and self.path_exists(declaration_manifest)
            ),
            "declaration_sha256": (
                self.file_sha256(declaration_manifest) if declaration_manifest else None
            ),
            "declaration": None,
            "receipts": {},
            "carriers": {},
            "corpus_documents": None,
            "corpus_page_denominator": None,
            "baseline_denominator": None,
            "preserved_tei_pins": {},
            "errors": [],
        }

        manifest = self.read_yaml(declaration_manifest) if declaration_manifest else None
        if not isinstance(manifest, dict):
            bundle["errors"].append("declaration_manifest_unreadable")
        else:
            block = _resolve_declared_pointer(manifest, declaration_pointer)
            if not isinstance(block, dict):
                bundle["errors"].append("declaration_pointer_unresolved")
            else:
                bundle["declaration"] = block
                bundle["available"] = True

        receipt_rules = rule.get("receipts")
        if isinstance(receipt_rules, dict):
            for receipt_id, spec in receipt_rules.items():
                spec = spec if isinstance(spec, dict) else {}
                rel = _text(spec.get("path"))
                document = None
                if rel is not None:
                    document = (
                        self.read_json(rel)
                        if rel.lower().endswith(".json")
                        else self.read_yaml(rel)
                    )
                bundle["receipts"][receipt_id] = {
                    "path": rel,
                    "required": spec.get("required") is True,
                    "schema_version": _text(spec.get("schema_version")),
                    "receipt_kind": _text(spec.get("receipt_kind")),
                    "exists": bool(rel and self.path_exists(rel)),
                    "sha256": self.file_sha256(rel) if rel else None,
                    "document": document,
                }

        carrier_rules = rule.get("carriers")
        if isinstance(carrier_rules, list):
            for entry in carrier_rules:
                if not isinstance(entry, dict):
                    continue
                carrier_id = _text(entry.get("id"))
                if carrier_id is None:
                    continue
                rel = _text(entry.get("path"))
                document = None
                if rel is not None:
                    document = (
                        self.read_json(rel)
                        if rel.lower().endswith(".json")
                        else self.read_yaml(rel)
                    )
                bundle["carriers"][carrier_id] = {
                    "id": carrier_id,
                    "path": rel,
                    "pointer": _text(entry.get("pointer")),
                    "kind": _text(entry.get("kind")),
                    "sha256": self.file_sha256(rel) if rel else None,
                    "document": document,
                }

        def _carrier(carrier_id: str) -> dict[str, Any]:
            entry = bundle["carriers"].get(carrier_id)
            return entry if isinstance(entry, dict) else {}

        layout = _carrier("layout_bakeoff")
        if isinstance(layout.get("document"), dict):
            resolved = _resolve_declared_pointer(layout["document"], layout.get("pointer"))
            bundle["corpus_documents"] = resolved if isinstance(resolved, list) else None
            corpus_block = layout["document"].get("corpus")
            if isinstance(corpus_block, dict):
                bundle["corpus_page_denominator"] = _count(corpus_block.get("page_denominator"))

        baseline = _carrier("absorption_progress_baseline")
        if isinstance(baseline.get("document"), dict):
            resolved = _resolve_declared_pointer(baseline["document"], baseline.get("pointer"))
            bundle["baseline_denominator"] = resolved if isinstance(resolved, dict) else None

        provenance = _carrier("preserved_tei_provenance")
        pins: dict[str, dict[str, Any]] = {}
        if isinstance(provenance.get("document"), dict):
            resolved = _resolve_declared_pointer(provenance["document"], provenance.get("pointer"))
            if isinstance(resolved, list):
                for entry in resolved:
                    if not isinstance(entry, dict):
                        continue
                    path = _text(entry.get("path"))
                    if path is None:
                        continue
                    pins[path] = {
                        "id": _text(entry.get("id")),
                        "sha256": _text(entry.get("sha256")),
                        "recomputed_sha256": self.file_sha256(path),
                    }
        bundle["preserved_tei_pins"] = pins

        return bundle

    # ── the native content receipt (S05/T07) ───────────────────────────
    def read_native_content_receipt(self, contract: Any) -> Optional[dict[str, Any]]:
        """Resolve the S05 content receipt, its carriers and the derived pins.

        The half this governs is the S05 evidence that the native body/section/
        reading-order/callout/visual bands measured the declared 3-document slice
        against an *independent* oracle, with a total group partition and a typed
        per-family matrix. The rule names only *where* the receipt lives, where the
        declaration authority and the oracle pins live, and what a receipt must
        declare; nothing about the receipt contents is restated in the rule, so the
        rule cannot drift from the artifact it governs.

        ``None`` means the rule does not govern a native content half. Otherwise a
        bundle carrying the receipt document with its recomputed ``sha256``, the
        recomputed declaration / oracle / segmentation carrier hashes, the derived
        corpus pin and page denominator, and the preserved-TEI fixture pins with
        their recomputed bytes. ``available`` is True only when the S03 declaration
        carrier resolves -- the same authority the Java-free and native-family
        halves read -- so a tree that simply does not carry that authority is
        *absence*, not a violation. An unreadable *receipt* under a resolvable
        declaration is not absence -- it is a claim with no evidence -- and the
        oracle reports it.
        """
        if not isinstance(contract, dict):
            return None
        rule = contract.get("native_content_receipt")
        if not isinstance(rule, dict):
            return None

        declaration_rule = rule.get("declaration")
        declaration_rule = declaration_rule if isinstance(declaration_rule, dict) else {}
        declaration_manifest = _text(declaration_rule.get("manifest"))
        declaration_pointer = _text(declaration_rule.get("pointer"))

        receipt_rule = rule.get("receipt")
        receipt_rule = receipt_rule if isinstance(receipt_rule, dict) else {}
        receipt_path = _text(receipt_rule.get("path"))

        corpus_rule = rule.get("corpus")
        corpus_rule = corpus_rule if isinstance(corpus_rule, dict) else {}
        corpus_manifest = _text(corpus_rule.get("manifest"))
        corpus_pointer = _text(corpus_rule.get("pointer"))

        oracle_rule = rule.get("oracle")
        oracle_rule = oracle_rule if isinstance(oracle_rule, dict) else {}
        oracle_manifest = _text(oracle_rule.get("manifest"))
        oracle_pointer = _text(oracle_rule.get("pointer"))
        segmentation_manifest = _text(oracle_rule.get("segmentation_manifest"))
        segmentation_source = _text(oracle_rule.get("segmentation_source"))

        bundle: dict[str, Any] = {
            "available": False,
            "declaration_manifest": declaration_manifest,
            "declaration_pointer": declaration_pointer,
            "declaration_manifest_present": bool(
                declaration_manifest and self.path_exists(declaration_manifest)
            ),
            "declaration_sha256": (
                self.file_sha256(declaration_manifest) if declaration_manifest else None
            ),
            "declaration": None,
            "receipt_path": receipt_path,
            "receipt_exists": bool(receipt_path and self.path_exists(receipt_path)),
            "receipt_sha256": self.file_sha256(receipt_path) if receipt_path else None,
            "receipt": None,
            "corpus_manifest": corpus_manifest,
            "corpus_pointer": corpus_pointer,
            "corpus_sha256": (
                self.file_sha256(corpus_manifest) if corpus_manifest else None
            ),
            "corpus_documents": None,
            "corpus_document_count": None,
            "corpus_page_denominator": None,
            "oracle_manifest": oracle_manifest,
            "oracle_pointer": oracle_pointer,
            "oracle_sha256": (
                self.file_sha256(oracle_manifest) if oracle_manifest else None
            ),
            "oracle_fixture_pins": {},
            "segmentation_manifest": segmentation_manifest,
            "segmentation_manifest_sha256": (
                self.file_sha256(segmentation_manifest) if segmentation_manifest else None
            ),
            "segmentation_source": segmentation_source,
            "segmentation_source_sha256": (
                self.file_sha256(segmentation_source) if segmentation_source else None
            ),
            "errors": [],
        }

        manifest = self.read_yaml(declaration_manifest) if declaration_manifest else None
        if not isinstance(manifest, dict):
            bundle["errors"].append("declaration_manifest_unreadable")
        else:
            block = _resolve_declared_pointer(manifest, declaration_pointer)
            if not isinstance(block, dict):
                bundle["errors"].append("declaration_pointer_unresolved")
            else:
                bundle["declaration"] = block
                bundle["available"] = True

        bundle["receipt"] = self.read_json(receipt_path) if receipt_path else None
        if bundle["receipt"] is None and bundle["receipt_exists"]:
            bundle["errors"].append("receipt_unreadable")

        if corpus_manifest:
            corpus_document = self.read_yaml(corpus_manifest)
            if not isinstance(corpus_document, dict):
                bundle["errors"].append("corpus_source_unreadable")
            else:
                documents = _resolve_declared_pointer(corpus_document, corpus_pointer)
                if isinstance(documents, list):
                    bundle["corpus_documents"] = [
                        {
                            "id": _text(entry.get("id")),
                            "sha256": _text(entry.get("sha256")),
                            "pages": _count(entry.get("pages")),
                        }
                        for entry in documents
                        if isinstance(entry, dict)
                    ]
                    bundle["corpus_document_count"] = len(bundle["corpus_documents"])
                else:
                    bundle["errors"].append("corpus_source_pointer_unresolved")
                corpus_block = corpus_document.get("corpus")
                if isinstance(corpus_block, dict):
                    declared_n = _count(corpus_block.get("corpus_n"))
                    if declared_n is not None:
                        bundle["corpus_document_count"] = declared_n
                    bundle["corpus_page_denominator"] = _count(
                        corpus_block.get("page_denominator")
                    )

        if oracle_manifest:
            provenance = self.read_yaml(oracle_manifest)
            if not isinstance(provenance, dict):
                bundle["errors"].append("oracle_source_unreadable")
            else:
                pins = _resolve_declared_pointer(provenance, oracle_pointer)
                if isinstance(pins, list):
                    fixtures: dict[str, dict[str, Any]] = {}
                    for entry in pins:
                        if not isinstance(entry, dict):
                            continue
                        path = _text(entry.get("path"))
                        if path is None:
                            continue
                        fixtures[path] = {
                            "id": _text(entry.get("id")),
                            "sha256": _text(entry.get("sha256")),
                            "recomputed_sha256": self.file_sha256(path),
                        }
                    bundle["oracle_fixture_pins"] = fixtures
                else:
                    bundle["errors"].append("oracle_source_pointer_unresolved")

        return bundle
    # ── the native route + same-n resource receipts (S06/T07) ──────────
    def _composite_freeze_revision(self, revision: Any) -> Optional[str]:
        """Recompute the T02 composite freeze revision from the pinned carriers.

        The rule declares the carriers (paths, in order) and a single
        ``line_format``; this recomputes ``sha256(join("<path>:<sha256-hex>",
        "\\n"))`` over their *committed bytes*. That is what makes the freeze a
        binding rather than a mention: a receipt that carries the revision is
        refused as stale the moment a carrier is edited, and no later reader has
        to trust the number it declared.

        ``None`` means the declaration is not shaped for that recomputation, so
        the caller leaves the revision unresolvable rather than fabricating one.
        """
        if not isinstance(revision, dict):
            return None
        carriers = revision.get("carriers")
        if not isinstance(carriers, list) or not carriers:
            return None
        lines: list[str] = []
        for carrier in carriers:
            if not isinstance(carrier, dict):
                return None
            relative = _text(carrier.get("path"))
            if relative is None or not _is_relative_path(relative):
                return None
            digest = self.file_sha256(relative)
            if digest is None:
                return None
            lines.append(f"{relative}:{digest.split('sha256:', 1)[-1]}")
        blob = "\n".join(lines).encode("utf-8")
        return f"sha256:{hashlib.sha256(blob).hexdigest()}"

    def _resolve_reference_pins(self, receipt: Any) -> dict[str, Any]:
        """Recompute every measured carrier an observed reference row names.

        The T06 receipt binds each observed reference number to a repository-
        relative carrier plus its ``sha256``. That pin is only a binding if it is
        checked against the committed bytes, so every distinct *measured* source
        named by the receipt is re-hashed here and published beside the number the
        receipt declared. A receipt that names a carrier it drifted from -- or a
        carrier that does not exist -- is then refused by the oracle rather than
        trusted because the row is shaped correctly.

        Only sources of *observed* rows are resolved: an absent row is a typed
        absence and names no carrier.
        """
        pins: dict[str, Any] = {}
        if not isinstance(receipt, dict):
            return pins

        rows: list[Any] = []
        populations = receipt.get("populations")
        if isinstance(populations, list):
            for population in populations:
                if not isinstance(population, dict):
                    continue
                metrics = population.get("metrics")
                if isinstance(metrics, list):
                    rows.extend(metrics)
        documents = receipt.get("documents")
        if isinstance(documents, list):
            for document in documents:
                if not isinstance(document, dict):
                    continue
                reference = document.get("reference")
                if isinstance(reference, list):
                    rows.extend(reference)

        for row in rows:
            if not isinstance(row, dict) or _text(row.get("state")) != "observed":
                continue
            source = row.get("source")
            if not isinstance(source, dict):
                continue
            manifest = _text(source.get("manifest"))
            if manifest is None or not _is_relative_path(manifest):
                continue
            pins[manifest] = {
                "declared_sha256": _text(source.get("sha256")),
                "declared_kind": _text(source.get("kind")),
                "recomputed_sha256": self.file_sha256(manifest),
            }
        return pins

    def read_native_route_e2e(self, contract: Any) -> Optional[dict[str, Any]]:
        """Resolve the S06 route receipt, the same-n resource receipt and the boundary.

        The half this governs is the S06 evidence that one PDF->native path ends in
        a typed document result on the declared 3-document slice with no Java and no
        prior TEI, measured against a limit freeze declared *before* any benchmark,
        plus the T06 same-n resource receipt that pairs that measurement with a
        pinned reference on the same documents. The rule names only *where* the two
        receipts live, where the declaration / cascade / corpus carriers live and
        which frozen limit dictionary the receipts must mirror; nothing about the
        receipt contents is restated in the rule, so the rule cannot drift from the
        artifacts it governs.

        ``None`` means the rule does not govern a native-route half. Otherwise a
        bundle carrying both receipt documents with their recomputed ``sha256``,
        the recomputed declaration / stage-carrier / corpus hashes, the derived
        corpus pin and page denominator, the declaration's own slice boundary
        (in-slice stages, out-of-slice stages, admission stage and the out-of-slice
        reason vocabulary) and the composite freeze revision recomputed from the
        T02 carriers. ``available`` is True only when the S03 declaration carrier
        resolves -- the same authority the Java-free, native-family and
        native-content halves read -- so a tree that simply does not carry that
        authority is *absence*, not a violation. An unreadable *receipt* under a
        resolvable declaration is not absence -- it is a claim with no evidence --
        and the oracle reports it.
        """
        if not isinstance(contract, dict):
            return None
        rule = contract.get("native_route_e2e")
        if not isinstance(rule, dict):
            return None

        declaration_rule = rule.get("declaration")
        declaration_rule = declaration_rule if isinstance(declaration_rule, dict) else {}
        declaration_manifest = _text(declaration_rule.get("manifest"))
        declaration_pointer = _text(declaration_rule.get("pointer"))

        stage_source_rule = rule.get("stage_id_source")
        stage_source_rule = stage_source_rule if isinstance(stage_source_rule, dict) else {}
        stage_source_manifest = _text(stage_source_rule.get("manifest"))
        stage_source_pointer = _text(stage_source_rule.get("pointer"))

        receipt_rule = rule.get("receipt")
        receipt_rule = receipt_rule if isinstance(receipt_rule, dict) else {}
        receipt_path = _text(receipt_rule.get("path"))

        resource_rule = rule.get("resource_receipt")
        resource_rule = resource_rule if isinstance(resource_rule, dict) else {}
        resource_path = _text(resource_rule.get("path"))

        corpus_rule = rule.get("corpus")
        corpus_rule = corpus_rule if isinstance(corpus_rule, dict) else {}
        corpus_manifest = _text(corpus_rule.get("manifest"))
        corpus_pointer = _text(corpus_rule.get("pointer"))

        limit_policy = rule.get("limit_policy")
        limit_policy = limit_policy if isinstance(limit_policy, dict) else {}

        bundle: dict[str, Any] = {
            "available": False,
            "declaration_manifest": declaration_manifest,
            "declaration_pointer": declaration_pointer,
            "declaration_manifest_present": bool(
                declaration_manifest and self.path_exists(declaration_manifest)
            ),
            "declaration_sha256": (
                self.file_sha256(declaration_manifest) if declaration_manifest else None
            ),
            "declaration": None,
            "stages_in_slice": [],
            "stages_out_of_slice": [],
            "admission_stage": None,
            "out_of_slice_reasons": [],
            "receipt_path": receipt_path,
            "receipt_exists": bool(receipt_path and self.path_exists(receipt_path)),
            "receipt_sha256": self.file_sha256(receipt_path) if receipt_path else None,
            "receipt": None,
            "resource_receipt_path": resource_path,
            "resource_receipt_exists": bool(
                resource_path and self.path_exists(resource_path)
            ),
            "resource_receipt_sha256": (
                self.file_sha256(resource_path) if resource_path else None
            ),
            "resource_receipt": None,
            "resource_reference_pins": {},
            "stage_source_manifest": stage_source_manifest,
            "stage_source_pointer": stage_source_pointer,
            "stage_source_sha256": (
                self.file_sha256(stage_source_manifest) if stage_source_manifest else None
            ),
            "stage_ids": [],
            "corpus_manifest": corpus_manifest,
            "corpus_pointer": corpus_pointer,
            "corpus_sha256": (
                self.file_sha256(corpus_manifest) if corpus_manifest else None
            ),
            "corpus_documents": None,
            "corpus_document_count": None,
            "corpus_page_denominator": None,
            "freeze_revision_recomputed": self._composite_freeze_revision(
                limit_policy.get("freeze_revision")
            ),
            "freeze_metric_ids": _text_list(limit_policy.get("metrics")),
            "errors": [],
        }

        manifest = self.read_yaml(declaration_manifest) if declaration_manifest else None
        if not isinstance(manifest, dict):
            bundle["errors"].append("declaration_manifest_unreadable")
        else:
            block = _resolve_declared_pointer(manifest, declaration_pointer)
            if not isinstance(block, dict):
                bundle["errors"].append("declaration_pointer_unresolved")
            else:
                bundle["declaration"] = block
                bundle["available"] = True
                bundle["stages_in_slice"] = [
                    _text(entry.get("stage_id"))
                    for entry in block.get("stages_in_slice") or []
                    if isinstance(entry, dict) and _text(entry.get("stage_id"))
                ]
                bundle["stages_out_of_slice"] = [
                    _text(entry.get("stage_id"))
                    for entry in block.get("stages_out_of_slice") or []
                    if isinstance(entry, dict) and _text(entry.get("stage_id"))
                ]
                admission = block.get("identity_admission_stage")
                if isinstance(admission, dict):
                    bundle["admission_stage"] = _text(admission.get("stage_id"))
                bundle["out_of_slice_reasons"] = _text_list(
                    block.get("out_of_slice_reason_vocabulary")
                )

        bundle["receipt"] = self.read_json(receipt_path) if receipt_path else None
        if bundle["receipt"] is None and bundle["receipt_exists"]:
            bundle["errors"].append("receipt_unreadable")
        bundle["resource_receipt"] = (
            self.read_json(resource_path) if resource_path else None
        )
        if bundle["resource_receipt"] is None and bundle["resource_receipt_exists"]:
            bundle["errors"].append("resource_receipt_unreadable")
        bundle["resource_reference_pins"] = self._resolve_reference_pins(
            bundle["resource_receipt"]
        )

        if stage_source_manifest:
            compatibility = self.read_yaml(stage_source_manifest)
            if not isinstance(compatibility, dict):
                bundle["errors"].append("stage_source_unreadable")
            else:
                cascade = _resolve_declared_pointer(compatibility, stage_source_pointer)
                if isinstance(cascade, dict):
                    bundle["stage_ids"] = [str(key) for key in cascade]
                else:
                    bundle["errors"].append("stage_source_pointer_unresolved")

        if corpus_manifest:
            corpus_document = self.read_yaml(corpus_manifest)
            if not isinstance(corpus_document, dict):
                bundle["errors"].append("corpus_source_unreadable")
            else:
                documents = _resolve_declared_pointer(corpus_document, corpus_pointer)
                if isinstance(documents, list):
                    bundle["corpus_documents"] = [
                        {
                            "id": _text(entry.get("id")),
                            "sha256": _text(entry.get("sha256")),
                            "pages": _count(entry.get("pages")),
                        }
                        for entry in documents
                        if isinstance(entry, dict)
                    ]
                    bundle["corpus_document_count"] = len(bundle["corpus_documents"])
                else:
                    bundle["errors"].append("corpus_source_pointer_unresolved")
                corpus_block = corpus_document.get("corpus")
                if isinstance(corpus_block, dict):
                    bundle["corpus_page_denominator"] = _count(
                        corpus_block.get("page_denominator")
                    )

        return bundle

    def read_acceptance_package(self, contract: Any) -> Optional[dict[str, Any]]:
        """Resolve the S08 acceptance package and every carrier it binds.

        The half this governs is the T05 acceptance package: one artifact that
        binds all fourteen S08 evidence carriers by repository-relative path +
        pointer + sha256, carries the T02 frozen-threshold evaluation, and
        enumerates every mandatory gate with a typed state and a computed
        ``s09_authorized``. The rule names only *where* the package lives, which
        carriers it must bind, what envelope it must carry and which frozen
        reference the threshold verdicts must stay bound to; nothing about the
        package contents is restated, so the rule cannot drift from the artifact
        it governs.

        ``None`` means the rule does not govern an acceptance-package half.
        Otherwise a bundle carrying the parsed package, the recomputed ``sha256``
        of every declared carrier, the resolved frozen-reference pins, and the
        rollback and same-n resource views the oracle refuses against.
        ``available`` is True only when the S03 declaration carrier resolves --
        the same authority the Java-free, native-family, native-content and
        native-route halves read -- so a tree that does not carry that authority
        is *absence*, not a violation. An unreadable *package* under a resolvable
        declaration is a claim with no evidence, and the oracle reports it.
        """
        if not isinstance(contract, dict):
            return None
        rule = contract.get("acceptance_package")
        if not isinstance(rule, dict):
            return None

        declaration_rule = rule.get("declaration")
        declaration_rule = declaration_rule if isinstance(declaration_rule, dict) else {}
        declaration_manifest = _text(declaration_rule.get("manifest"))
        declaration_pointer = _text(declaration_rule.get("pointer"))

        package_rule = rule.get("package")
        package_rule = package_rule if isinstance(package_rule, dict) else {}
        package_path = _text(package_rule.get("path"))
        package_schema = _text(package_rule.get("schema_version"))

        evaluation_rule = rule.get("evaluation")
        evaluation_rule = evaluation_rule if isinstance(evaluation_rule, dict) else {}
        reference_path = _text(evaluation_rule.get("frozen_reference_path"))
        reference_pointer = _text(evaluation_rule.get("reference_pointer"))

        rollback_rule = rule.get("rollback_carrier")
        rollback_rule = rollback_rule if isinstance(rollback_rule, dict) else {}
        rollback_path = _text(rollback_rule.get("path"))

        resource_rule = rule.get("resource_carrier")
        resource_rule = resource_rule if isinstance(resource_rule, dict) else {}
        resource_path = _text(resource_rule.get("path"))

        declared_carriers: list[dict[str, str]] = []
        carrier_rule = rule.get("evidence_carriers")
        if isinstance(carrier_rule, list):
            for entry in carrier_rule:
                if not isinstance(entry, dict):
                    continue
                manifest = _text(entry.get("manifest"))
                pointer = _text(entry.get("pointer"))
                if manifest and pointer:
                    declared_carriers.append({"manifest": manifest, "pointer": pointer})

        bundle: dict[str, Any] = {
            "available": False,
            "declaration_manifest": declaration_manifest,
            "declaration_pointer": declaration_pointer,
            "declaration_manifest_present": bool(
                declaration_manifest and self.path_exists(declaration_manifest)
            ),
            "package_schema_version": package_schema,
            "package_path": package_path,
            "package_exists": bool(package_path and self.path_exists(package_path)),
            "package_sha256": self.file_sha256(package_path) if package_path else None,
            "package": None,
            "declared_carriers": declared_carriers,
            "carrier_sha256": {},
            "reference_path": reference_path,
            "reference_pointer": reference_pointer,
            "reference_pins": None,
            "reference_pins_sha256": (
                self.file_sha256(reference_path) if reference_path else None
            ),
            "rollback_path": rollback_path,
            "rollback": None,
            "rollback_sha256": (
                self.file_sha256(rollback_path) if rollback_path else None
            ),
            "resource_path": resource_path,
            "resource": None,
            "resource_sha256": (
                self.file_sha256(resource_path) if resource_path else None
            ),
            "errors": [],
        }

        manifest = self.read_yaml(declaration_manifest) if declaration_manifest else None
        if not isinstance(manifest, dict):
            bundle["errors"].append("declaration_manifest_unreadable")
        else:
            block = _resolve_declared_pointer(manifest, declaration_pointer)
            if not isinstance(block, dict):
                bundle["errors"].append("declaration_pointer_unresolved")
            else:
                bundle["available"] = True

        bundle["package"] = self.read_json(package_path) if package_path else None
        if bundle["package"] is None and bundle["package_exists"]:
            bundle["errors"].append("package_unreadable")

        # Recompute every declared carrier's bytes so the package's own pins are
        # checked against content rather than trusted from a label.
        for entry in declared_carriers:
            rel = entry["manifest"]
            if rel not in bundle["carrier_sha256"]:
                bundle["carrier_sha256"][rel] = self.file_sha256(rel)

        reference_doc = self.read_json(reference_path) if reference_path else None
        if reference_doc is None and reference_path and self.path_exists(reference_path):
            bundle["errors"].append("reference_unreadable")
        if isinstance(reference_doc, dict):
            resolved = _resolve_declared_pointer(reference_doc, reference_pointer)
            bundle["reference_pins"] = resolved if isinstance(resolved, list) else []

        rollback_doc = self.read_json(rollback_path) if rollback_path else None
        if rollback_doc is None and rollback_path and self.path_exists(rollback_path):
            bundle["errors"].append("rollback_unreadable")
        bundle["rollback"] = self._acceptance_rollback_view(rollback_doc, rollback_rule)

        resource_doc = self.read_json(resource_path) if resource_path else None
        if resource_doc is None and resource_path and self.path_exists(resource_path):
            bundle["errors"].append("resource_unreadable")
        bundle["resource"] = self._acceptance_resource_view(resource_doc, resource_rule)

        return bundle

    @staticmethod
    def _acceptance_rollback_view(
        document: Any, rule: dict[str, Any]
    ) -> Optional[dict[str, Any]]:
        """The rollback fields the oracle refuses against, plus a byte-identity count.

        The rehearsal carrier is a per-document byte-identity proof; the summary
        counters alone could stay correct while the per-document evidence was
        dropped, so the view recomputes how many rows actually hold
        ``byte_identity`` and how many were rehearsed, and keeps the refused rows
        (a typed refusal is evidence, a silent skip is not).
        """
        if not isinstance(document, dict):
            return None
        residuals_pointer = _text(rule.get("residuals_pointer")) or "residuals"
        documents = document.get("documents")
        byte_identical = 0
        rehearsed = 0
        refusals: list[dict[str, Any]] = []
        if isinstance(documents, list):
            for entry in documents:
                if not isinstance(entry, dict):
                    continue
                if _text(entry.get("state")) == "refused":
                    refusals.append({
                        "id": _text(entry.get("id")),
                        "state": _text(entry.get("state")),
                        "refusal_reason": _text(entry.get("refusal_reason")),
                    })
                    continue
                rehearsed += 1
                if entry.get("byte_identity") is True:
                    byte_identical += 1
        return {
            "schema_version": _text(document.get("schema_version")),
            "claim_scope": _text(document.get("claim_scope")),
            "product_coverage_effect": _text(document.get("product_coverage_effect")),
            "thresholds": _text(document.get("thresholds")),
            "locks": document.get("locks"),
            "corpus_claim": document.get("corpus_claim"),
            "residuals": document.get(residuals_pointer)
            if isinstance(document.get(residuals_pointer), dict)
            else document.get("residuals"),
            "populations": document.get("populations"),
            "documents_rehearsed": rehearsed,
            "documents_byte_identical": byte_identical,
            "refusals": refusals,
        }

    @staticmethod
    def _acceptance_resource_view(
        document: Any, rule: dict[str, Any]
    ) -> Optional[dict[str, Any]]:
        """The same-n resource fields the oracle refuses against.

        The reference rows, the pairing contract and the residual bound-row count
        are kept; the per-document route rows are not, because the oracle refuses
        a pooled or saving claim from those three alone and the view must stay
        small enough to carry in a fixture.
        """
        if not isinstance(document, dict):
            return None
        residuals_pointer = _text(rule.get("residuals_pointer")) or "residuals"
        reference_rows = document.get("reference_metrics")
        return {
            "schema_version": _text(document.get("schema_version")),
            "claim_scope": _text(document.get("claim_scope")),
            "product_coverage_effect": _text(document.get("product_coverage_effect")),
            "thresholds": _text(document.get("thresholds")),
            "reference_metrics": reference_rows if isinstance(reference_rows, list) else [],
            "pairing": document.get("pairing"),
            "residuals": document.get(residuals_pointer)
            if isinstance(document.get(residuals_pointer), dict)
            else document.get("residuals"),
        }

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

    missing.extend(_java_free_slice_vocabulary(contract.get("java_free_slice")))
    missing.extend(_native_family_receipt_vocabulary(contract.get("native_family_receipt")))
    missing.extend(_native_content_receipt_vocabulary(contract.get("native_content_receipt")))
    missing.extend(_native_route_vocabulary(contract.get("native_route_e2e")))
    missing.extend(_acceptance_package_vocabulary(contract.get("acceptance_package")))
    return missing

def _java_free_slice_vocabulary(block: Any) -> list[str]:
    """Dotted names of ``java_free_slice`` fields that are absent or unusable.

    Without these the half cannot tell a declared-and-evidenced Java-free slice
    from a claim with no receipt: it needs to know where the claim lives, what a
    receipt must look like, the closed stage dispositions, and the closed label
    vocabulary. The declared label set must be exactly the domain's literal set —
    a label the oracle can emit must be registered, and a registered label must be
    one the oracle can emit — so an unregistered emission is a data/implementation
    mismatch rather than a silent finding. The caller fails closed on a non-empty
    result.
    """
    if not isinstance(block, dict):
        return ["java_free_slice"]
    missing: list[str] = []

    declaration = block.get("declaration")
    if not isinstance(declaration, dict):
        missing.append("java_free_slice.declaration")
    else:
        for key in ("manifest", "pointer"):
            if _text(declaration.get(key)) is None:
                missing.append(f"java_free_slice.declaration.{key}")
        if not _is_relative_path(declaration.get("manifest")):
            missing.append("java_free_slice.declaration.manifest.path")

    receipt = block.get("receipt")
    if not isinstance(receipt, dict):
        missing.append("java_free_slice.receipt")
    else:
        if _text(receipt.get("path")) is None:
            missing.append("java_free_slice.receipt.path")
        elif not _is_relative_path(receipt.get("path")):
            missing.append("java_free_slice.receipt.path")
        if _text(receipt.get("schema_version")) is None:
            missing.append("java_free_slice.receipt.schema_version")
        if receipt.get("required") is not True:
            missing.append("java_free_slice.receipt.required")

    stage_source = block.get("stage_id_source")
    if not isinstance(stage_source, dict):
        missing.append("java_free_slice.stage_id_source")
    else:
        for key in ("manifest", "pointer"):
            if _text(stage_source.get(key)) is None:
                missing.append(f"java_free_slice.stage_id_source.{key}")

    corpus = block.get("corpus")
    if not isinstance(corpus, dict):
        missing.append("java_free_slice.corpus")
    else:
        for key in ("manifest", "pointer"):
            if _text(corpus.get(key)) is None:
                missing.append(f"java_free_slice.corpus.{key}")

    expected = block.get("expected")
    if not isinstance(expected, dict):
        missing.append("java_free_slice.expected")
    else:
        for key in ("claim_scope", "product_coverage_effect", "guard_observer"):
            if _text(expected.get(key)) is None:
                missing.append(f"java_free_slice.expected.{key}")
        if expected.get("guard_installed") is not True:
            missing.append("java_free_slice.expected.guard_installed")
        if expected.get("guard_identity_isolated") is not True:
            missing.append("java_free_slice.expected.guard_identity_isolated")
        if _count(expected.get("observed_java_calls")) != 0:
            missing.append("java_free_slice.expected.observed_java_calls")

    dispositions = block.get("stage_dispositions")
    if not isinstance(dispositions, dict):
        missing.append("java_free_slice.stage_dispositions")
    else:
        for key in ("in_slice", "out_of_slice", "identity_admission"):
            if _text(dispositions.get(key)) is None:
                missing.append(f"java_free_slice.stage_dispositions.{key}")

    registered = set(_text_list(block.get("violations")))
    if registered != set(JAVA_FREE_SLICE_VIOLATION_LABELS):
        missing.append("java_free_slice.violations")

    fixtures = block.get("fixtures")
    if not isinstance(fixtures, dict):
        missing.append("java_free_slice.fixtures")
    else:
        for polarity in ("positive", "negative"):
            spec = fixtures.get(polarity)
            if not isinstance(spec, dict) or not _is_relative_path(spec.get("path")):
                missing.append(f"java_free_slice.fixtures.{polarity}.path")
    return missing

def _native_family_receipt_vocabulary(block: Any) -> list[str]:
    """Dotted names of ``native_family_receipt`` fields absent or unusable.

    Without these the half cannot tell a declared-and-evidenced native family
    receipt from a claim with no receipt, an untyped population or an
    unregistered label. The declared label set must be exactly the domain's
    literal set — a label the oracle can emit must be registered, and a
    registered label must be one the oracle can emit — so an unregistered
    emission is a data/implementation mismatch rather than a silent finding. The
    caller fails closed on a non-empty result.
    """
    if not isinstance(block, dict):
        return ["native_family_receipt"]
    missing: list[str] = []

    declaration = block.get("declaration")
    if not isinstance(declaration, dict):
        missing.append("native_family_receipt.declaration")
    else:
        for key in ("manifest", "pointer"):
            if _text(declaration.get(key)) is None:
                missing.append(f"native_family_receipt.declaration.{key}")
        if not _is_relative_path(declaration.get("manifest")):
            missing.append("native_family_receipt.declaration.manifest.path")

    receipts = block.get("receipts")
    if not isinstance(receipts, dict) or not receipts:
        missing.append("native_family_receipt.receipts")
    else:
        for receipt_id, spec in receipts.items():
            if not isinstance(spec, dict):
                missing.append(f"native_family_receipt.receipts.{receipt_id}")
                continue
            if not _is_relative_path(spec.get("path")):
                missing.append(f"native_family_receipt.receipts.{receipt_id}.path")
            for key in ("schema_version", "receipt_kind"):
                if _text(spec.get(key)) is None:
                    missing.append(f"native_family_receipt.receipts.{receipt_id}.{key}")
            if spec.get("required") is not True:
                missing.append(f"native_family_receipt.receipts.{receipt_id}.required")

    carriers = block.get("carriers")
    if not isinstance(carriers, list) or not carriers:
        missing.append("native_family_receipt.carriers")
    else:
        for index, entry in enumerate(carriers):
            if not isinstance(entry, dict) or _text(entry.get("id")) is None:
                missing.append(f"native_family_receipt.carriers[{index}]")
                continue
            if not _is_relative_path(entry.get("path")):
                missing.append(f"native_family_receipt.carriers[{index}].path")
            if _text(entry.get("pointer")) is None:
                missing.append(f"native_family_receipt.carriers[{index}].pointer")
            if _text(entry.get("kind")) is None:
                missing.append(f"native_family_receipt.carriers[{index}].kind")

    populations = block.get("populations")
    if not isinstance(populations, dict):
        missing.append("native_family_receipt.populations")
    else:
        parity = populations.get("parity")
        if not isinstance(parity, dict):
            missing.append("native_family_receipt.populations.parity")
        else:
            for key in ("population_kind", "document_count", "page_denominator"):
                if _count(parity.get(key)) is None and _text(parity.get(key)) is None:
                    missing.append(f"native_family_receipt.populations.parity.{key}")
            if not _text_list(parity.get("family_dispositions")):
                missing.append("native_family_receipt.populations.parity.family_dispositions")
        exclusion = populations.get("exclusion")
        if not isinstance(exclusion, dict):
            missing.append("native_family_receipt.populations.exclusion")
        else:
            for key in ("population_kind", "assigned", "excluded", "measurable", "targeted"):
                if _count(exclusion.get(key)) is None and _text(exclusion.get(key)) is None:
                    missing.append(f"native_family_receipt.populations.exclusion.{key}")
            if not isinstance(exclusion.get("reason_counts"), dict) or not exclusion.get("reason_counts"):
                missing.append("native_family_receipt.populations.exclusion.reason_counts")
            if not _text_list(exclusion.get("recovery_dispositions")):
                missing.append("native_family_receipt.populations.exclusion.recovery_dispositions")

    expected = block.get("expected")
    if not isinstance(expected, dict):
        missing.append("native_family_receipt.expected")
    else:
        for key in ("claim_scope", "product_coverage_effect", "thresholds", "guard_observer"):
            if _text(expected.get(key)) is None:
                missing.append(f"native_family_receipt.expected.{key}")
        for key in ("oracle_independent", "same_generator_refused"):
            if expected.get(key) is not True:
                missing.append(f"native_family_receipt.expected.{key}")

    registered = set(_text_list(block.get("violations")))
    if registered != set(NATIVE_FAMILY_VIOLATION_LABELS):
        missing.append("native_family_receipt.violations")

    fixtures = block.get("fixtures")
    if not isinstance(fixtures, dict):
        missing.append("native_family_receipt.fixtures")
    else:
        for polarity in ("positive", "negative"):
            spec = fixtures.get(polarity)
            if not isinstance(spec, dict) or not _is_relative_path(spec.get("path")):
                missing.append(f"native_family_receipt.fixtures.{polarity}.path")
    return missing

def _native_content_receipt_vocabulary(block: Any) -> list[str]:
    """Dotted names of ``native_content_receipt`` fields absent or unusable.

    Without these the half cannot tell a declared-and-evidenced content receipt
    from a claim with no receipt, an untyped group partition or an unregistered
    label. The declared label set must be exactly the domain's literal set — a
    label the oracle can emit must be registered, and a registered label must be
    one the oracle can emit — so an unregistered emission is a data/implementation
    mismatch rather than a silent finding. The caller fails closed on a non-empty
    result.
    """
    if not isinstance(block, dict):
        return ["native_content_receipt"]
    missing: list[str] = []

    declaration = block.get("declaration")
    if not isinstance(declaration, dict):
        missing.append("native_content_receipt.declaration")
    else:
        for key in ("manifest", "pointer"):
            if _text(declaration.get(key)) is None:
                missing.append(f"native_content_receipt.declaration.{key}")
        if not _is_relative_path(declaration.get("manifest")):
            missing.append("native_content_receipt.declaration.manifest.path")

    receipt = block.get("receipt")
    if not isinstance(receipt, dict):
        missing.append("native_content_receipt.receipt")
    else:
        if not _is_relative_path(receipt.get("path")):
            missing.append("native_content_receipt.receipt.path")
        if _text(receipt.get("schema_version")) is None:
            missing.append("native_content_receipt.receipt.schema_version")
        if receipt.get("required") is not True:
            missing.append("native_content_receipt.receipt.required")

    corpus = block.get("corpus")
    if not isinstance(corpus, dict):
        missing.append("native_content_receipt.corpus")
    else:
        for key in ("manifest", "pointer"):
            if _text(corpus.get(key)) is None:
                missing.append(f"native_content_receipt.corpus.{key}")

    oracle = block.get("oracle")
    if not isinstance(oracle, dict):
        missing.append("native_content_receipt.oracle")
    else:
        for key in ("manifest", "pointer", "segmentation_manifest", "segmentation_source"):
            if _text(oracle.get(key)) is None:
                missing.append(f"native_content_receipt.oracle.{key}")

    populations = block.get("populations")
    if not isinstance(populations, dict):
        missing.append("native_content_receipt.populations")
    else:
        for key in ("document_count", "page_denominator"):
            if _count(populations.get(key)) is None:
                missing.append(f"native_content_receipt.populations.{key}")

    expected = block.get("expected")
    if not isinstance(expected, dict):
        missing.append("native_content_receipt.expected")
    else:
        for key in (
            "claim_scope",
            "product_coverage_effect",
            "thresholds",
            "guard_observer",
            "forbidden_family_token",
        ):
            if _text(expected.get(key)) is None:
                missing.append(f"native_content_receipt.expected.{key}")
        for key in (
            "families",
            "required_families",
            "family_dispositions",
            "unmet_reasons",
            "body_verdicts",
            "body_reasons",
            "group_vocabulary",
            "group_source_vocabulary",
            "partition_states",
            "bytes_absent_reasons",
        ):
            if not _text_list(expected.get(key)):
                missing.append(f"native_content_receipt.expected.{key}")

    registered = set(_text_list(block.get("violations")))
    if registered != set(NATIVE_CONTENT_VIOLATION_LABELS):
        missing.append("native_content_receipt.violations")

    fixtures = block.get("fixtures")
    if not isinstance(fixtures, dict):
        missing.append("native_content_receipt.fixtures")
    else:
        for polarity in ("positive", "negative"):
            spec = fixtures.get(polarity)
            if not isinstance(spec, dict) or not _is_relative_path(spec.get("path")):
                missing.append(f"native_content_receipt.fixtures.{polarity}.path")
    return missing
def _native_route_vocabulary(block: Any) -> list[str]:
    """Dotted names of ``native_route_e2e`` fields absent or unusable.

    Without these the receipt-claim half cannot tell a declared-and-evidenced
    route from a claim with no receipt, an untyped stage/family/body cell or an
    unregistered label. The declared label set must be exactly the domain's
    literal set -- a label the oracle can emit must be registered, and a
    registered label must be one the oracle can emit -- so an unregistered
    emission is a data/implementation mismatch rather than a silent finding. The
    caller fails closed on a non-empty result.
    """
    if not isinstance(block, dict):
        return ["native_route_e2e"]
    missing: list[str] = []

    declaration = block.get("declaration")
    if not isinstance(declaration, dict):
        missing.append("native_route_e2e.declaration")
    else:
        for key in ("manifest", "pointer"):
            if _text(declaration.get(key)) is None:
                missing.append(f"native_route_e2e.declaration.{key}")
        if not _is_relative_path(declaration.get("manifest")):
            missing.append("native_route_e2e.declaration.manifest.path")

    stage_source = block.get("stage_id_source")
    if not isinstance(stage_source, dict):
        missing.append("native_route_e2e.stage_id_source")
    else:
        for key in ("manifest", "pointer"):
            if _text(stage_source.get(key)) is None:
                missing.append(f"native_route_e2e.stage_id_source.{key}")

    for key in ("receipt", "resource_receipt"):
        receipt = block.get(key)
        if not isinstance(receipt, dict):
            missing.append(f"native_route_e2e.{key}")
            continue
        if not _is_relative_path(receipt.get("path")):
            missing.append(f"native_route_e2e.{key}.path")
        if _text(receipt.get("schema_version")) is None:
            missing.append(f"native_route_e2e.{key}.schema_version")
        if receipt.get("required") is not True:
            missing.append(f"native_route_e2e.{key}.required")

    corpus = block.get("corpus")
    if not isinstance(corpus, dict):
        missing.append("native_route_e2e.corpus")
    else:
        for key in ("manifest", "pointer"):
            if _text(corpus.get(key)) is None:
                missing.append(f"native_route_e2e.corpus.{key}")
        for key in ("document_count", "page_denominator"):
            if _count(corpus.get(key)) is None:
                missing.append(f"native_route_e2e.corpus.{key}")

    limit_policy = block.get("limit_policy")
    if not isinstance(limit_policy, dict):
        missing.append("native_route_e2e.limit_policy")
    else:
        for key in ("state", "freeze_scope", "claim_scope", "product_coverage_effect", "thresholds"):
            if _text(limit_policy.get(key)) is None:
                missing.append(f"native_route_e2e.limit_policy.{key}")
        if not _text_list(limit_policy.get("metrics")):
            missing.append("native_route_e2e.limit_policy.metrics")
        if not _text_list(limit_policy.get("direction_vocabulary")):
            missing.append("native_route_e2e.limit_policy.direction_vocabulary")
        if not isinstance(limit_policy.get("freeze_revision"), dict):
            missing.append("native_route_e2e.limit_policy.freeze_revision")

    limit_vocabulary = block.get("limit_vocabulary")
    if not isinstance(limit_vocabulary, dict):
        missing.append("native_route_e2e.limit_vocabulary")
    else:
        for key in ("source", "metric_source"):
            if _text(limit_vocabulary.get(key)) is None:
                missing.append(f"native_route_e2e.limit_vocabulary.{key}")
        for key in ("fields", "direction_vocabulary"):
            if not _text_list(limit_vocabulary.get(key)):
                missing.append(f"native_route_e2e.limit_vocabulary.{key}")

    expected = block.get("expected")
    if not isinstance(expected, dict):
        missing.append("native_route_e2e.expected")
    else:
        for key in (
            "claim_scope",
            "product_coverage_effect",
            "thresholds",
            "guard_observer",
            "native_population",
            "out_of_slice_stance",
            "admission_stance",
            "forbidden_family_token",
            "reference_source_kind",
        ):
            if _text(expected.get(key)) is None:
                missing.append(f"native_route_e2e.expected.{key}")
        locks = expected.get("locks")
        if not isinstance(locks, dict):
            missing.append("native_route_e2e.expected.locks")
        else:
            for key in (
                "deployed",
                "import_eligible",
                "auto_promote",
                "production_route",
                "graph_write",
            ):
                if key not in locks:
                    missing.append(f"native_route_e2e.expected.locks.{key}")
        for key in (
            "population_vocabulary",
            "resource_population_vocabulary",
            "observation_state_vocabulary",
            "absence_reason_vocabulary",
            "native_metric_vocabulary",
            "reference_metric_vocabulary",
            "body_verdict_vocabulary",
            "body_reason_vocabulary",
            "stage_stance_vocabulary",
            "family_disposition_vocabulary",
            "family_unmet_reason_vocabulary",
            "family_refusal_reason_vocabulary",
            "residual_fields",
        ):
            if not _text_list(expected.get(key)):
                missing.append(f"native_route_e2e.expected.{key}")
        pairing = expected.get("pairing_vocabulary")
        if not isinstance(pairing, dict):
            missing.append("native_route_e2e.expected.pairing_vocabulary")
        else:
            for key in ("rule", "pooled", "saving_claim", "mismatch_disposition"):
                if _text(pairing.get(key)) is None:
                    missing.append(f"native_route_e2e.expected.pairing_vocabulary.{key}")

    registered = set(_text_list(block.get("violations")))
    if registered != set(NATIVE_ROUTE_VIOLATION_LABELS):
        missing.append("native_route_e2e.violations")

    fixtures = block.get("fixtures")
    if not isinstance(fixtures, dict):
        missing.append("native_route_e2e.fixtures")
    else:
        for polarity in ("positive", "negative"):
            spec = fixtures.get(polarity)
            if not isinstance(spec, dict) or not _is_relative_path(spec.get("path")):
                missing.append(f"native_route_e2e.fixtures.{polarity}.path")
    return missing


# ── shared value helpers ───────────────────────────────────────────────────


#: Marker returned by :func:`resolve_pointer` when a pointer cannot be followed.
_UNRESOLVED = object()

#: One pointer segment: an object key, optionally selecting from a list of
#: mappings by ``field=value``. Enough to address a measurement by its metric_id
#: without inventing a JSON-pointer library (and its escape grammar) for it.
_POINTER_SELECTOR_RE = re.compile(r"^([A-Za-z0-9_]+)\[([A-Za-z0-9_]+)=([^\[\]]+)\]$")


def _acceptance_package_vocabulary(block: Any) -> list[str]:
    """Dotted names of ``acceptance_package`` fields absent or unusable.

    Without these the gate-ledger half cannot tell a declared-and-evidenced
    package from a claim with no package, an untyped gate ledger or an
    unregistered label. The declared label set must be exactly the domain's
    literal set -- a label the oracle can emit must be registered, and a
    registered label must be one the oracle can emit -- so an unregistered
    emission is a data/implementation mismatch rather than a silent finding. The
    caller fails closed on a non-empty result.
    """
    if not isinstance(block, dict):
        return ["acceptance_package"]
    missing: list[str] = []

    declaration = block.get("declaration")
    if not isinstance(declaration, dict):
        missing.append("acceptance_package.declaration")
    else:
        for key in ("manifest", "pointer"):
            if _text(declaration.get(key)) is None:
                missing.append(f"acceptance_package.declaration.{key}")
        if not _is_relative_path(declaration.get("manifest")):
            missing.append("acceptance_package.declaration.manifest.path")

    package = block.get("package")
    if not isinstance(package, dict):
        missing.append("acceptance_package.package")
    else:
        if not _is_relative_path(package.get("path")):
            missing.append("acceptance_package.package.path")
        if _text(package.get("schema_version")) is None:
            missing.append("acceptance_package.package.schema_version")
        if package.get("required") is not True:
            missing.append("acceptance_package.package.required")

    carriers = block.get("evidence_carriers")
    if not isinstance(carriers, list) or not carriers:
        missing.append("acceptance_package.evidence_carriers")
    else:
        for entry in carriers:
            if (
                not isinstance(entry, dict)
                or _text(entry.get("manifest")) is None
                or _text(entry.get("pointer")) is None
            ):
                missing.append("acceptance_package.evidence_carriers.entry")

    evaluation = block.get("evaluation")
    if not isinstance(evaluation, dict):
        missing.append("acceptance_package.evaluation")
    else:
        for key in (
            "frozen_reference_path",
            "reference_pointer",
            "candidate_source_path",
            "threshold_policy_state",
            "body_verdict",
            "body_residual_token",
        ):
            if _text(evaluation.get(key)) is None:
                missing.append(f"acceptance_package.evaluation.{key}")

    rollback = block.get("rollback_carrier")
    if not isinstance(rollback, dict):
        missing.append("acceptance_package.rollback_carrier")
    else:
        for key in (
            "path",
            "pointer",
            "claim_scope",
            "byte_identity_field",
            "fragment_leak_field",
        ):
            if _text(rollback.get(key)) is None:
                missing.append(f"acceptance_package.rollback_carrier.{key}")

    resource = block.get("resource_carrier")
    if not isinstance(resource, dict):
        missing.append("acceptance_package.resource_carrier")
    else:
        for key in (
            "path",
            "pointer",
            "absence_state",
            "pairing_rule",
            "forbidden_pooling",
            "forbidden_saving",
        ):
            if _text(resource.get(key)) is None:
                missing.append(f"acceptance_package.resource_carrier.{key}")

    expected = block.get("expected")
    if not isinstance(expected, dict):
        missing.append("acceptance_package.expected")
    else:
        for key in (
            "claim_scope",
            "product_coverage_effect",
            "thresholds",
            "authorization_basis",
        ):
            if _text(expected.get(key)) is None:
                missing.append(f"acceptance_package.expected.{key}")
        if _count(expected.get("minimum_mandatory_gates")) is None:
            missing.append("acceptance_package.expected.minimum_mandatory_gates")
        for key in (
            "gate_state_vocabulary",
            "non_blocking_states",
            "gate_kind_vocabulary",
            "required_gate_ids",
            "redaction_flags",
        ):
            if not _text_list(expected.get(key)):
                missing.append(f"acceptance_package.expected.{key}")
        locks = expected.get("locks")
        if not isinstance(locks, dict):
            missing.append("acceptance_package.expected.locks")
        else:
            for key in (
                "deployed",
                "import_eligible",
                "auto_promote",
                "production_route",
                "graph_write",
            ):
                if key not in locks:
                    missing.append(f"acceptance_package.expected.locks.{key}")

    registered = set(_text_list(block.get("violations")))
    if registered != set(ACCEPTANCE_PACKAGE_VIOLATION_LABELS):
        missing.append("acceptance_package.violations")

    fixtures = block.get("fixtures")
    if not isinstance(fixtures, dict):
        missing.append("acceptance_package.fixtures")
    else:
        for polarity in ("positive", "negative"):
            spec = fixtures.get(polarity)
            if not isinstance(spec, dict) or not _is_relative_path(spec.get("path")):
                missing.append(f"acceptance_package.fixtures.{polarity}.path")
    return missing

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


def _resolve_declared_pointer(document: Any, pointer: Any) -> Any:
    """Follow a pointer in either dialect the slice DATA actually uses.

    The Governor's generic :func:`resolve_pointer` grammar is slash-separated. The
    Java-free slice declaration — the ADR-077 §3 authority in the layout-bakeoff
    DATA — addresses its sources with dot-separated pointers
    (``defaults.cascade_stages``, ``corpus.documents``). Rewriting the authority to
    the governor's dialect would move its ``sha256`` and stale every receipt pin,
    so the reader accepts the dialect the authority declares: slash first, then
    dot-as-slash. :data:`_UNRESOLVED` means neither landed, which the caller
    reports as a typed error rather than as an empty resolution.
    """
    resolved = resolve_pointer(document, pointer)
    if resolved is not _UNRESOLVED:
        return resolved
    if isinstance(pointer, str) and "." in pointer:
        return resolve_pointer(document, pointer.replace(".", "/"))
    return _UNRESOLVED

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
    _java_free_slice_violations_into(contract, observations, violations)
    _native_family_receipt_violations_into(contract, observations, violations)
    _native_content_receipt_violations_into(contract, observations, violations)
    _native_route_violations_into(contract, observations, violations)
    _acceptance_package_violations_into(contract, observations, violations)
    if _scope_accepted(rows, contract):
        _regression_violations(scorecard, rows, contract, violations)
    return violations

def _numeric_field(block: Any, keys: tuple[str, ...]) -> Optional[int]:
    """The first non-negative integer under ``keys``, or ``None``.

    Used to tell a *typed* stage disposition from a *counted* one: an
    out-of-slice stage that carries any numeric count is claiming a measurement
    it must not have, and an in-slice stage without one is unmeasured.
    """
    if not isinstance(block, dict):
        return None
    for key in keys:
        value = _count(block.get(key))
        if value is not None:
            return value
    return None

def _java_free_slice_violations_into(
    contract: Any,
    observations: Any,
    violations: set[str],
) -> None:
    """Add the Java-free vertical slice labels carried by the observations."""
    violations.update(
        _java_free_slice_violations(contract, observations.get("java_free_slice"))
        if isinstance(observations, dict)
        else set()
    )

def _java_free_slice_violations(contract: Any, evidence: Any) -> set[str]:
    """Refuse a Java-free slice claim that its receipt does not actually evidence.

    The half this guards is the shape R141 forbids: a slice that is *declared*
    Java-free (or, worse, offered as product coverage) while the artifact that would
    evidence it is absent, stale, scoped to another slice, or reports an untyped or
    counted stage where the boundary says "outside the slice". Concretely:

    * ``claim_without_receipt`` — the declaration resolves but no usable receipt
      exists, so there is a claim and no evidence;
    * ``receipt_slice_mismatch`` — the receipt is not the receipt of *this*
      declaration (schema, slice, ADR, requirements, binding pointers or the pinned
      corpus identities differ), or the declaration does not classify every cascade
      stage;
    * ``receipt_stale`` — the receipt pins declaration/source bytes whose recomputed
      ``sha256`` no longer matches, so it describes an older tree;
    * ``unmeasured_stage`` — a cascade stage has no typed disposition in the receipt
      (or the receipt names a stage the declaration does not);
    * ``stage_outside_slice_counted`` — an out-of-slice stage carries a numeric
      count or an ``observed`` state instead of its typed reason, i.e. the boundary
      is being reported as a measurement;
    * ``guard_not_installed`` — an ``observed`` zero is offered without the guard
      provenance (installed + identity-isolated observer) that would make the zero
      mean anything;
    * ``java_calls_nonzero`` — the receipt contradicts itself: Java calls were seen
      on a slice that claims none;
    * ``product_coverage_claimed`` — the claim is not scoped diagnostic-only with no
      product-coverage effect, which is the only scope that does not move the
      product ``java_independence`` axis (D499).

    ``{""}`` (empty) means clean. A missing or absent-evidence bundle is *unverified*,
    never clean and never refuted: a tree that does not carry the claim is silence,
    not compliance, so nothing here is inferred from an artifact's mere existence.
    """
    violations: set[str] = set()
    if not isinstance(contract, dict):
        return violations
    rule = contract.get("java_free_slice")
    if not isinstance(rule, dict):
        return violations
    if not isinstance(evidence, dict) or not evidence:
        return violations
    if evidence.get("available") is not True:
        return violations
    declaration = evidence.get("declaration")
    if not isinstance(declaration, dict):
        return violations

    receipt_rule = rule.get("receipt") if isinstance(rule.get("receipt"), dict) else {}
    receipt = evidence.get("receipt")
    if not isinstance(receipt, dict):
        if receipt_rule.get("required") is True:
            violations.add("java_free_slice_claim_without_receipt")
        return violations

    expected = rule.get("expected") if isinstance(rule.get("expected"), dict) else {}
    dispositions = (
        rule.get("stage_dispositions")
        if isinstance(rule.get("stage_dispositions"), dict)
        else {}
    )

    # ── claim scope: a diagnostic slice is never product coverage ──────────
    expected_scope = _text(expected.get("claim_scope"))
    expected_effect = _text(expected.get("product_coverage_effect"))
    for block in (receipt, declaration):
        if _text(block.get("claim_scope")) != expected_scope:
            violations.add("java_free_slice_product_coverage_claimed")
        if _text(block.get("product_coverage_effect")) != expected_effect:
            violations.add("java_free_slice_product_coverage_claimed")

    # ── identity: the receipt must be the receipt of THIS declaration ──────
    mismatch = False
    if _text(receipt.get("schema_version")) != _text(receipt_rule.get("schema_version")):
        mismatch = True
    if _text(receipt.get("slice_id")) != _text(declaration.get("slice_id")):
        mismatch = True
    if _text(receipt.get("adr_ref")) != _text(declaration.get("adr_ref")):
        mismatch = True
    if sorted(_text_list(receipt.get("requirement_ids"))) != sorted(
        _text_list(declaration.get("requirement_ids"))
    ):
        mismatch = True

    rule_declaration = rule.get("declaration") if isinstance(rule.get("declaration"), dict) else {}
    receipt_declaration = (
        receipt.get("declaration") if isinstance(receipt.get("declaration"), dict) else {}
    )
    if _text(receipt_declaration.get("manifest")) != _text(rule_declaration.get("manifest")):
        mismatch = True
    if _text(receipt_declaration.get("pointer")) != _text(rule_declaration.get("pointer")):
        mismatch = True

    rule_stage_source = (
        rule.get("stage_id_source") if isinstance(rule.get("stage_id_source"), dict) else {}
    )
    receipt_stage_source = (
        receipt.get("stage_id_source") if isinstance(receipt.get("stage_id_source"), dict) else {}
    )
    if _text(receipt_stage_source.get("manifest")) != _text(rule_stage_source.get("manifest")):
        mismatch = True
    if _text(receipt_stage_source.get("pointer")) != _text(rule_stage_source.get("pointer")):
        mismatch = True

    # ── staleness: the receipt pins the bytes it was derived from ──────────
    declaration_pin = _text(receipt_declaration.get("revision_sha256"))
    if declaration_pin is None or declaration_pin != _text(evidence.get("declaration_sha256")):
        violations.add("java_free_slice_receipt_stale")
    stage_pin = _text(receipt_stage_source.get("revision_sha256"))
    if stage_pin is None or stage_pin != _text(evidence.get("stage_id_source_sha256")):
        violations.add("java_free_slice_receipt_stale")

    # ── the corpus binding: the pinned identities, not a restated count ────
    corpus = receipt.get("corpus") if isinstance(receipt.get("corpus"), dict) else None
    rule_corpus = rule.get("corpus") if isinstance(rule.get("corpus"), dict) else {}
    pinned_documents = {
        _text(entry.get("id")): _text(entry.get("sha256"))
        for entry in (evidence.get("corpus_documents") or [])
        if isinstance(entry, dict)
    }
    if not isinstance(corpus, dict):
        mismatch = True
    else:
        if _text(corpus.get("manifest")) != _text(rule_corpus.get("manifest")):
            mismatch = True
        if _text(corpus.get("pointer")) != _text(rule_corpus.get("pointer")):
            mismatch = True
        binding = (
            declaration.get("corpus_binding")
            if isinstance(declaration.get("corpus_binding"), dict)
            else {}
        )
        declared_count = _count(binding.get("document_count"))
        receipt_count = _count(corpus.get("document_count"))
        if receipt_count is None or (
            declared_count is not None and receipt_count != declared_count
        ):
            mismatch = True
        if receipt_count != len(pinned_documents):
            mismatch = True
        identities = corpus.get("identities")
        if not isinstance(identities, list):
            mismatch = True
        else:
            receipt_documents = {
                _text(entry.get("id")): _text(entry.get("sha256"))
                for entry in identities
                if isinstance(entry, dict)
            }
            if receipt_documents != pinned_documents:
                mismatch = True

    # ── the stage matrix: every cascade stage has a typed disposition ──────
    scope_by_stage: dict[str, str] = {}
    admission_reason: Optional[str] = None
    admission = declaration.get("identity_admission_stage")
    if isinstance(admission, dict):
        stage_id = _text(admission.get("stage_id"))
        if stage_id is not None:
            scope_by_stage[stage_id] = "identity_admission"
        admission_reason = _text(admission.get("disposition"))
    for key, scope in (
        ("stages_in_slice", "in_slice"),
        ("stages_out_of_slice", "out_of_slice"),
    ):
        entries = declaration.get(key)
        if not isinstance(entries, list):
            continue
        for entry in entries:
            if not isinstance(entry, dict):
                continue
            stage_id = _text(entry.get("stage_id"))
            if stage_id is not None:
                scope_by_stage[stage_id] = scope

    cascade_stage_ids = {
        stage_id
        for stage_id in (evidence.get("cascade_stage_ids") or [])
        if isinstance(stage_id, str) and stage_id.strip()
    }
    if set(scope_by_stage) != cascade_stage_ids:
        # The declaration must classify the whole cascade: an unclassified stage
        # is a hole in the boundary, not a data detail.
        mismatch = True

    out_of_slice_reasons = set(_text_list(declaration.get("out_of_slice_reason_vocabulary")))
    guard = receipt.get("guard") if isinstance(receipt.get("guard"), dict) else None
    guard_provenance = (
        isinstance(guard, dict)
        and guard.get("installed") is True
        and guard.get("identity_isolated") is True
        and _text(guard.get("observer")) == _text(expected.get("guard_observer"))
    )
    if isinstance(guard, dict):
        observed_calls = _count(guard.get("observed_java_calls"))
        if observed_calls is None or observed_calls != 0:
            violations.add("java_free_slice_java_calls_nonzero")
        elif not guard_provenance:
            # Zero observed Java calls is only evidence when the observer that
            # would have seen them was installed and identity-isolated.
            violations.add("java_free_slice_guard_not_installed")
    else:
        violations.add("java_free_slice_guard_not_installed")

    stages = receipt.get("stages")
    if not isinstance(stages, list):
        violations.add("java_free_slice_unmeasured_stage")
    else:
        receipt_stage_ids: set[str] = set()
        for entry in stages:
            if not isinstance(entry, dict):
                violations.add("java_free_slice_unmeasured_stage")
                continue
            stage_id = _text(entry.get("stage_id"))
            if stage_id is None:
                violations.add("java_free_slice_unmeasured_stage")
                continue
            receipt_stage_ids.add(stage_id)
            scope = scope_by_stage.get(stage_id)
            count = entry.get("count") if isinstance(entry.get("count"), dict) else None
            if scope is None or count is None:
                violations.add("java_free_slice_unmeasured_stage")
                continue
            state = _text(count.get("state"))
            if scope == "out_of_slice":
                if (
                    _numeric_field(count, ("attempts", "count", "java_calls", "invocations"))
                    is not None
                ):
                    violations.add("java_free_slice_stage_outside_slice_counted")
                if state != _text(dispositions.get("out_of_slice")):
                    violations.add("java_free_slice_stage_outside_slice_counted")
                reason = _text(count.get("reason"))
                if reason is None or (
                    out_of_slice_reasons and reason not in out_of_slice_reasons
                ):
                    violations.add("java_free_slice_unmeasured_stage")
            elif scope == "identity_admission":
                if state != _text(dispositions.get("identity_admission")):
                    violations.add("java_free_slice_unmeasured_stage")
                reason = _text(count.get("reason"))
                if reason is None or (
                    admission_reason is not None and reason != admission_reason
                ):
                    violations.add("java_free_slice_unmeasured_stage")
            else:
                if state != _text(dispositions.get("in_slice")):
                    violations.add("java_free_slice_unmeasured_stage")
                attempts = _numeric_field(count, ("attempts",))
                if attempts is None:
                    violations.add("java_free_slice_unmeasured_stage")
                elif attempts > 0:
                    violations.add("java_free_slice_java_calls_nonzero")
                elif not guard_provenance:
                    # Observed(0) without guard provenance is not a measurement.
                    violations.add("java_free_slice_guard_not_installed")
        if receipt_stage_ids != set(scope_by_stage):
            violations.add("java_free_slice_unmeasured_stage")

    if mismatch:
        violations.add("java_free_slice_receipt_slice_mismatch")
    return violations

def _native_family_receipt_violations_into(
    contract: Any,
    observations: Any,
    violations: set[str],
) -> None:
    """Add the native family receipt labels carried by the observations."""
    violations.update(
        _native_family_receipt_violations(
            contract,
            observations.get("native_family_receipt") if isinstance(observations, dict) else None,
        )
    )

def _native_content_receipt_violations_into(
    contract: Any,
    observations: Any,
    violations: set[str],
) -> None:
    """Add the native content receipt labels carried by the observations."""
    violations.update(
        _native_content_receipt_violations(
            contract,
            observations.get("native_content_receipt")
            if isinstance(observations, dict)
            else None,
        )
    )

def _native_route_violations_into(
    contract: Any,
    observations: Any,
    violations: set[str],
) -> None:
    """Add the native route + same-n resource labels carried by the observations."""
    violations.update(
        _native_route_violations(
            contract,
            observations.get("native_route_e2e")
            if isinstance(observations, dict)
            else None,
        )
    )

def _acceptance_package_violations_into(
    contract: Any,
    observations: Any,
    violations: set[str],
) -> None:
    """Add the acceptance-package gate-ledger labels carried by the observations."""
    violations.update(
        _acceptance_package_violations(
            contract,
            observations.get("acceptance_package")
            if isinstance(observations, dict)
            else None,
        )
    )

def _native_family_receipt_violations(contract: Any, evidence: Any) -> set[str]:
    """Refuse a native family receipt claim the artifacts do not evidence.

    The shape R141 forbids is a *claim* of source-bound native progress — header
    and bibliography atoms produced by the native parsers, on the declared
    3-document slice — with no receipt, or with a receipt that is stale, scoped
    outside the frozen population, guarded by nobody, scored by its own generator,
    or publishing a product-coverage or threshold claim the diagnostic boundary
    does not allow. Concretely:

    * ``claim_without_receipt`` — the S03 declaration resolves but a required
      receipt is absent or unreadable, so there is a claim and no evidence;
    * ``receipt_population_mismatch`` — a receipt is not the receipt of *this*
      rule (schema/kind/declaration/carrier pointers differ), its 3-document
      corpus pin or the frozen 610/531/79/72-5-2 denominator does not match the
      carrier bytes, or a family/document/recovery row carries no typed
      disposition;
    * ``receipt_stale`` — a receipt pins carrier bytes whose recomputed
      ``sha256`` no longer matches, so it describes an older tree;
    * ``product_coverage_claimed`` — the claim is not diagnostic-only with no
      product-coverage effect, or the promotion locks are not all closed, which
      is the only scope that does not move the product ``java_independence`` axis
      (D499);
    * ``java_calls_nonzero`` — the receipt reports Java calls on a plane that
      claims none;
    * ``guard_not_installed`` — an observed zero is offered without the guard
      provenance (installed + identity-isolated observer) that would make the
      zero mean anything;
    * ``oracle_not_independent`` — the parity oracle is missing or not declared
      independent / same-generator-refused / byte-bound to the preserved TEI pins;
    * ``threshold_claimed`` — a numeric threshold is published although the
      acquisition declares none (``thresholds != not_accepted``);
    * ``exclusion_denominator_dropped`` — the 79 typed baseline exclusions no
      longer partition the denominator, or their per-document rows are not the
      79 targeted documents.

    ``set()`` (empty) means clean. A missing or absent-evidence bundle is
    *unverified*, never clean and never refuted: a tree that does not carry the
    declaration authority is silence, not compliance, so nothing here is inferred
    from an artifact's mere existence.
    """
    violations: set[str] = set()
    if not isinstance(contract, dict) or not isinstance(evidence, dict) or not evidence:
        return violations
    if evidence.get("available") is not True:
        return violations
    rule = contract.get("native_family_receipt")
    if not isinstance(rule, dict):
        return violations
    expected = rule.get("expected") if isinstance(rule.get("expected"), dict) else {}
    populations = rule.get("populations") if isinstance(rule.get("populations"), dict) else {}
    receipts = evidence.get("receipts") if isinstance(evidence.get("receipts"), dict) else {}
    carriers = evidence.get("carriers") if isinstance(evidence.get("carriers"), dict) else {}

    rule_carriers: dict[str, dict[str, Any]] = {}
    for entry in rule.get("carriers") or []:
        if isinstance(entry, dict) and _text(entry.get("id")) is not None:
            rule_carriers[str(_text(entry.get("id")))] = entry

    def _carrier_path(carrier_id: str) -> Optional[str]:
        entry = rule_carriers.get(carrier_id)
        return _text(entry.get("path")) if isinstance(entry, dict) else None

    def _carrier_pointer(carrier_id: str) -> Optional[str]:
        entry = rule_carriers.get(carrier_id)
        return _text(entry.get("pointer")) if isinstance(entry, dict) else None

    def _carrier_sha(carrier_id: str) -> Optional[str]:
        entry = carriers.get(carrier_id)
        return _text(entry.get("sha256")) if isinstance(entry, dict) else None

    def mismatch() -> None:
        violations.add("native_family_receipt_population_mismatch")

    def stale() -> None:
        violations.add("native_family_receipt_stale")

    def _bare(digest: Any) -> Optional[str]:
        """A digest with the ``sha256:`` prefix removed, for like-for-like comparison."""
        text = _text(digest)
        if text is None:
            return None
        return text.split("sha256:", 1)[1] if text.startswith("sha256:") else text

    def _declares_false(value: Any) -> bool:
        """True when a lock declares false in either JSON representation."""
        if value is False:
            return True
        return isinstance(value, str) and value.strip().lower() == "false"

    # ── a required receipt that does not exist is a claim without evidence ─
    for receipt_id, spec in (rule.get("receipts") or {}).items():
        entry = receipts.get(receipt_id)
        entry = entry if isinstance(entry, dict) else {}
        if entry.get("document") is None and isinstance(spec, dict) and spec.get("required") is True:
            violations.add("native_family_claim_without_receipt")

    # ── both receipts: scope, locks, thresholds, guard, redaction ─────────
    for receipt_id in ("parity", "exclusion"):
        entry = receipts.get(receipt_id)
        entry = entry if isinstance(entry, dict) else {}
        document = entry.get("document")
        if not isinstance(document, dict):
            continue
        if _text(document.get("schema_version")) != _text(entry.get("schema_version")):
            mismatch()
        if _text(document.get("receipt_kind")) != _text(entry.get("receipt_kind")):
            mismatch()
        if _text(document.get("claim_scope")) != _text(expected.get("claim_scope")):
            violations.add("native_family_product_coverage_claimed")
        if _text(document.get("product_coverage_effect")) != _text(
            expected.get("product_coverage_effect")
        ):
            violations.add("native_family_product_coverage_claimed")
        locks = document.get("locks")
        if not isinstance(locks, dict):
            violations.add("native_family_product_coverage_claimed")
        elif (
            _text(locks.get("auto_promote")) != "forbidden"
            or not _declares_false(locks.get("deployed"))
            or _text(locks.get("graph_write")) != "forbidden"
            or not _declares_false(locks.get("import_eligible"))
            or _text(locks.get("production_route")) != "forbidden"
        ):
            violations.add("native_family_product_coverage_claimed")
        if _text(document.get("thresholds")) != _text(expected.get("thresholds")):
            violations.add("native_family_threshold_claimed")
        if any(key in document for key in ("threshold", "threshold_value", "numeric_threshold")):
            violations.add("native_family_threshold_claimed")
        redaction = document.get("redaction")
        if isinstance(redaction, dict) and any(value is True for value in redaction.values()):
            violations.add("native_family_product_coverage_claimed")
        guard = document.get("guard")
        if not isinstance(guard, dict):
            violations.add("native_family_guard_not_installed")
        else:
            observed = _count(guard.get("observed_java_calls"))
            if observed is None or observed != 0:
                violations.add("native_family_java_calls_nonzero")
            elif (
                guard.get("installed") is not True
                or guard.get("identity_isolated") is not True
                or _text(guard.get("observer")) != _text(expected.get("guard_observer"))
            ):
                violations.add("native_family_guard_not_installed")

    # ── parity: independent oracle + the three-document corpus pin ────────
    parity_entry = receipts.get("parity")
    parity_entry = parity_entry if isinstance(parity_entry, dict) else {}
    parity = parity_entry.get("document")
    parity_pop = populations.get("parity") if isinstance(populations.get("parity"), dict) else {}
    if isinstance(parity, dict):
        declaration = parity.get("declaration")
        declaration = declaration if isinstance(declaration, dict) else {}
        if _text(declaration.get("manifest")) != _text(evidence.get("declaration_manifest")):
            mismatch()
        if _text(declaration.get("pointer")) != _text(evidence.get("declaration_pointer")):
            mismatch()
        if _text(declaration.get("revision_sha256")) != _carrier_sha("layout_bakeoff"):
            stale()

        oracle = parity.get("oracle")
        if not isinstance(oracle, dict):
            violations.add("native_family_oracle_not_independent")
        else:
            if (
                oracle.get("independent") is not True
                or oracle.get("same_generator_refused") is not True
                or _text(oracle.get("generator_id")) is None
                or _text(oracle.get("backend")) is None
            ):
                violations.add("native_family_oracle_not_independent")
            if _text(oracle.get("manifest")) != _carrier_path("preserved_tei_provenance"):
                violations.add("native_family_oracle_not_independent")
            if _text(oracle.get("revision_sha256")) != _carrier_sha("preserved_tei_provenance"):
                stale()
            pins = evidence.get("preserved_tei_pins")
            pins = pins if isinstance(pins, dict) else {}
            documents = oracle.get("documents")
            if not isinstance(documents, list) or not documents:
                violations.add("native_family_oracle_not_independent")
            else:
                for document in documents:
                    if not isinstance(document, dict):
                        violations.add("native_family_oracle_not_independent")
                        continue
                    fixture_path = _text(document.get("fixture_path"))
                    declared_sha = _text(document.get("sha256"))
                    pin = pins.get(fixture_path) if fixture_path is not None else None
                    if not isinstance(pin, dict):
                        violations.add("native_family_oracle_not_independent")
                        continue
                    if _bare(pin.get("sha256")) != _bare(declared_sha) or _bare(
                        pin.get("recomputed_sha256")
                    ) != _bare(declared_sha):
                        stale()

        population_blocks = parity.get("populations")
        first = (
            population_blocks[0]
            if isinstance(population_blocks, list) and population_blocks
            else None
        )
        if not isinstance(first, dict):
            mismatch()
        else:
            if _text(first.get("population_kind")) != _text(parity_pop.get("population_kind")):
                mismatch()
            if _text(first.get("manifest")) != _carrier_path("layout_bakeoff"):
                mismatch()
            if _text(first.get("pointer")) != _carrier_pointer("layout_bakeoff"):
                mismatch()
            if _text(first.get("revision_sha256")) != _carrier_sha("layout_bakeoff"):
                stale()
            if _count(first.get("document_count")) != _count(parity_pop.get("document_count")):
                mismatch()
            if _count(first.get("page_denominator")) != _count(parity_pop.get("page_denominator")):
                mismatch()
            if _count(first.get("page_denominator")) != _count(
                evidence.get("corpus_page_denominator")
            ):
                mismatch()
            corpus_documents = evidence.get("corpus_documents")
            declared_documents = first.get("documents")
            if not isinstance(corpus_documents, list) or not isinstance(declared_documents, list):
                mismatch()
            else:
                pinned = {
                    _text(entry.get("id")): _text(entry.get("sha256"))
                    for entry in corpus_documents
                    if isinstance(entry, dict)
                }
                declared_map = {
                    _text(entry.get("id")): _text(entry.get("sha256"))
                    for entry in declared_documents
                    if isinstance(entry, dict)
                }
                if declared_map != pinned:
                    mismatch()
                if len(declared_documents) != _count(parity_pop.get("document_count")):
                    mismatch()
            _native_family_matrix_violations(first, parity_pop, mismatch)
        pooling = parity.get("pooling")
        if isinstance(pooling, dict) and pooling.get("citation_names_pooled_with_biblstruct") is True:
            mismatch()

    # ── exclusion: the 79 typed exclusions stay the denominator ─────────
    exclusion_entry = receipts.get("exclusion")
    exclusion_entry = exclusion_entry if isinstance(exclusion_entry, dict) else {}
    exclusion = exclusion_entry.get("document")
    exclusion_pop = (
        populations.get("exclusion") if isinstance(populations.get("exclusion"), dict) else {}
    )
    if isinstance(exclusion, dict):
        declaration = exclusion.get("declaration")
        declaration = declaration if isinstance(declaration, dict) else {}
        if _text(declaration.get("manifest")) != _carrier_path("holdout_manifests"):
            mismatch()
        if _text(declaration.get("pointer")) != _carrier_pointer("holdout_manifests"):
            mismatch()
        if _text(declaration.get("revision_sha256")) != _carrier_sha("holdout_manifests"):
            stale()
        probe = exclusion.get("probe")
        probe = probe if isinstance(probe, dict) else {}
        if _text(probe.get("manifest")) != _carrier_path("external_holdout_release_probe"):
            mismatch()
        if _text(probe.get("pointer")) != _carrier_pointer("external_holdout_release_probe"):
            mismatch()
        if _text(probe.get("revision_sha256")) != _carrier_sha("external_holdout_release_probe"):
            stale()
        baseline = exclusion.get("baseline")
        baseline = baseline if isinstance(baseline, dict) else {}
        if _text(baseline.get("manifest")) != _carrier_path("absorption_progress_baseline"):
            mismatch()
        if _text(baseline.get("pointer")) != _carrier_pointer("absorption_progress_baseline"):
            mismatch()
        if _text(baseline.get("revision_sha256")) != _carrier_sha(
            "absorption_progress_baseline"
        ):
            stale()

        population = exclusion.get("population")
        frozen = evidence.get("baseline_denominator")
        frozen = frozen if isinstance(frozen, dict) else {}
        frozen_reasons = frozen.get("exclusion_reasons")
        frozen_reasons = frozen_reasons if isinstance(frozen_reasons, dict) else {}
        excluded_count = (
            _count(population.get("excluded")) if isinstance(population, dict) else None
        )
        targeted_count = (
            _count(population.get("targeted")) if isinstance(population, dict) else None
        )
        if not isinstance(population, dict):
            mismatch()
        else:
            for key in ("assigned", "excluded", "measurable"):
                frozen_key = {"assigned": "assigned", "excluded": "excluded", "measurable": "executed"}[key]
                if _count(population.get(key)) != _count(exclusion_pop.get(key)):
                    mismatch()
                if _count(population.get(key)) != _count(frozen.get(frozen_key)):
                    mismatch()
            if _count(population.get("targeted")) != _count(exclusion_pop.get("targeted")):
                mismatch()
            if targeted_count is None or targeted_count != excluded_count:
                mismatch()
            reasons = population.get("reasons")
            expected_reasons = (
                exclusion_pop.get("reason_counts")
                if isinstance(exclusion_pop.get("reason_counts"), dict)
                else {}
            )
            if not isinstance(reasons, dict):
                mismatch()
            else:
                for reason, count in expected_reasons.items():
                    if _count(reasons.get(reason)) != _count(count) or _count(
                        frozen_reasons.get(reason)
                    ) != _count(count):
                        mismatch()
                for reason in reasons:
                    if reason not in expected_reasons:
                        mismatch()

        aggregates = exclusion.get("aggregates")
        total_denominator = 0
        aggregates_typed = True
        if not isinstance(aggregates, list) or not aggregates:
            aggregates_typed = False
        else:
            for row in aggregates:
                if not isinstance(row, dict):
                    aggregates_typed = False
                    continue
                denominator = _count(row.get("denominator"))
                parts = [
                    _count(row.get(key))
                    for key in ("recovered", "not_recovered", "out_of_scope", "typed_refusal")
                ]
                if (
                    denominator is None
                    or any(part is None for part in parts)
                    or sum(parts) != denominator
                ):
                    aggregates_typed = False
                if denominator is not None:
                    total_denominator += denominator
        if not aggregates_typed or excluded_count is None or total_denominator != excluded_count:
            violations.add("native_family_exclusion_denominator_dropped")

        recoveries = exclusion.get("recoveries")
        if (
            not isinstance(recoveries, list)
            or targeted_count is None
            or len(recoveries) != targeted_count
        ):
            violations.add("native_family_exclusion_denominator_dropped")
        else:
            disposition_vocabulary = set(_text_list(exclusion_pop.get("recovery_dispositions")))
            reason_vocabulary = set(expected_reasons)
            for row in recoveries:
                if not isinstance(row, dict):
                    mismatch()
                    continue
                reason = _text(row.get("reason"))
                disposition = _text(row.get("disposition"))
                if reason is None or (reason_vocabulary and reason not in reason_vocabulary):
                    mismatch()
                if disposition is None or (
                    disposition_vocabulary and disposition not in disposition_vocabulary
                ):
                    mismatch()
                if _text(row.get("document_id")) is None:
                    mismatch()
        if exclusion.get("integrity_failures"):
            mismatch()
    return violations

def _native_family_matrix_violations(
    population_block: dict[str, Any],
    parity_pop: dict[str, Any],
    mismatch,
) -> None:
    """A typed disposition for every family of every document in the matrix.

    The rule does not restate the family or region names (that would let the rule
    drift from the receipt); it requires that each document carries a non-empty
    family list whose every entry names a family and a disposition from the closed
    vocabulary, that every region names a region and a non-empty verdict, and that
    all documents agree on both name sets. A missing, untyped or inconsistent
    matrix is a population mismatch: the receipt would be publishing a population
    whose per-family shape it does not actually declare.
    """
    matrix = population_block.get("matrix")
    if not isinstance(matrix, list) or not matrix:
        mismatch()
        return
    family_dispositions = set(_text_list(parity_pop.get("family_dispositions")))
    ids: set[str] = set()
    family_sets: list[frozenset] = []
    region_sets: list[frozenset] = []
    for entry in matrix:
        if not isinstance(entry, dict):
            mismatch()
            continue
        identifier = _text(entry.get("id"))
        if identifier is None or identifier in ids:
            mismatch()
        else:
            ids.add(identifier)
        if _text(entry.get("sha256")) is None:
            mismatch()
        families = entry.get("families")
        if not isinstance(families, list) or not families:
            mismatch()
        else:
            names: set[str] = set()
            for family in families:
                if not isinstance(family, dict):
                    mismatch()
                    continue
                name = _text(family.get("family"))
                disposition = _text(family.get("disposition"))
                if (
                    name is None
                    or disposition is None
                    or (family_dispositions and disposition not in family_dispositions)
                ):
                    mismatch()
                else:
                    names.add(name)
            family_sets.append(frozenset(names))
        regions = entry.get("regions")
        if not isinstance(regions, list) or not regions:
            mismatch()
        else:
            region_names: set[str] = set()
            for region in regions:
                if not isinstance(region, dict):
                    mismatch()
                    continue
                name = _text(region.get("region"))
                verdict = _text(region.get("verdict"))
                if name is None or verdict is None:
                    mismatch()
                else:
                    region_names.add(name)
            region_sets.append(frozenset(region_names))
    if len(set(family_sets)) > 1:
        mismatch()
    if len(set(region_sets)) > 1:
        mismatch()


def _native_content_receipt_violations(contract: Any, evidence: Any) -> set[str]:
    """Refuse a native content receipt the committed carriers do not derive.

    The shape R141 forbids is a *claim* of source-bound native content progress —
    the body, section/reading-order, visual and callout bands measured against an
    independent oracle on the declared 3-document slice — with no receipt, or with
    a receipt that is stale, scoped outside the frozen population, guarded by
    nobody, scored by its own generator or by an oracle that carries only absences,
    with an untyped group partition, a required family silently moved to
    ``out_of_scope``, or a body gap whose D498 obligation is unresolved.
    Concretely:

    * ``claim_without_receipt`` — the S03 declaration resolves but the required
      receipt is absent or unreadable, so there is a claim and no evidence;
    * ``receipt_population_mismatch`` — the receipt is not the receipt of *this*
      rule (schema/declaration pointers differ), or its population, page
      denominator, document pin or per-document family matrix does not match the
      frozen corpus carriers, or a family/document row carries no typed
      disposition;
    * ``receipt_stale`` — a declared pin no longer matches the recomputed bytes
      (declaration, oracle carrier, preserved-TEI fixture or reviewed segmentation
      source), so it describes an older tree;
    * ``product_coverage_claimed`` — the claim is not diagnostic-only with no
      product-coverage effect and every promotion lock closed, publishes a
      threshold the acquisition does not accept, or leaks a redaction flag — the
      only scope that does not move the product ``java_independence`` axis (D499);
    * ``java_calls_nonzero`` — the receipt reports Java calls on a plane that
      claims none;
    * ``guard_not_installed`` — an observed zero is offered without the guard
      provenance (installed + identity-isolated observer) that would make the zero
      mean anything;
    * ``oracle_not_independent`` — the oracle is missing, not declared independent
      / same-generator-refused, names a generator or backend it does not carry,
      points at another pin carrier, or does not bind every preserved-TEI fixture;
    * ``oracle_absence_only`` — a required family has no positive oracle row
      anywhere on the slice (or the whole oracle carries none), so it could not
      discriminate a measured family from a missing one;
    * ``required_family_out_of_scope`` — a family cell carries the token the closed
      vocabulary deliberately has no member for, which is how a required family
      would otherwise be silently dropped from the denominator;
    * ``group_partition_untyped`` — the group partition is absent, untyped,
      non-total over the declared corpus, names a group or source outside the
      closed vocabulary, or pools the strata;
    * ``body_gap_unresolved`` — the body axis is absent, carries a token outside
      the closed verdict/reason vocabulary, is missing a numeric field, or records
      content loss (or absent character material) without the matching D498
      obligation.

    ``set()`` (empty) means clean. A missing or absent-evidence bundle is
    *unverified*, never clean and never refuted: a tree that does not carry the
    declaration authority is silence, not compliance, so nothing here is inferred
    from an artifact's mere existence.
    """
    violations: set[str] = set()
    if not isinstance(contract, dict) or not isinstance(evidence, dict) or not evidence:
        return violations
    if evidence.get("available") is not True:
        return violations
    rule = contract.get("native_content_receipt")
    if not isinstance(rule, dict):
        return violations

    expected = rule.get("expected") if isinstance(rule.get("expected"), dict) else {}
    populations_rule = (
        rule.get("populations") if isinstance(rule.get("populations"), dict) else {}
    )
    receipt_rule = rule.get("receipt") if isinstance(rule.get("receipt"), dict) else {}
    declaration_rule = (
        rule.get("declaration") if isinstance(rule.get("declaration"), dict) else {}
    )
    oracle_rule = rule.get("oracle") if isinstance(rule.get("oracle"), dict) else {}

    families_vocabulary = set(_text_list(expected.get("families")))
    required_families = _text_list(expected.get("required_families"))
    family_dispositions = set(_text_list(expected.get("family_dispositions")))
    unmet_reasons = set(_text_list(expected.get("unmet_reasons")))
    forbidden_token = _text(expected.get("forbidden_family_token"))
    body_verdicts = set(_text_list(expected.get("body_verdicts")))
    body_reasons = set(_text_list(expected.get("body_reasons")))
    group_vocabulary = set(_text_list(expected.get("group_vocabulary")))
    group_source_vocabulary = set(_text_list(expected.get("group_source_vocabulary")))
    partition_states = set(_text_list(expected.get("partition_states")))
    absent_reasons = set(_text_list(expected.get("bytes_absent_reasons")))
    expected_document_count = _count(populations_rule.get("document_count"))
    expected_page_denominator = _count(populations_rule.get("page_denominator"))

    def mismatch() -> None:
        violations.add("native_content_receipt_population_mismatch")

    def stale() -> None:
        violations.add("native_content_receipt_stale")

    def untyped_group() -> None:
        violations.add("native_content_group_partition_untyped")

    def unresolved_body() -> None:
        violations.add("native_content_body_gap_unresolved")

    def _bare(digest: Any) -> Optional[str]:
        """A digest with the ``sha256:`` prefix removed, for like-for-like comparison."""
        text = _text(digest)
        if text is None:
            return None
        return text.split("sha256:", 1)[1] if text.startswith("sha256:") else text

    def _declares_false(value: Any) -> bool:
        """True when a lock declares false in either JSON representation."""
        if value is False:
            return True
        return isinstance(value, str) and value.strip().lower() == "false"

    # ── a required receipt that does not exist is a claim without evidence ─
    receipt = evidence.get("receipt")
    if not isinstance(receipt, dict):
        if receipt_rule.get("required") is True:
            violations.add("native_content_claim_without_receipt")
        return violations

    # ── identity: the receipt must be the receipt of THIS rule ──────────
    if _text(receipt.get("schema_version")) != _text(receipt_rule.get("schema_version")):
        mismatch()
    declaration = receipt.get("declaration")
    declaration = declaration if isinstance(declaration, dict) else {}
    if _text(declaration.get("manifest")) != _text(declaration_rule.get("manifest")):
        mismatch()
    if _text(declaration.get("pointer")) != _text(declaration_rule.get("pointer")):
        mismatch()

    # ── scope, locks, thresholds, redaction ─────────────────────────────
    if _text(receipt.get("claim_scope")) != _text(expected.get("claim_scope")):
        violations.add("native_content_product_coverage_claimed")
    if _text(receipt.get("product_coverage_effect")) != _text(
        expected.get("product_coverage_effect")
    ):
        violations.add("native_content_product_coverage_claimed")
    locks = receipt.get("locks")
    if not isinstance(locks, dict):
        violations.add("native_content_product_coverage_claimed")
    elif (
        _text(locks.get("auto_promote")) != "forbidden"
        or not _declares_false(locks.get("deployed"))
        or _text(locks.get("graph_write")) != "forbidden"
        or not _declares_false(locks.get("import_eligible"))
        or _text(locks.get("production_route")) != "forbidden"
    ):
        violations.add("native_content_product_coverage_claimed")
    if _text(receipt.get("thresholds")) != _text(expected.get("thresholds")):
        violations.add("native_content_product_coverage_claimed")
    if any(key in receipt for key in ("threshold", "threshold_value", "numeric_threshold")):
        violations.add("native_content_product_coverage_claimed")
    redaction = receipt.get("redaction")
    if isinstance(redaction, dict) and any(value is True for value in redaction.values()):
        violations.add("native_content_product_coverage_claimed")

    # ── guard provenance: a zero is only evidence under an installed guard ─
    guard = receipt.get("guard")
    if not isinstance(guard, dict):
        violations.add("native_content_guard_not_installed")
    else:
        observed = _count(guard.get("observed_java_calls"))
        if observed is None or observed != 0:
            violations.add("native_content_java_calls_nonzero")
        elif (
            guard.get("installed") is not True
            or guard.get("identity_isolated") is not True
            or _text(guard.get("observer")) != _text(expected.get("guard_observer"))
        ):
            violations.add("native_content_guard_not_installed")

    # ── staleness: every declared pin against the recomputed carrier bytes ─
    if _text(declaration.get("revision_sha256")) != _text(evidence.get("declaration_sha256")):
        stale()

    # ── the independent oracle ──────────────────────────────────────────
    oracle = receipt.get("oracle")
    if not isinstance(oracle, dict):
        violations.add("native_content_oracle_not_independent")
    else:
        if (
            oracle.get("independent") is not True
            or oracle.get("same_generator_refused") is not True
            or _text(oracle.get("generator_id")) is None
            or _text(oracle.get("backend")) is None
        ):
            violations.add("native_content_oracle_not_independent")
        if _text(oracle.get("manifest")) != _text(oracle_rule.get("manifest")):
            violations.add("native_content_oracle_not_independent")
        if _text(oracle.get("segmentation_manifest")) != _text(
            oracle_rule.get("segmentation_manifest")
        ):
            violations.add("native_content_oracle_not_independent")
        if _text(oracle.get("segmentation_source")) != _text(
            oracle_rule.get("segmentation_source")
        ):
            violations.add("native_content_oracle_not_independent")
        if _text(oracle.get("revision_sha256")) != _text(evidence.get("oracle_sha256")):
            stale()
        if _text(oracle.get("segmentation_revision_sha256")) != _text(
            evidence.get("segmentation_manifest_sha256")
        ):
            stale()
        if _text(oracle.get("segmentation_source_sha256")) != _text(
            evidence.get("segmentation_source_sha256")
        ):
            stale()
        pins = evidence.get("oracle_fixture_pins")
        pins = pins if isinstance(pins, dict) else {}
        documents = oracle.get("documents")
        if not isinstance(documents, list) or not documents:
            violations.add("native_content_oracle_not_independent")
        else:
            for document in documents:
                if not isinstance(document, dict):
                    violations.add("native_content_oracle_not_independent")
                    continue
                fixture_path = _text(document.get("fixture_path"))
                declared_sha = _text(document.get("sha256"))
                if (
                    _text(document.get("document_id")) is None
                    or fixture_path is None
                    or declared_sha is None
                ):
                    violations.add("native_content_oracle_not_independent")
                    continue
                pin = pins.get(fixture_path)
                if not isinstance(pin, dict):
                    violations.add("native_content_oracle_not_independent")
                    continue
                if _bare(pin.get("sha256")) != _bare(declared_sha) or _bare(
                    pin.get("recomputed_sha256")
                ) != _bare(declared_sha):
                    stale()

    # ── the group-stratified populations and the declared corpus pin ────
    matrix_rows: list[dict[str, Any]] = []
    declared_pins: dict[str, Optional[str]] = {}
    populations = receipt.get("populations")
    if not isinstance(populations, list) or not populations:
        mismatch()
    else:
        total_documents = 0
        total_pages = 0
        for population in populations:
            if not isinstance(population, dict):
                mismatch()
                continue
            documents = population.get("documents")
            matrix = population.get("matrix")
            if not isinstance(documents, list) or not documents:
                mismatch()
                continue
            if _count(population.get("document_count")) != len(documents):
                mismatch()
            page_sum = 0
            for document in documents:
                if not isinstance(document, dict):
                    mismatch()
                    continue
                identifier = _text(document.get("id"))
                if identifier is None or identifier in declared_pins:
                    mismatch()
                else:
                    declared_pins[identifier] = _text(document.get("sha256"))
                if _text(document.get("sha256")) is None or _text(
                    document.get("source_pdf")
                ) is None:
                    mismatch()
                pages = _count(document.get("pages"))
                if pages is None:
                    mismatch()
                else:
                    page_sum += pages
            if _count(population.get("page_denominator")) != page_sum:
                mismatch()
            total_documents += len(documents)
            total_pages += page_sum
            if not isinstance(matrix, list) or len(matrix) != len(documents):
                mismatch()
                continue
            for row in matrix:
                matrix_rows.append(row if isinstance(row, dict) else {})
        if expected_document_count is not None and total_documents != expected_document_count:
            mismatch()
        if (
            expected_page_denominator is not None
            and total_pages != expected_page_denominator
        ):
            mismatch()
        if _count(evidence.get("corpus_document_count")) != total_documents:
            mismatch()
        if _count(evidence.get("corpus_page_denominator")) != total_pages:
            mismatch()
        corpus_documents = evidence.get("corpus_documents")
        if isinstance(corpus_documents, list):
            corpus_pins = {
                _text(entry.get("id")): _text(entry.get("sha256"))
                for entry in corpus_documents
                if isinstance(entry, dict)
            }
            if corpus_pins and corpus_pins != declared_pins:
                mismatch()

    # ── the per-document family matrix: typed disposition for every family ─
    observed_family_names: set[str] = set()
    for row in matrix_rows:
        if not row:
            mismatch()
            continue
        if _text(row.get("id")) is None or _text(row.get("sha256")) is None:
            mismatch()
        families = row.get("families")
        if not isinstance(families, list) or not families:
            mismatch()
        else:
            names: set[str] = set()
            for cell in families:
                if not isinstance(cell, dict):
                    mismatch()
                    continue
                name = _text(cell.get("family"))
                disposition = _text(cell.get("disposition"))
                if name is None or disposition is None:
                    mismatch()
                    continue
                names.add(name)
                if disposition == forbidden_token:
                    violations.add("native_content_required_family_out_of_scope")
                    continue
                if disposition not in family_dispositions:
                    mismatch()
                    continue
                reason = _text(cell.get("unmet_reason"))
                count = _count(cell.get("observed_count"))
                if disposition == "measured":
                    if count is None or reason is not None:
                        mismatch()
                elif disposition == "unmet":
                    if count is not None or reason is None or reason not in unmet_reasons:
                        mismatch()
                elif disposition == "not_observed":
                    if count is not None or reason is not None:
                        mismatch()
                if _count(cell.get("oracle_positive_rows")) is None:
                    mismatch()
            observed_family_names |= names
            if families_vocabulary and names != families_vocabulary:
                mismatch()

    # ── oracle non-vacuity: a required family must carry a positive row ──
    def _all_cells() -> list[dict[str, Any]]:
        cells: list[dict[str, Any]] = []
        for row in matrix_rows:
            families = row.get("families")
            if isinstance(families, list):
                cells.extend(cell for cell in families if isinstance(cell, dict))
        return cells

    cells = _all_cells()
    total_positive = sum(_count(cell.get("oracle_positive_rows")) or 0 for cell in cells)
    if matrix_rows and total_positive == 0:
        violations.add("native_content_oracle_absence_only")
    for family in required_families:
        positive = sum(
            _count(cell.get("oracle_positive_rows")) or 0
            for cell in cells
            if _text(cell.get("family")) == family
        )
        if positive == 0:
            violations.add("native_content_oracle_absence_only")

    # ── the body axis: a typed gap or a named D498 obligation ───────────
    for row in matrix_rows:
        body = row.get("body")
        if not isinstance(body, dict):
            unresolved_body()
            continue
        plane = _text(body.get("plane_preservation"))
        region = _text(body.get("body_region"))
        reason = _text(body.get("body_reason"))
        if plane not in body_verdicts or region not in body_verdicts or reason not in body_reasons:
            unresolved_body()
        loss = plane == "content_loss"
        if loss != (body.get("plane_d498_reopen_required") is True):
            unresolved_body()
        absent = _count(body.get("plane_absent_units"))
        if absent is None:
            unresolved_body()
        elif absent > 0 and body.get("plane_d498_reopen_required") is not True:
            unresolved_body()
        for key in (
            "plane_present_in_token_units",
            "plane_spanning_token_units",
            "plane_character_material_units",
            "body_token_count",
            "oracle_units",
            "native_plane_units",
        ):
            if _count(body.get(key)) is None:
                unresolved_body()

    # ── the total group partition ───────────────────────────────────────
    partition = receipt.get("partition")
    if not isinstance(partition, dict):
        untyped_group()
    else:
        if _text(partition.get("state")) not in partition_states:
            untyped_group()
        if _count(partition.get("declared_document_count")) != expected_document_count:
            untyped_group()
        assignments = partition.get("assignments")
        assigned_ids: set[str] = set()
        if not isinstance(assignments, list) or not assignments:
            untyped_group()
        else:
            for assignment in assignments:
                if not isinstance(assignment, dict):
                    untyped_group()
                    continue
                document_id = _text(assignment.get("document_id"))
                if document_id is None or document_id in assigned_ids:
                    untyped_group()
                else:
                    assigned_ids.add(document_id)
                source = assignment.get("source")
                source = source if isinstance(source, dict) else {}
                kind = _text(source.get("kind"))
                group = _text(assignment.get("group"))
                if kind not in group_source_vocabulary:
                    untyped_group()
                    continue
                if kind == "bytes_absent":
                    if group is not None or _text(source.get("reason")) not in absent_reasons:
                        untyped_group()
                elif (
                    group not in group_vocabulary
                    or _text(source.get("pdf_type")) is None
                    or _text(source.get("pdf_sha256")) is None
                ):
                    untyped_group()
        if declared_pins and assigned_ids != set(declared_pins):
            untyped_group()
    pooling = receipt.get("pooling")
    if isinstance(pooling, dict) and pooling.get("group_strata_pooled") is True:
        untyped_group()

    # ── every matrix row carries its own typed group evidence ───────────
    for row in matrix_rows:
        row_group = row.get("group")
        if not isinstance(row_group, dict):
            untyped_group()
            continue
        row_source = row_group.get("source")
        row_source = row_source if isinstance(row_source, dict) else {}
        kind = _text(row_source.get("kind"))
        group = _text(row_group.get("group"))
        if _text(row_group.get("document_id")) != _text(row.get("id")):
            untyped_group()
        elif kind == "bytes_absent":
            if group is not None or _text(row_source.get("reason")) not in absent_reasons:
                untyped_group()
        elif kind == "measured":
            if group not in group_vocabulary:
                untyped_group()
        else:
            untyped_group()

    return violations


def _native_route_violations(contract: Any, evidence: Any) -> set[str]:
    """Refuse a native-route claim the two S06 receipts do not evidence.

    The shape R141 forbids is a *claim* of native-route progress — one PDF→native
    result on the declared 3-document / 202-page slice, with no Java and no prior
    TEI, measured against a ceiling frozen *before* the benchmark and paired with a
    same-n reference — that is backed by no receipt, by a receipt that drifted from
    the corpus/boundary it claims, by a pin that no longer matches the carrier
    bytes, or by a population, stage, family, body or metric cell the rule left
    untyped. Concretely:

    * ``claim_without_receipt`` — the S03 declaration resolves but the required
      route receipt is absent or unreadable, so there is a claim and no evidence;
    * ``receipt_population_mismatch`` — the receipt is not the receipt of *this*
      rule, its document pins do not reproduce the corpus carrier (identity or
      page count), its population documents do not partition that corpus, its
      per-document document/page counts are internally inconsistent, or its
      residual closure counters do not re-derive from its own stage/family cells;
    * ``receipt_stale`` — a declared pin (the declaration revision on either
      receipt, or the frozen limit revision) no longer matches the recomputed
      carrier bytes, so it describes an older tree;
    * ``product_coverage_claimed`` — the claim is not diagnostic-only with no
      product-coverage effect and every promotion lock closed, publishes a
      threshold the acquisition does not accept, or leaks a redaction flag;
    * ``java_calls_nonzero`` — either receipt reports Java calls on a plane that
      claims none;
    * ``guard_not_installed`` — an observed zero is offered without the installed,
      identity-isolated transport observer that would make the zero mean anything,
      or by an observer other than the declared one;
    * ``limit_not_frozen`` — the ceiling is not declared frozen/prospective, the
      claim-without-freeze or pass-requires-frozen invariant is missing, the
      direction vocabulary drifted, or the frozen metric set is not mirrored;
    * ``limit_tuned_after_scoring`` — a post-hoc tuning/clamp affordance is open,
      or the zero-is-not-a-measurement invariant is dropped;
    * ``stage_outside_slice_counted`` — a document's stage cells do not cover the
      cascade, a stage outside the slice is counted as measured, the admission
      stage is not the boundary, or a stance leaves the closed vocabulary;
    * ``family_out_of_scope`` — a family cell carries the token the closed
      vocabulary deliberately has no member for, carries a disposition/reason
      outside its vocabulary, or an unclassified cell appears;
    * ``body_gap_untyped`` — the body axis is absent, carries a token outside the
      closed verdict/reason vocabulary, is missing a numeric field, or records
      content loss (or absent character material) without the matching D498
      obligation;
    * ``resource_pair_unbound`` — the required same-n resource receipt is absent,
      its pairing is not the declared same-documents/hard-failure contract, its
      corpus is not the route corpus, or a measured reference row names no
      repository-relative carrier whose bytes still match the pin;
    * ``metric_untyped`` — a route or resource metric row names an id outside the
      declared vocabulary, an observation state outside the closed pair, or an
      absent row without a reason from the closed absence vocabulary.

    ``set()`` (empty) means clean. A missing or absent-evidence bundle is
    *unverified*, never clean and never refuted: a tree that does not carry the
    declaration authority is silence, not compliance, so nothing here is inferred
    from an artifact's mere existence.
    """
    violations: set[str] = set()
    if not isinstance(contract, dict) or not isinstance(evidence, dict) or not evidence:
        return violations
    if evidence.get("available") is not True:
        return violations
    rule = contract.get("native_route_e2e")
    if not isinstance(rule, dict):
        return violations

    def _mapping(value: Any) -> dict[str, Any]:
        return value if isinstance(value, dict) else {}

    expected = _mapping(rule.get("expected"))
    limit_policy = _mapping(rule.get("limit_policy"))
    receipt_rule = _mapping(rule.get("receipt"))
    resource_rule = _mapping(rule.get("resource_receipt"))
    declaration_rule = _mapping(rule.get("declaration"))
    corpus_rule = _mapping(rule.get("corpus"))

    claim_scope = _text(expected.get("claim_scope"))
    product_coverage = _text(expected.get("product_coverage_effect"))
    thresholds = _text(expected.get("thresholds"))
    guard_observer = _text(expected.get("guard_observer"))
    locks_rule = _mapping(expected.get("locks"))
    population_vocabulary = set(_text_list(expected.get("population_vocabulary")))
    resource_population_vocabulary = set(
        _text_list(expected.get("resource_population_vocabulary"))
    )
    observation_states = set(_text_list(expected.get("observation_state_vocabulary")))
    absence_reasons = set(_text_list(expected.get("absence_reason_vocabulary")))
    native_metrics = set(_text_list(expected.get("native_metric_vocabulary")))
    reference_metrics = set(_text_list(expected.get("reference_metric_vocabulary")))
    reference_source_kind = _text(expected.get("reference_source_kind"))
    pairing_rule = _mapping(expected.get("pairing_vocabulary"))
    body_verdicts = set(_text_list(expected.get("body_verdict_vocabulary")))
    body_reasons = set(_text_list(expected.get("body_reason_vocabulary")))
    stage_stances = set(_text_list(expected.get("stage_stance_vocabulary")))
    out_of_slice_stance = _text(expected.get("out_of_slice_stance"))
    admission_stance = _text(expected.get("admission_stance"))
    family_dispositions = set(_text_list(expected.get("family_disposition_vocabulary")))
    family_unmet_reasons = set(_text_list(expected.get("family_unmet_reason_vocabulary")))
    family_refusal_reasons = set(_text_list(expected.get("family_refusal_reason_vocabulary")))
    forbidden_family_token = _text(expected.get("forbidden_family_token"))

    freeze_state = _text(limit_policy.get("state"))
    freeze_scope = _text(limit_policy.get("freeze_scope"))
    direction_vocabulary = set(_text_list(limit_policy.get("direction_vocabulary")))
    tuned_forbidden = _text(limit_policy.get("tuned_after_scoring"))
    silent_clamp_forbidden = _text(limit_policy.get("silent_clamp"))
    claim_without_freeze_forbidden = _text(limit_policy.get("claim_without_freeze"))

    freeze_metric_ids = set(_text_list(evidence.get("freeze_metric_ids")))
    recomputed_declaration = _text(evidence.get("declaration_sha256"))
    recomputed_freeze = _text(evidence.get("freeze_revision_recomputed"))
    cascade_stages = set(_text_list(evidence.get("stage_ids")))
    out_of_slice_stages = set(_text_list(evidence.get("stages_out_of_slice")))
    admission_stage = _text(evidence.get("admission_stage"))

    corpus_pins: dict[str, tuple[Optional[str], Optional[int]]] = {}
    corpus_documents = evidence.get("corpus_documents")
    if isinstance(corpus_documents, list):
        for entry in corpus_documents:
            if not isinstance(entry, dict):
                continue
            doc_id = _text(entry.get("id"))
            if doc_id is not None:
                corpus_pins[doc_id] = (_text(entry.get("sha256")), _count(entry.get("pages")))
    corpus_count = _count(evidence.get("corpus_document_count"))
    corpus_pages = _count(evidence.get("corpus_page_denominator"))

    def claim_without_receipt() -> None:
        violations.add("native_route_claim_without_receipt")

    def mismatch() -> None:
        violations.add("native_route_receipt_population_mismatch")

    def stale() -> None:
        violations.add("native_route_receipt_stale")

    def coverage() -> None:
        violations.add("native_route_product_coverage_claimed")

    def java_nonzero() -> None:
        violations.add("native_route_java_calls_nonzero")

    def guard_missing() -> None:
        violations.add("native_route_guard_not_installed")

    def not_frozen() -> None:
        violations.add("native_route_limit_not_frozen")

    def tuned() -> None:
        violations.add("native_route_limit_tuned_after_scoring")

    def stage_counted() -> None:
        violations.add("native_route_stage_outside_slice_counted")

    def family_out_of_scope() -> None:
        violations.add("native_route_family_out_of_scope")

    def body_untyped() -> None:
        violations.add("native_route_body_gap_untyped")

    def pair_unbound() -> None:
        violations.add("native_route_resource_pair_unbound")

    def metric_untyped() -> None:
        violations.add("native_route_metric_untyped")

    def declares_false(value: Any) -> bool:
        """True when a lock declares false in either JSON representation."""
        if value is False:
            return True
        return isinstance(value, str) and value.strip().lower() == "false"

    def bare_digest(value: Any) -> Optional[str]:
        """A sha256 with any ``sha256:`` prefix removed, for like-for-like compare."""
        text = _text(value)
        if text is None:
            return None
        return text.split("sha256:", 1)[1] if text.startswith("sha256:") else text

    def check_limits(limits: Any) -> None:
        """The frozen-ceiling declaration both receipts must mirror."""
        if not isinstance(limits, dict):
            not_frozen()
            return
        revision = _text(limits.get("freeze_revision"))
        if revision is None:
            not_frozen()
        elif recomputed_freeze is not None and revision != recomputed_freeze:
            stale()
        if _text(limits.get("freeze_state")) != freeze_state:
            not_frozen()
        if _text(limits.get("freeze_scope")) != freeze_scope:
            not_frozen()
        if _text(limits.get("claim_without_freeze")) != claim_without_freeze_forbidden:
            not_frozen()
        if limits.get("pass_requires_frozen_threshold") is not True:
            not_frozen()
        if set(_text_list(limits.get("direction_vocabulary"))) != direction_vocabulary:
            not_frozen()
        metrics = limits.get("metrics")
        if not isinstance(metrics, list) or not metrics:
            not_frozen()
        else:
            ids = {_text(entry.get("id")) for entry in metrics if isinstance(entry, dict)}
            if ids != freeze_metric_ids:
                not_frozen()
        if _text(limits.get("tuned_after_scoring")) != tuned_forbidden:
            tuned()
        if _text(limits.get("silent_clamp")) != silent_clamp_forbidden:
            tuned()
        if limits.get("zero_is_not_a_measurement") is not True:
            tuned()

    def check_envelope(document: Any, schema_version: Optional[str]) -> None:
        """The claim/locks/redaction/guard/limits envelope both receipts share."""
        if _text(document.get("schema_version")) != schema_version:
            mismatch()
        if _text(document.get("claim_scope")) != claim_scope:
            coverage()
        if _text(document.get("product_coverage_effect")) != product_coverage:
            coverage()
        if _text(document.get("thresholds")) != thresholds:
            coverage()
        locks = document.get("locks")
        if not isinstance(locks, dict):
            coverage()
        else:
            for lock, want in locks_rule.items():
                found = locks.get(lock)
                if isinstance(want, bool):
                    if want is False and not declares_false(found):
                        coverage()
                    elif want is True and found is not True:
                        coverage()
                elif _text(want) is None or _text(found) != _text(want):
                    coverage()
        redaction = document.get("redaction")
        if not isinstance(redaction, dict):
            coverage()
        else:
            for flag in (
                "raw_text",
                "raw_pdf",
                "tei_xml",
                "images",
                "physical_paths",
                "hostname",
                "executable",
            ):
                if redaction.get(flag) is True:
                    coverage()
        guard = document.get("guard")
        if not isinstance(guard, dict):
            guard_missing()
        else:
            calls = _count(guard.get("observed_java_calls"))
            if calls is None:
                guard_missing()
            elif calls > 0:
                java_nonzero()
            if _text(guard.get("observer")) != guard_observer:
                guard_missing()
            if guard.get("installed") is not True or guard.get("identity_isolated") is not True:
                guard_missing()
        declaration = document.get("declaration")
        if isinstance(declaration, dict):
            declared_revision = _text(declaration.get("revision_sha256"))
            if (
                declared_revision is not None
                and recomputed_declaration is not None
                and declared_revision != recomputed_declaration
            ):
                stale()
        check_limits(document.get("limits"))

    def check_body(body: Any) -> None:
        """One typed body axis, including the D498 obligation for content loss."""
        if not isinstance(body, dict):
            body_untyped()
            return
        verdict = _text(body.get("plane_preservation"))
        reason = _text(body.get("plane_gap_reason"))
        if verdict is None or verdict not in body_verdicts:
            body_untyped()
        if reason is not None and reason not in body_reasons:
            body_untyped()
        for key in (
            "plane_character_material_units",
            "plane_absent_units",
            "plane_present_in_token_units",
            "plane_spanning_token_units",
        ):
            if _count(body.get(key)) is None:
                body_untyped()
        absent = _count(body.get("plane_absent_units"))
        reopen = body.get("plane_d498_reopen_required")
        if verdict == "content_loss" and reopen is not True:
            body_untyped()
        if absent is not None and absent > 0 and reopen is not True:
            body_untyped()

    def check_metric(metric: Any, vocabulary: set[str]) -> None:
        """One observed/absent metric row, typed on both axes."""
        if not isinstance(metric, dict):
            metric_untyped()
            return
        metric_id = _text(metric.get("id"))
        if metric_id is None or metric_id not in vocabulary:
            metric_untyped()
        state = _text(metric.get("state"))
        if state not in observation_states:
            metric_untyped()
            return
        if state == "observed":
            if _count(metric.get("value")) is None or metric.get("reason") is not None:
                metric_untyped()
        else:
            reason = _text(metric.get("reason"))
            if reason is None or reason not in absence_reasons:
                metric_untyped()

    def check_reference_row(row: Any) -> None:
        """A same-n reference row: a closed id/state plus a pinned measured source."""
        if not isinstance(row, dict):
            metric_untyped()
            return
        metric_id = _text(row.get("id"))
        if metric_id is None or metric_id not in reference_metrics:
            metric_untyped()
        state = _text(row.get("state"))
        if state not in observation_states:
            metric_untyped()
            return
        if state == "observed":
            source = row.get("source")
            if not isinstance(source, dict):
                pair_unbound()
                return
            if _text(source.get("kind")) != reference_source_kind:
                pair_unbound()
            if not _is_relative_path(source.get("manifest")):
                pair_unbound()
            if _text(source.get("sha256")) is None:
                pair_unbound()
        else:
            reason = _text(row.get("reason"))
            if reason is None or reason not in absence_reasons:
                metric_untyped()

    # ── the route receipt: identity, scope and the typed 3-document result ──
    receipt = evidence.get("receipt")
    if not isinstance(receipt, dict):
        if receipt_rule.get("required") is True:
            claim_without_receipt()
    else:
        check_envelope(receipt, _text(receipt_rule.get("schema_version")))
        declaration = _mapping(receipt.get("declaration"))
        if _text(declaration.get("manifest")) != _text(declaration_rule.get("manifest")):
            mismatch()
        if _text(declaration.get("pointer")) != _text(declaration_rule.get("pointer")):
            mismatch()
        if _count(corpus_rule.get("document_count")) is not None and (
            corpus_count != _count(corpus_rule.get("document_count"))
        ):
            mismatch()
        if _count(corpus_rule.get("page_denominator")) is not None and (
            corpus_pages != _count(corpus_rule.get("page_denominator"))
        ):
            mismatch()

        seen_documents: set[str] = set()
        unmeasured_stages = 0
        unclassified_families = 0
        documents = receipt.get("documents")
        if not isinstance(documents, list) or not documents:
            mismatch()
        else:
            for document in documents:
                if not isinstance(document, dict):
                    mismatch()
                    continue
                doc_id = _text(document.get("id"))
                if doc_id is None or doc_id in seen_documents:
                    mismatch()
                    continue
                seen_documents.add(doc_id)
                pin = corpus_pins.get(doc_id)
                if pin is None:
                    mismatch()
                elif (
                    _text(document.get("sha256")) != pin[0]
                    or _count(document.get("pages")) != pin[1]
                ):
                    mismatch()

                stages = document.get("stages")
                if not isinstance(stages, list) or not stages:
                    stage_counted()
                else:
                    stage_ids: set[str] = set()
                    for cell in stages:
                        if not isinstance(cell, dict):
                            stage_counted()
                            continue
                        stage_id = _text(cell.get("stage"))
                        stance = _text(cell.get("stance"))
                        if stage_id is None or stance is None or stance not in stage_stances:
                            stage_counted()
                        else:
                            stage_ids.add(stage_id)
                            if stance != "native_measured":
                                unmeasured_stages += 1
                            if stage_id in out_of_slice_stages and stance != out_of_slice_stance:
                                stage_counted()
                            if stage_id == admission_stage and stance != admission_stance:
                                stage_counted()
                    if cascade_stages and stage_ids != cascade_stages:
                        stage_counted()

                families = document.get("families")
                if not isinstance(families, list) or not families:
                    family_out_of_scope()
                else:
                    for cell in families:
                        if not isinstance(cell, dict):
                            family_out_of_scope()
                            continue
                        family = _text(cell.get("family"))
                        disposition = _text(cell.get("disposition"))
                        if family is None or disposition is None:
                            family_out_of_scope()
                            continue
                        if disposition == forbidden_family_token:
                            family_out_of_scope()
                            continue
                        if disposition not in family_dispositions:
                            unclassified_families += 1
                            family_out_of_scope()
                            continue
                        if disposition == "measured":
                            if _count(cell.get("observed_count")) is None:
                                family_out_of_scope()
                        elif disposition == "unmet":
                            reason = _text(cell.get("reason"))
                            if reason is None or reason not in family_unmet_reasons:
                                family_out_of_scope()
                            if _count(cell.get("observed_count")) is not None:
                                family_out_of_scope()
                        elif disposition == "refused":
                            reason = _text(cell.get("reason"))
                            if reason is None or reason not in family_refusal_reasons:
                                family_out_of_scope()

                check_body(document.get("body"))

        if corpus_pins and set(corpus_pins) - seen_documents:
            mismatch()

        populations = receipt.get("populations")
        if not isinstance(populations, list) or not populations:
            mismatch()
        else:
            population_seen: set[str] = set()
            for population in populations:
                if not isinstance(population, dict):
                    mismatch()
                    continue
                token = _text(population.get("population"))
                if token not in population_vocabulary:
                    mismatch()
                    continue
                population_documents = population.get("documents")
                if not isinstance(population_documents, list) or not population_documents:
                    mismatch()
                    continue
                if _count(population.get("document_count")) != len(population_documents):
                    mismatch()
                page_sum = 0
                for entry in population_documents:
                    if not isinstance(entry, dict):
                        mismatch()
                        continue
                    entry_id = _text(entry.get("id"))
                    entry_pin = corpus_pins.get(entry_id)
                    if entry_pin is None or (
                        _text(entry.get("sha256")) != entry_pin[0]
                        or _count(entry.get("pages")) != entry_pin[1]
                    ):
                        mismatch()
                    pages = _count(entry.get("pages"))
                    if pages is not None:
                        page_sum += pages
                    if entry_id is not None:
                        if entry_id in population_seen:
                            mismatch()
                        population_seen.add(entry_id)
                if _count(population.get("page_denominator")) != page_sum:
                    mismatch()
                metrics = population.get("metrics")
                if not isinstance(metrics, list) or not metrics:
                    metric_untyped()
                else:
                    for metric in metrics:
                        if not isinstance(metric, dict):
                            metric_untyped()
                            continue
                        metric_id = _text(metric.get("id"))
                        if metric_id is None or metric_id not in freeze_metric_ids:
                            metric_untyped()
                        if _text(metric.get("direction")) not in direction_vocabulary:
                            metric_untyped()
                        if metric_id == "native_route_java_calls":
                            value = _count(metric.get("value"))
                            if value is not None and value > 0:
                                java_nonzero()
            if corpus_pins and set(corpus_pins) - population_seen:
                mismatch()

        residuals = receipt.get("residuals")
        if not isinstance(residuals, dict):
            stage_counted()
            family_out_of_scope()
        else:
            if _count(residuals.get("unmeasured_stages")) != unmeasured_stages:
                stage_counted()
            if _count(residuals.get("unclassified_families")) != unclassified_families:
                family_out_of_scope()
            check_body(residuals.get("body"))

    # ── the same-n resource receipt: pairing + typed reference rows ───────
    resource = evidence.get("resource_receipt")
    if not isinstance(resource, dict):
        if resource_rule.get("required") is True:
            pair_unbound()
    else:
        check_envelope(resource, _text(resource_rule.get("schema_version")))
        r_declaration = _mapping(resource.get("declaration"))
        if _text(r_declaration.get("manifest")) != _text(declaration_rule.get("manifest")):
            pair_unbound()
        if _text(r_declaration.get("pointer")) != _text(declaration_rule.get("pointer")):
            pair_unbound()

        pairing = resource.get("pairing")
        if not isinstance(pairing, dict):
            pair_unbound()
        else:
            for key in ("rule", "pooled", "saving_claim", "mismatch_disposition"):
                want = _text(pairing_rule.get(key))
                if want is None or _text(pairing.get(key)) != want:
                    pair_unbound()

        r_corpus = resource.get("corpus")
        if not isinstance(r_corpus, dict):
            pair_unbound()
        else:
            if _count(r_corpus.get("document_count")) != len(corpus_pins):
                pair_unbound()
            if _count(r_corpus.get("page_denominator")) != corpus_pages:
                pair_unbound()
            r_pins: dict[str, tuple[Optional[str], Optional[int]]] = {}
            r_corpus_documents = r_corpus.get("documents")
            if isinstance(r_corpus_documents, list):
                for entry in r_corpus_documents:
                    if not isinstance(entry, dict):
                        continue
                    entry_id = _text(entry.get("id"))
                    if entry_id is not None:
                        r_pins[entry_id] = (
                            _text(entry.get("sha256")),
                            _count(entry.get("pages")),
                        )
            if r_pins != corpus_pins:
                pair_unbound()

        r_populations = resource.get("populations")
        if not isinstance(r_populations, list) or not r_populations:
            pair_unbound()
        else:
            for population in r_populations:
                if not isinstance(population, dict):
                    pair_unbound()
                    continue
                token = _text(population.get("population"))
                if token not in resource_population_vocabulary:
                    pair_unbound()
                    continue
                if _count(population.get("document_count")) != corpus_count:
                    pair_unbound()
                if _count(population.get("page_denominator")) != corpus_pages:
                    pair_unbound()
                population_documents = population.get("documents")
                if isinstance(population_documents, list):
                    r_pop_pins = {}
                    for entry in population_documents:
                        if not isinstance(entry, dict):
                            continue
                        entry_id = _text(entry.get("id"))
                        if entry_id is not None:
                            r_pop_pins[entry_id] = (
                                _text(entry.get("sha256")),
                                _count(entry.get("pages")),
                            )
                    if r_pop_pins != corpus_pins:
                        pair_unbound()
                else:
                    pair_unbound()
                vocabulary = native_metrics if token == "native" else reference_metrics
                metrics = population.get("metrics")
                if not isinstance(metrics, list) or not metrics:
                    metric_untyped()
                else:
                    for metric in metrics:
                        check_metric(metric, vocabulary)

        r_documents = resource.get("documents")
        if not isinstance(r_documents, list) or not r_documents:
            pair_unbound()
        else:
            for document in r_documents:
                if not isinstance(document, dict):
                    pair_unbound()
                    continue
                doc_id = _text(document.get("id"))
                pin = corpus_pins.get(doc_id)
                if pin is None or (
                    _text(document.get("sha256")) != pin[0]
                    or _count(document.get("pages")) != pin[1]
                ):
                    pair_unbound()
                rows = document.get("reference")
                if not isinstance(rows, list) or not rows:
                    metric_untyped()
                else:
                    for row in rows:
                        check_reference_row(row)

        pins = evidence.get("resource_reference_pins")
        if not isinstance(pins, dict) or not pins:
            pair_unbound()
        else:
            for pin in pins.values():
                if not isinstance(pin, dict):
                    pair_unbound()
                    continue
                declared_sha = bare_digest(pin.get("declared_sha256"))
                recomputed_sha = bare_digest(pin.get("recomputed_sha256"))
                if declared_sha is None or recomputed_sha is None or declared_sha != recomputed_sha:
                    pair_unbound()

        r_residuals = resource.get("residuals")
        if not isinstance(r_residuals, dict) or _count(r_residuals.get("untyped_metrics")) != 0:
            metric_untyped()

    return violations


def _acceptance_package_violations(contract: Any, evidence: Any) -> set[str]:
    """Refuse an acceptance-package claim the package does not evidence.

    The shape R141 forbids is a *claim* of delivery progress — an acceptance
    package that says the S08 gates are closed and S09 may proceed — that is
    backed by no package, by a package whose carrier pins no longer match the
    committed bytes, by a threshold verdict detached from the frozen reference,
    by a silently dropped mandatory gate, by an S09 authorization while a gate
    still blocks, by a renamed body residual, by an authorization read from a GSD
    lifecycle state, by a pooled or saving resource claim, or by a rollback claim
    with no byte-identity evidence. Concretely:

    * ``claim_without_package`` — the S03 declaration resolves but the required
      package is absent or unreadable, so there is a claim and no evidence;
    * ``schema_drift`` — the package names a schema version other than this
      rule's;
    * ``evidence_drift`` — a binding is not a repo-relative path + pointer +
      sha256, a binding repeats, or the bound carrier set is not the declared
      one;
    * ``carrier_stale`` — a declared carrier pin no longer matches the recomputed
      bytes, so the package describes an older tree;
    * ``coverage_claimed`` — the claim is not diagnostic-only with no
      product-coverage effect and every lock closed, or it publishes a threshold
      the acquisition does not accept, or it leaks a redaction flag;
    * ``threshold_unbound`` — the evaluation is not frozen, names another
      reference or candidate, or a verdict's id/family/metric/stratum/value no
      longer re-derives from the pinned reference fraction;
    * ``mandatory_gate_dropped`` — a gate is untyped, its ``blocks_s09`` disagrees
      with its own state, a required gate id is missing, a gate repeats, or the
      ledger is smaller than the declared minimum;
    * ``body_residual_renamed`` — the body-axis gate is absent, its state is not
      the declared verdict, or the residual wording no longer appears verbatim;
    * ``unmet_gate_authorized`` — the package authorizes S09 while a gate still
      blocks;
    * ``authorization_drift`` — the declared blocking set disagrees with the
      recomputed one, the basis drifted, or S09 is refused while nothing blocks;
    * ``lifecycle_authorization`` — the package derives authorization from a GSD
      lifecycle state;
    * ``pooled_resource_claim`` — the same-n resource pairing is not the declared
      same-documents contract, or a reference metric is not the typed absence it
      must be;
    * ``rollback_without_byte_identity`` — the rollback rehearsal does not carry
      byte identity, leaks a native fragment, or has a refused row with no typed
      reason.

    ``set()`` (empty) means clean. A missing or absent-evidence bundle is
    *unverified*, never clean and never refuted: a tree that does not carry the
    declaration authority is silence, not compliance.
    """
    violations: set[str] = set()
    if not isinstance(contract, dict) or not isinstance(evidence, dict) or not evidence:
        return violations
    if evidence.get("available") is not True:
        return violations
    rule = contract.get("acceptance_package")
    if not isinstance(rule, dict):
        return violations

    def _mapping(value: Any) -> dict[str, Any]:
        return value if isinstance(value, dict) else {}

    def _add(label: str) -> None:
        violations.add(label)

    package_rule = _mapping(rule.get("package"))
    expected = _mapping(rule.get("expected"))
    evaluation_rule = _mapping(rule.get("evaluation"))
    rollback_rule = _mapping(rule.get("rollback_carrier"))
    resource_rule = _mapping(rule.get("resource_carrier"))

    claim_scope = _text(expected.get("claim_scope"))
    product_coverage = _text(expected.get("product_coverage_effect"))
    thresholds = _text(expected.get("thresholds"))
    authorization_basis = _text(expected.get("authorization_basis"))
    gate_states = set(_text_list(expected.get("gate_state_vocabulary")))
    gate_kinds = set(_text_list(expected.get("gate_kind_vocabulary")))
    non_blocking = set(_text_list(expected.get("non_blocking_states")))
    required_gate_ids = set(_text_list(expected.get("required_gate_ids")))
    minimum_gates = _count(expected.get("minimum_mandatory_gates"))
    locks_rule = _mapping(expected.get("locks"))
    redaction_flags = _text_list(expected.get("redaction_flags"))

    frozen_reference_path = _text(evaluation_rule.get("frozen_reference_path"))
    candidate_source_path = _text(evaluation_rule.get("candidate_source_path"))
    threshold_policy_state = _text(evaluation_rule.get("threshold_policy_state"))
    body_verdict = _text(evaluation_rule.get("body_verdict"))
    body_token = _text(evaluation_rule.get("body_residual_token"))

    rollback_claim_scope = _text(rollback_rule.get("claim_scope"))
    byte_identity_field = _text(rollback_rule.get("byte_identity_field"))
    fragment_leak_field = _text(rollback_rule.get("fragment_leak_field"))

    absence_state = _text(resource_rule.get("absence_state"))
    pairing_rule = _text(resource_rule.get("pairing_rule"))
    forbidden_pooling = _text(resource_rule.get("forbidden_pooling"))
    forbidden_saving = _text(resource_rule.get("forbidden_saving"))

    declared_set: set[tuple[Optional[str], Optional[str]]] = set()
    declared_carriers = evidence.get("declared_carriers")
    if isinstance(declared_carriers, list):
        for entry in declared_carriers:
            if isinstance(entry, dict):
                declared_set.add(
                    (_text(entry.get("manifest")), _text(entry.get("pointer")))
                )

    def declares_false(value: Any) -> bool:
        """True when a lock declares false in either JSON representation."""
        if value is False:
            return True
        return isinstance(value, str) and value.strip().lower() == "false"

    package = evidence.get("package")
    required = package_rule.get("required") is True
    if not isinstance(package, dict):
        if evidence.get("package_exists") is True or required:
            _add("acceptance_package_claim_without_package")
        return violations

    if _text(package.get("schema_version")) != _text(package_rule.get("schema_version")):
        _add("acceptance_package_schema_drift")

    if _text(package.get("claim_scope")) != claim_scope:
        _add("acceptance_package_coverage_claimed")
    if _text(package.get("product_coverage_effect")) != product_coverage:
        _add("acceptance_package_coverage_claimed")
    if _text(package.get("thresholds")) != thresholds:
        _add("acceptance_package_coverage_claimed")
    locks = package.get("locks")
    if not isinstance(locks, dict):
        _add("acceptance_package_coverage_claimed")
    else:
        for lock, want in locks_rule.items():
            found = locks.get(lock)
            if isinstance(want, bool):
                if want is False and not declares_false(found):
                    _add("acceptance_package_coverage_claimed")
                elif want is True and found is not True:
                    _add("acceptance_package_coverage_claimed")
            elif _text(want) is None or _text(found) != _text(want):
                _add("acceptance_package_coverage_claimed")
    redaction = package.get("redaction")
    if not isinstance(redaction, dict):
        _add("acceptance_package_coverage_claimed")
    else:
        for flag in redaction_flags:
            if redaction.get(flag) is True:
                _add("acceptance_package_coverage_claimed")

    # ── evidence carriers: a pin must still match the committed bytes ──
    entries = package.get("evidence")
    carrier_sha = evidence.get("carrier_sha256")
    carrier_sha = carrier_sha if isinstance(carrier_sha, dict) else {}
    if not isinstance(entries, list) or not entries:
        _add("acceptance_package_evidence_drift")
    else:
        seen: set[tuple[Optional[str], Optional[str]]] = set()
        for entry in entries:
            if not isinstance(entry, dict):
                _add("acceptance_package_evidence_drift")
                continue
            manifest = _text(entry.get("manifest"))
            pointer = _text(entry.get("pointer"))
            digest = _text(entry.get("revision_sha256"))
            if (
                manifest is None
                or pointer is None
                or not _is_relative_path(manifest)
                or digest is None
                or _SHA256_RE.match(digest) is None
            ):
                _add("acceptance_package_evidence_drift")
                continue
            key = (manifest, pointer)
            if key in seen:
                _add("acceptance_package_evidence_drift")
            seen.add(key)
            recomputed = carrier_sha.get(manifest)
            if _text(recomputed) is None or _text(recomputed) != digest:
                _add("acceptance_package_carrier_stale")
        if declared_set and seen != declared_set:
            _add("acceptance_package_evidence_drift")

    # ── frozen-threshold binding: verdicts must re-derive from the pins ──
    evaluation = package.get("evaluation")
    pins = evidence.get("reference_pins")
    pin_index: dict[str, dict[str, Any]] = {}
    if isinstance(pins, list):
        for pin in pins:
            if isinstance(pin, dict):
                threshold_id = _text(pin.get("threshold_id"))
                if threshold_id is not None:
                    pin_index[threshold_id] = pin
    if not isinstance(evaluation, dict):
        _add("acceptance_package_threshold_unbound")
    else:
        if _text(evaluation.get("threshold_policy_state")) != threshold_policy_state:
            _add("acceptance_package_threshold_unbound")
        if _text(evaluation.get("frozen_reference_path")) != frozen_reference_path:
            _add("acceptance_package_threshold_unbound")
        if _text(evaluation.get("candidate_source_path")) != candidate_source_path:
            _add("acceptance_package_threshold_unbound")
        verdicts = evaluation.get("thresholds")
        if not isinstance(verdicts, list) or not verdicts:
            _add("acceptance_package_threshold_unbound")
        else:
            seen_thresholds: set[Optional[str]] = set()
            for verdict in verdicts:
                if not isinstance(verdict, dict):
                    _add("acceptance_package_threshold_unbound")
                    continue
                threshold_id = _text(verdict.get("threshold_id"))
                if threshold_id is None or threshold_id in seen_thresholds:
                    _add("acceptance_package_threshold_unbound")
                    continue
                seen_thresholds.add(threshold_id)
                pin = pin_index.get(threshold_id)
                if pin is None:
                    _add("acceptance_package_threshold_unbound")
                    continue
                if _text(verdict.get("family_id")) != _text(pin.get("family_id")):
                    _add("acceptance_package_threshold_unbound")
                if _text(verdict.get("metric_id")) != _text(pin.get("metric_id")):
                    _add("acceptance_package_threshold_unbound")
                if _text(verdict.get("stratum")) != _text(pin.get("stratum")):
                    _add("acceptance_package_threshold_unbound")
                if _text(verdict.get("verdict")) is None:
                    _add("acceptance_package_threshold_unbound")
                fraction = pin.get("fraction") if isinstance(pin.get("fraction"), dict) else {}
                numerator = _count(fraction.get("numerator"))
                denominator = _count(fraction.get("denominator"))
                reference_value = verdict.get("frozen_reference_value")
                if (
                    numerator is None
                    or denominator in (None, 0)
                    or not isinstance(reference_value, (int, float))
                    or isinstance(reference_value, bool)
                ):
                    _add("acceptance_package_threshold_unbound")
                elif abs(float(reference_value) - (numerator / denominator)) > 1e-9:
                    _add("acceptance_package_threshold_unbound")
            if pin_index and seen_thresholds != set(pin_index):
                _add("acceptance_package_threshold_unbound")

    # ── mandatory gate ledger: every required gate, typed ──
    gates = package.get("mandatory_gates")
    gate_ids: list[Optional[str]] = []
    if not isinstance(gates, list) or not gates:
        _add("acceptance_package_mandatory_gate_dropped")
        gates = []
    else:
        if minimum_gates is not None and len(gates) < minimum_gates:
            _add("acceptance_package_mandatory_gate_dropped")
        for gate in gates:
            if not isinstance(gate, dict):
                _add("acceptance_package_mandatory_gate_dropped")
                continue
            gate_id = _text(gate.get("gate_id"))
            gate_ids.append(gate_id)
            kind = _text(gate.get("kind"))
            state = _text(gate.get("state"))
            if kind is None or kind not in gate_kinds:
                _add("acceptance_package_mandatory_gate_dropped")
            if state is None or state not in gate_states:
                _add("acceptance_package_mandatory_gate_dropped")
            if not isinstance(gate.get("evidence"), dict):
                _add("acceptance_package_mandatory_gate_dropped")
            if _text(gate.get("detail")) is None:
                _add("acceptance_package_mandatory_gate_dropped")
            if _text(gate.get("evidence_needed")) is None:
                _add("acceptance_package_mandatory_gate_dropped")
            if not isinstance(gate.get("required"), bool):
                _add("acceptance_package_mandatory_gate_dropped")
            if not isinstance(gate.get("blocks_s09"), bool):
                _add("acceptance_package_mandatory_gate_dropped")
            else:
                want_blocks = gate.get("required") is True and state not in non_blocking
                if gate.get("blocks_s09") is not want_blocks:
                    _add("acceptance_package_mandatory_gate_dropped")
        if required_gate_ids - set(gate_ids):
            _add("acceptance_package_mandatory_gate_dropped")
        if len(gate_ids) != len(set(gate_ids)):
            _add("acceptance_package_mandatory_gate_dropped")

    recomputed_blocking = [
        (_text(gate.get("gate_id")), _text(gate.get("state")))
        for gate in gates
        if isinstance(gate, dict) and gate.get("blocks_s09") is True
    ]
    blocking_declared = package.get("blocking_gates")
    if not isinstance(blocking_declared, list):
        _add("acceptance_package_authorization_drift")
    else:
        declared_pairs = [
            (_text(entry.get("gate_id")), _text(entry.get("state")))
            for entry in blocking_declared
            if isinstance(entry, dict)
        ]
        if declared_pairs != recomputed_blocking:
            _add("acceptance_package_authorization_drift")

    authorization = package.get("authorization")
    if not isinstance(authorization, dict):
        _add("acceptance_package_authorization_drift")
        _add("acceptance_package_lifecycle_authorization")
    else:
        if authorization.get("gsd_lifecycle_authorizes") is not False:
            _add("acceptance_package_lifecycle_authorization")
        if _text(authorization.get("basis")) != authorization_basis:
            _add("acceptance_package_authorization_drift")
        s09 = authorization.get("s09_authorized")
        if s09 is True:
            if recomputed_blocking:
                _add("acceptance_package_unmet_gate_authorized")
        elif s09 is False:
            if not recomputed_blocking:
                _add("acceptance_package_authorization_drift")
        else:
            _add("acceptance_package_authorization_drift")

    # ── the body residual must survive verbatim (D499) ──
    body_gates = [
        gate
        for gate in gates
        if isinstance(gate, dict) and _text(gate.get("kind")) == "body_axis"
    ]
    if not body_gates:
        _add("acceptance_package_body_residual_renamed")
    else:
        if body_verdict is not None and not any(
            _text(gate.get("state")) == body_verdict for gate in body_gates
        ):
            _add("acceptance_package_body_residual_renamed")
        if body_token is not None and not any(
            body_token in f"{gate.get('detail') or ''} {gate.get('reason') or ''}"
            for gate in body_gates
        ):
            _add("acceptance_package_body_residual_renamed")

    # ── the same-n resource receipt: no pooled or saving claim ──
    resource = evidence.get("resource")
    if isinstance(resource, dict):
        pairing = _mapping(resource.get("pairing"))
        if _text(pairing.get("rule")) != pairing_rule:
            _add("acceptance_package_pooled_resource_claim")
        if _text(pairing.get("pooled")) != forbidden_pooling:
            _add("acceptance_package_pooled_resource_claim")
        if _text(pairing.get("saving_claim")) != forbidden_saving:
            _add("acceptance_package_pooled_resource_claim")
        rows = resource.get("reference_metrics")
        if not isinstance(rows, list) or not rows:
            _add("acceptance_package_pooled_resource_claim")
        else:
            bound = 0
            for row in rows:
                if not isinstance(row, dict):
                    _add("acceptance_package_pooled_resource_claim")
                    continue
                if _text(row.get("state")) != absence_state:
                    _add("acceptance_package_pooled_resource_claim")
                if row.get("source") is not None:
                    bound += 1
            residuals = _mapping(resource.get("residuals"))
            declared_bound = _count(residuals.get("reference_rows_bound"))
            if declared_bound is not None and declared_bound != bound:
                _add("acceptance_package_pooled_resource_claim")

    # ── the rollback rehearsal: byte-identity evidence or refusal ──
    rollback = evidence.get("rollback")
    if not isinstance(rollback, dict):
        _add("acceptance_package_rollback_without_byte_identity")
    else:
        if _text(rollback.get("claim_scope")) != rollback_claim_scope:
            _add("acceptance_package_rollback_without_byte_identity")
        claim = _mapping(rollback.get("corpus_claim"))
        if byte_identity_field is None or claim.get(byte_identity_field) is not True:
            _add("acceptance_package_rollback_without_byte_identity")
        if fragment_leak_field is not None and claim.get(fragment_leak_field) is not True:
            _add("acceptance_package_rollback_without_byte_identity")
        residuals = _mapping(rollback.get("residuals"))
        if _count(residuals.get("byte_identity_violations")) != 0:
            _add("acceptance_package_rollback_without_byte_identity")
        if _count(residuals.get("native_fragment_leaks")) != 0:
            _add("acceptance_package_rollback_without_byte_identity")
        declared_rehearsed = _count(claim.get("rehearsed_documents"))
        declared_total = _count(claim.get("declared_documents"))
        if _count(rollback.get("documents_byte_identical")) != declared_rehearsed:
            _add("acceptance_package_rollback_without_byte_identity")
        if _count(rollback.get("documents_rehearsed")) != declared_total:
            _add("acceptance_package_rollback_without_byte_identity")
        refusals = rollback.get("refusals")
        if isinstance(refusals, list):
            for refusal in refusals:
                if (
                    not isinstance(refusal, dict)
                    or _text(refusal.get("refusal_reason")) is None
                ):
                    _add("acceptance_package_rollback_without_byte_identity")

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
