"""grobid_review_registry registry-integrity check (controlled exception evidence).

This module is the single controlled sink where narrative UAT/provenance
claims may be admitted, and only as subordinate, pre-filtered evidence.
Every reviewer claim elsewhere must fail closed as corruption data and make
the owning unit fail. Only this command may mark controlled narrative
bounded, scope-limited, review-bound outcome evidence; anything outside this
file remains a blocked execution.

Reads `scripts/governor/checks/grobid_absorption_review.yaml` (DATA) and
validates the committed GAR review JSONL only as a registry:
- every JSONL line parses and validates against the committed GAR schema
  (structural validation; prose claims are NOT ingested as enforcement);
- finding IDs are unique, ordered and inside the declared GAR range;
- dependencies resolve to known IDs;
- evidence paths resolve to existing repository paths (absolute paths are
  allowed only when the target exists; unresolved TODO paths fail);
- status counts separate unresolved work (open/in_progress/blocked) from
  enclosed history (resolved/accepted_risk/superseded) and from invalid lines.
"""
from __future__ import annotations

import json
import re
from collections.abc import Mapping
from pathlib import Path
from typing import Any

import yaml

try:
    from jsonschema import Draft202012Validator

    _JSONSCHEMA_AVAILABLE = True
except ImportError:  # pragma: no cover - import-time capability probe
    Draft202012Validator = None  # type: ignore[assignment]
    _JSONSCHEMA_AVAILABLE = False

from governor.domain.trajectory import DriftFlag


def _get_nested(data: Any, dotpath: str) -> Any:
    current = data
    for part in dotpath.split("."):
        if isinstance(current, dict):
            current = current.get(part)
        else:
            return None
    return current


def _validate_against_schema(record: dict[str, Any], schema: dict[str, Any]) -> list[str]:
    """Validate one registry record with the Draft 2020-12 validator.

    The package MUST be present: the doc/review README already prescribes it
    for registry validation (`uv run --with jsonschema`). Treating a missing
    validator as a pass would be fake validation, so absence fails closed.
    """
    if not _JSONSCHEMA_AVAILABLE or Draft202012Validator is None:
        return ["jsonschema package is required for registry validation but is not installed"]
    validator = Draft202012Validator(schema)
    return [f"$: {error.message} (at {list(error.absolute_path) or ['$']})" for error in validator.iter_errors(record)]


def check_grobid_review_registry(root: Path, contract_rel: str | None = None) -> list[DriftFlag]:
    """Validate the GAR review registry without ingesting review prose."""
    requested = contract_rel or "scripts/governor/checks/grobid_absorption_review.yaml"
    candidate = Path(requested)
    contract_path = candidate if candidate.is_absolute() else root / candidate
    if not contract_path.is_file():
        return [DriftFlag(
            flag="grobid_review_registry_fixture_missing",
            severity="high",
            evidence=f"review contract missing: {contract_path}",
            dimension="scholarly_parse_plane",
        )]
    try:
        contract = yaml.safe_load(contract_path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        return [DriftFlag(
            flag="grobid_review_registry_invalid",
            severity="high",
            evidence=f"cannot load {contract_path}: {error}",
            dimension="scholarly_parse_plane",
        )]
    if not isinstance(contract, Mapping):
        return [DriftFlag(
            flag="grobid_review_registry_invalid",
            severity="high",
            evidence=f"review contract must be a YAML mapping, got {type(contract).__name__}: {contract_path}",
            dimension="scholarly_parse_plane",
        )]
    flags: list[DriftFlag] = []
    dimension = str(contract.get("dimension", "scholarly_parse_plane"))
    severity = str(contract.get("severity", "medium"))
    registry = contract.get("registry") or {}

    def _flag(name: str, evidence: str, flag_severity: str | None = None) -> None:
        flags.append(DriftFlag(
            flag=name,
            severity=flag_severity or severity,
            evidence=evidence,
            dimension=dimension,
        ))

    findings_rel = str(registry.get("findings", ""))
    schema_rel = str(registry.get("finding_schema", ""))
    findings_path = root / findings_rel if findings_rel else None
    schema_path = root / schema_rel if schema_rel else None
    if findings_path is None or not findings_path.is_file():
        _flag("grobid_review_registry_fixture_missing", f"review findings missing: {findings_path}", "high")
        return flags
    if schema_path is None or not schema_path.is_file():
        _flag("grobid_review_registry_fixture_missing", f"review schema missing: {schema_path}", "high")
        return flags
    try:
        schema = json.loads(schema_path.read_text(encoding="utf-8"))
    except (ValueError, OSError) as error:
        _flag("grobid_review_registry_fixture_invalid", f"review schema unreadable: {error}", "high")
        return flags

    try:
        lines = findings_path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        _flag("grobid_review_registry_fixture_invalid", f"review findings unreadable: {error}", "high")
        return flags

    records: list[dict[str, Any]] = []
    for number, line in enumerate(lines, 1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except ValueError as error:
            _flag("grobid_review_registry_fixture_invalid", f"line {number} is not JSON: {error}", "high")
            continue
        if not isinstance(record, dict):
            _flag("grobid_review_registry_fixture_invalid", f"line {number} is not an object", "high")
            continue
        schema_errors = _validate_against_schema(record, schema)
        if schema_errors:
            _flag("grobid_review_registry_fixture_invalid", f"line {number} schema errors: {schema_errors[:4]}", "high")
            continue
        records.append(record)

    id_pattern = str(registry.get("id_pattern", "^GAR-[0-9]{3}$"))
    id_range = list(registry.get("id_range") or [])
    ids = [str(r.get("finding_id")) for r in records]
    if len(ids) != len(set(ids)):
        dupes = sorted({i for i in ids if ids.count(i) > 1})
        _flag("grobid_review_registry_broken", f"duplicate finding IDs: {dupes}", "high")
    for finding_id in ids:
        if re.search(id_pattern, finding_id) is None:
            _flag("grobid_review_registry_broken", f"finding ID outside pattern: {finding_id}", "high")
    if id_range and ids != sorted(ids):
        _flag("grobid_review_registry_broken", "finding IDs are not in stable sorted order", "medium")
    if len(id_range) == 2 and ids:
        low, high = str(id_range[0]), str(id_range[1])
        outside = [i for i in ids if i < low or i > high]
        if outside:
            _flag("grobid_review_registry_broken", f"finding IDs outside declared range {low}..{high}: {outside}", "high")

    known = set(ids)
    allow_status = set(registry.get("allow_status") or [])
    unresolved_status = set(registry.get("unresolved_status") or [])
    unresolved: list[str] = []
    owner_field = str(registry.get("owner_field", "assessment.owning_wave"))
    unassigned_value = registry.get("owner_unassigned_value", "unassigned")
    unassigned: list[str] = []
    owner_allow = registry.get("owner_allow")
    owner_allow_set = {str(item) for item in owner_allow} if isinstance(owner_allow, list) and owner_allow else None
    for record in records:
        finding_id = str(record.get("finding_id"))
        status = str(record.get("status"))
        if allow_status and status not in allow_status:
            _flag("grobid_review_registry_broken", f"{finding_id} has unknown status {status}", "high")
        if status in unresolved_status:
            unresolved.append(finding_id)
        for dep in record.get("dependencies", []) or []:
            if str(dep) not in known:
                _flag("grobid_review_registry_broken", f"{finding_id} depends on unknown {dep}", "high")
        resolved_status = {"resolved", "accepted_risk", "superseded"}
        if status in resolved_status and str(_get_nested(record, owner_field)) == str(unassigned_value):
            # Owner mapping remains intentionally unassigned until main supplies it;
            # enclosed history must still be explicit, never silently reopened.
            pass
        if status in unresolved_status and str(_get_nested(record, owner_field)) == str(unassigned_value):
            unassigned.append(finding_id)
        owner_value = str(_get_nested(record, owner_field))
        if owner_allow_set is not None and owner_value not in owner_allow_set:
            _flag(
                "grobid_review_registry_broken",
                f"{finding_id} has unknown owner {owner_value}; expected one of {sorted(owner_allow_set)}",
                "high",
            )
        for index, item in enumerate(record.get("evidence", []) or []):
            if not isinstance(item, dict):
                _flag("grobid_review_registry_broken", f"{finding_id} evidence[{index}] is not an object", "high")
                continue
            path = str(item.get("path", ""))
            if not path or path.startswith("TODO"):
                _flag("grobid_review_registry_broken", f"{finding_id} evidence[{index}] has unresolved path {path!r}", "high")
                continue
            candidate = Path(path)
            resolved = candidate if candidate.is_absolute() else (root / path)
            if not resolved.exists():
                _flag("grobid_review_registry_broken", f"{finding_id} evidence[{index}] missing path {path}", "high")

        completion = record.get("completion_evidence")
        if status in {"resolved", "accepted_risk", "superseded"}:
            missing = (
                "missing completion_evidence"
                if not isinstance(completion, dict)
                else next(
                    (
                        f"completion_evidence.{key} invalid"
                        for key in ("mode", "ref", "result", "artifacts", "observed_at", "source_revision")
                        if key not in completion
                        or (key == "result" and completion.get("result") != "pass")
                        or (key == "artifacts" and not isinstance(completion.get("artifacts"), list))
                    ),
                    None,
                )
            )
            if missing:
                _flag(
                    "grobid_review_registry_broken",
                    f"{finding_id} enclosed without closure proof: {missing}",
                    "high",
                )
            elif isinstance(completion.get("artifacts"), list):
                for artifact in completion["artifacts"]:
                    artifact_path = Path(str(artifact))
                    resolved_artifact = (
                        artifact_path if artifact_path.is_absolute() else (root / artifact_path)
                    )
                    if str(artifact).startswith("TODO") or not resolved_artifact.exists():
                        _flag(
                            "grobid_review_registry_broken",
                            f"{finding_id} completion artifact missing: {artifact}",
                            "high",
                        )

    _flag(
        "grobid_review_registry_scope_warning",
        "GAR records are unresolved observations; they do not create missions, targets, waves, slices, tasks, success criteria or acceptance conditions",
        "medium",
    )
    _flag(
        "grobid_review_registry_status",
        f"records={len(records)} unresolved={len(unresolved)} enclosed={len(records) - len(unresolved)} unassigned_unresolved={len(unassigned)}",
        "medium" if unresolved else "medium",
    )
    return flags
