"""grobid_horizon_review registry-integrity check (planning honesty control).

This module validates a *planning-only* long-horizon review registry for the
GROBID -> Rust migration. It never ingests review prose as enforcement and it
never lets a plan claim an implementation result.

It reads `scripts/governor/checks/grobid_rust_horizon_review.yaml` (DATA) and
validates `doc/review/grobid-rust-horizon-review.json` as a control registry:

- the document validates against the committed Draft 2020-12 schema;
- stage IDs are unique, inside the declared contiguous range H01..H13, and
  every declared stage is present (no gaps, no extras);
- every stage declares the contract's required fields and an allowed status;
- implementation evidence and verification evidence are both present and
  remain separate (no path is reused as a verification ref);
- implementation evidence paths resolve to real repository paths;
- artifact-mode verification refs resolve to real repository paths;
- tested source revision exists and records whether it was verified;
- index freshness is either current or carries a documented limitation
  (GitNexus/CBM staleness must never be silently presented as fresh);
- negative tests exist with unique IDs;
- every ADR-051 test-matrix row names a known contour and test type and states
  applicability with a reason;
- under `claim_scope: planning` no stage may claim `implementation_pass` /
  `verification_pass`, and no stage may leave the unresolved status set; an
  `evidence` scope may claim a PASS only with current closure evidence.
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


_DEFAULT_REQUIRED_STAGE_FIELDS = (
    "stage_id",
    "title",
    "scope",
    "status",
    "depends_on",
    "owner",
    "adr_refs",
    "implementation_evidence",
    "verification_evidence",
    "tested_source",
    "index_freshness",
    "negative_tests",
    "test_matrix",
    "implementation_pass",
    "verification_pass",
)
_VERIFICATION_MODES = {"governor_rule", "command", "artifact", "runtime", "human"}
_COMPLETION_MODES = {"governor_rule", "command", "artifact", "runtime", "human"}
_DATE_PATTERN = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$")
_STAGE_ID_PATTERN = re.compile(r"^([A-Z]+)([0-9]+)$")


def _get_nested(data: Any, dotpath: str) -> Any:
    current = data
    for part in dotpath.split("."):
        if isinstance(current, dict):
            current = current.get(part)
        else:
            return None
    return current


def _validate_against_schema(record: Any, schema: dict[str, Any]) -> list[str]:
    """Validate the registry document with the Draft 2020-12 validator.

    The package MUST be present: doc/review prescribes `jsonschema` for
    registry validation. Treating a missing validator as a pass would be fake
    validation, so absence fails closed.
    """
    if not _JSONSCHEMA_AVAILABLE or Draft202012Validator is None:
        return ["jsonschema package is required for registry validation but is not installed"]
    validator = Draft202012Validator(schema)
    return [f"$: {error.message} (at {list(error.absolute_path) or ['$']})" for error in validator.iter_errors(record)]


def _expand_stage_range(id_range: Any) -> list[str]:
    """Expand [H01, H13] into the full contiguous stage ID list."""
    if not isinstance(id_range, (list, tuple)) or len(id_range) != 2:
        return []
    low, high = str(id_range[0]), str(id_range[1])
    low_match = _STAGE_ID_PATTERN.fullmatch(low)
    high_match = _STAGE_ID_PATTERN.fullmatch(high)
    if not low_match or not high_match or low_match.group(1) != high_match.group(1):
        return []
    start, end = int(low_match.group(2)), int(high_match.group(2))
    if end < start:
        return []
    width = len(low_match.group(2))
    return [f"{low_match.group(1)}{number:0{width}d}" for number in range(start, end + 1)]


def _is_non_empty_list(value: Any) -> bool:
    return isinstance(value, list) and len(value) > 0


def _is_non_empty_text(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _resolve(root: Path, raw: Any) -> Path | None:
    text = str(raw)
    if not text or text.startswith("TODO"):
        return None
    candidate = Path(text)
    return candidate if candidate.is_absolute() else (root / text)


def check_grobid_horizon_review(root: Path, contract_rel: str | None = None) -> list[DriftFlag]:
    """Validate the long-horizon GROBID Rust review registry without ingesting prose."""
    requested = contract_rel or "scripts/governor/checks/grobid_rust_horizon_review.yaml"
    candidate = Path(requested)
    contract_path = candidate if candidate.is_absolute() else root / candidate
    if not contract_path.is_file():
        return [DriftFlag(
            flag="grobid_horizon_review_fixture_missing",
            severity="high",
            evidence=f"review contract missing: {contract_path}",
            dimension="scholarly_parse_plane",
        )]
    try:
        contract = yaml.safe_load(contract_path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        return [DriftFlag(
            flag="grobid_horizon_review_invalid",
            severity="high",
            evidence=f"cannot load {contract_path}: {error}",
            dimension="scholarly_parse_plane",
        )]
    if not isinstance(contract, Mapping):
        return [DriftFlag(
            flag="grobid_horizon_review_invalid",
            severity="high",
            evidence=f"review contract must be a YAML mapping, got {type(contract).__name__}: {contract_path}",
            dimension="scholarly_parse_plane",
        )]

    flags: list[DriftFlag] = []
    dimension = str(contract.get("dimension", "scholarly_parse_plane"))
    severity = str(contract.get("severity", "medium"))
    registry = contract.get("registry") or {}
    if not isinstance(registry, Mapping):
        registry = {}

    def _flag(name: str, evidence: str, flag_severity: str | None = None) -> None:
        flags.append(DriftFlag(
            flag=name,
            severity=flag_severity or severity,
            evidence=evidence,
            dimension=dimension,
        ))

    review_rel = str(registry.get("review", ""))
    schema_rel = str(registry.get("review_schema", ""))
    review_path = (root / review_rel) if review_rel else None
    schema_path = (root / schema_rel) if schema_rel else None
    if review_path is None or not review_path.is_file():
        _flag("grobid_horizon_review_fixture_missing", f"review registry missing: {review_path}", "high")
        return flags
    if schema_path is None or not schema_path.is_file():
        _flag("grobid_horizon_review_fixture_missing", f"review schema missing: {schema_path}", "high")
        return flags
    try:
        schema = json.loads(schema_path.read_text(encoding="utf-8"))
    except (ValueError, OSError) as error:
        _flag("grobid_horizon_review_fixture_invalid", f"review schema unreadable: {error}", "high")
        return flags
    try:
        document = json.loads(review_path.read_text(encoding="utf-8"))
    except (ValueError, OSError) as error:
        _flag("grobid_horizon_review_fixture_invalid", f"review registry unreadable: {error}", "high")
        return flags
    if not isinstance(document, Mapping):
        _flag(
            "grobid_horizon_review_fixture_invalid",
            f"review registry must be a JSON object, got {type(document).__name__}",
            "high",
        )
        return flags

    schema_errors = _validate_against_schema(document, schema)
    if schema_errors:
        _flag(
            "grobid_horizon_review_fixture_invalid",
            f"registry schema errors: {schema_errors[:4]}",
            "high",
        )

    stages = document.get("stages")
    if not isinstance(stages, list):
        _flag("grobid_horizon_review_broken", "registry 'stages' must be a list", "high")
        return flags

    required_fields = registry.get("required_stage_fields")
    if not isinstance(required_fields, list) or not required_fields:
        required_fields = list(_DEFAULT_REQUIRED_STAGE_FIELDS)
    required_fields = [str(field) for field in required_fields]

    allow_status = {str(item) for item in (registry.get("allow_status") or [])}
    unresolved_status = {str(item) for item in (registry.get("unresolved_status") or [])}
    id_pattern = str(registry.get("stage_id_pattern", "^H[0-9]{2}$"))
    id_range = registry.get("stage_id_range")
    contours = {str(item) for item in (registry.get("contours") or [])}
    test_types = {str(item) for item in (registry.get("test_types") or [])}
    owner_field = str(registry.get("owner_field", "owner"))
    unassigned_value = str(registry.get("owner_unassigned_value", "unassigned"))
    owner_allow = registry.get("owner_allow")
    owner_allow_set = {str(item) for item in owner_allow} if isinstance(owner_allow, list) and owner_allow else None
    claim_scope = str(document.get("claim_scope", ""))

    stage_ids: list[str] = []
    unresolved: list[str] = []
    unresolved_unassigned: list[str] = []
    claim_count = 0

    for position, stage in enumerate(stages):
        label = f"stage[{position}]"
        if not isinstance(stage, Mapping):
            _flag("grobid_horizon_review_broken", f"{label} is not an object", "high")
            continue
        stage_id = str(stage.get("stage_id", ""))
        label = stage_id or label

        missing = [field for field in required_fields if field not in stage]
        if missing:
            _flag("grobid_horizon_review_broken", f"{label} missing required fields {missing}", "high")

        if stage_id:
            stage_ids.append(stage_id)
            if re.search(id_pattern, stage_id) is None:
                _flag("grobid_horizon_review_broken", f"{label} stage ID outside pattern {id_pattern}", "high")

        status = str(stage.get("status", ""))
        if allow_status and status not in allow_status:
            _flag("grobid_horizon_review_broken", f"{label} has unknown status {status!r}", "high")
        if status in unresolved_status:
            unresolved.append(label)

        owner_value = str(_get_nested(stage, owner_field))
        if status in unresolved_status and owner_value == unassigned_value:
            unresolved_unassigned.append(label)
        if owner_allow_set is not None and owner_value not in owner_allow_set:
            _flag(
                "grobid_horizon_review_broken",
                f"{label} has unknown owner {owner_value!r}; expected one of {sorted(owner_allow_set)}",
                "high",
            )

        adr_refs = stage.get("adr_refs")
        if not _is_non_empty_list(adr_refs):
            _flag("grobid_horizon_review_broken", f"{label} must declare at least one ADR ref", "high")
        else:
            for adr in adr_refs:
                if re.fullmatch(r"ADR-[0-9]{3}", str(adr)) is None:
                    _flag("grobid_horizon_review_broken", f"{label} has malformed ADR ref {adr!r}", "high")

        implementation = stage.get("implementation_evidence")
        implementation_paths: set[str] = set()
        if not _is_non_empty_list(implementation):
            _flag("grobid_horizon_review_broken", f"{label} must declare implementation evidence", "high")
        else:
            for index, item in enumerate(implementation):
                if not isinstance(item, Mapping):
                    _flag("grobid_horizon_review_broken", f"{label} implementation_evidence[{index}] is not an object", "high")
                    continue
                raws = str(item.get("path", ""))
                resolved = _resolve(root, raws)
                if resolved is None or not resolved.exists():
                    _flag(
                        "grobid_horizon_review_broken",
                        f"{label} implementation_evidence[{index}] missing path {raws!r}",
                        "high",
                    )
                else:
                    implementation_paths.add(raws)

        verification = stage.get("verification_evidence")
        verification_refs: set[str] = set()
        if not _is_non_empty_list(verification):
            _flag("grobid_horizon_review_broken", f"{label} must declare verification evidence", "high")
        else:
            for index, item in enumerate(verification):
                if not isinstance(item, Mapping):
                    _flag("grobid_horizon_review_broken", f"{label} verification_evidence[{index}] is not an object", "high")
                    continue
                mode = str(item.get("mode", ""))
                ref = str(item.get("ref", ""))
                if mode not in _VERIFICATION_MODES:
                    _flag("grobid_horizon_review_broken", f"{label} verification_evidence[{index}] unknown mode {mode!r}", "high")
                if not _is_non_empty_text(ref) or ref.startswith("TODO"):
                    _flag("grobid_horizon_review_broken", f"{label} verification_evidence[{index}] unresolved ref {ref!r}", "high")
                    continue
                verification_refs.add(ref)
                if mode == "artifact":
                    resolved = _resolve(root, ref)
                    if resolved is None or not resolved.exists():
                        _flag(
                            "grobid_horizon_review_broken",
                            f"{label} verification_evidence[{index}] artifact missing {ref!r}",
                            "high",
                        )

        overlap = implementation_paths & verification_refs
        if overlap:
            _flag(
                "grobid_horizon_review_broken",
                f"{label} implementation/verification evidence overlap: {sorted(overlap)}",
                "high",
            )

        tested_source = stage.get("tested_source")
        if not isinstance(tested_source, Mapping):
            _flag("grobid_horizon_review_broken", f"{label} tested_source must be an object", "high")
        else:
            revision = tested_source.get("revision")
            verified = tested_source.get("verified")
            if not _is_non_empty_text(revision) or len(str(revision)) < 5:
                _flag("grobid_horizon_review_broken", f"{label} tested_source.revision missing or too short", "high")
            if not isinstance(verified, bool):
                _flag("grobid_horizon_review_broken", f"{label} tested_source.verified must be a boolean", "high")

        freshness = stage.get("index_freshness")
        if not isinstance(freshness, Mapping):
            _flag("grobid_horizon_review_broken", f"{label} index_freshness must be an object", "high")
        else:
            state = str(freshness.get("state", ""))
            if state in {"stale", "unknown"}:
                if not _is_non_empty_text(freshness.get("limitation")):
                    _flag(
                        "grobid_horizon_review_broken",
                        f"{label} index freshness is {state} without a documented limitation",
                        "high",
                    )
            elif state == "fresh":
                has_gitnexus = isinstance(freshness.get("gitnexus"), Mapping) and bool(freshness.get("gitnexus"))
                has_cbm = isinstance(freshness.get("codebase_memory"), Mapping) and bool(freshness.get("codebase_memory"))
                if not has_gitnexus and not has_cbm:
                    _flag(
                        "grobid_horizon_review_broken",
                        f"{label} claims fresh index without GitNexus or codebase-memory evidence",
                        "high",
                    )
            else:
                _flag("grobid_horizon_review_broken", f"{label} has unknown index freshness state {state!r}", "high")

        negative_tests = stage.get("negative_tests")
        if not _is_non_empty_list(negative_tests):
            _flag("grobid_horizon_review_broken", f"{label} must declare negative tests", "high")
        else:
            seen_ids: set[str] = set()
            for index, item in enumerate(negative_tests):
                if not isinstance(item, Mapping):
                    _flag("grobid_horizon_review_broken", f"{label} negative_tests[{index}] is not an object", "high")
                    continue
                test_id = str(item.get("id", ""))
                if not _is_non_empty_text(test_id):
                    _flag("grobid_horizon_review_broken", f"{label} negative_tests[{index}] missing id", "high")
                elif test_id in seen_ids:
                    _flag("grobid_horizon_review_broken", f"{label} duplicate negative test id {test_id!r}", "high")
                else:
                    seen_ids.add(test_id)
                if not _is_non_empty_text(item.get("scenario")) or not _is_non_empty_text(item.get("expected")):
                    _flag(
                        "grobid_horizon_review_broken",
                        f"{label} negative_tests[{index}] needs scenario and expected",
                        "high",
                    )

        matrix = stage.get("test_matrix")
        if not _is_non_empty_list(matrix):
            _flag("grobid_horizon_review_broken", f"{label} must declare ADR-051 test matrix rows", "high")
        else:
            for index, row in enumerate(matrix):
                if not isinstance(row, Mapping):
                    _flag("grobid_horizon_review_broken", f"{label} test_matrix[{index}] is not an object", "high")
                    continue
                contour = str(row.get("contour", ""))
                test_type = str(row.get("type", ""))
                applicability = str(row.get("applicability", ""))
                if contours and contour not in contours:
                    _flag("grobid_horizon_review_broken", f"{label} test_matrix[{index}] unknown contour {contour!r}", "high")
                if test_types and test_type not in test_types:
                    _flag("grobid_horizon_review_broken", f"{label} test_matrix[{index}] unknown test type {test_type!r}", "high")
                if applicability not in {"applicable", "not_applicable"}:
                    _flag(
                        "grobid_horizon_review_broken",
                        f"{label} test_matrix[{index}] applicability must be applicable or not_applicable",
                        "high",
                    )
                if not _is_non_empty_text(row.get("reason")):
                    _flag("grobid_horizon_review_broken", f"{label} test_matrix[{index}] needs an applicability reason", "high")

        implementation_pass = stage.get("implementation_pass")
        verification_pass = stage.get("verification_pass")
        claims = implementation_pass is True or verification_pass is True
        if claims:
            claim_count += 1
        if claim_scope == "planning" and claims:
            _flag(
                "grobid_horizon_review_broken",
                f"{label} claims implementation/verification PASS under claim_scope=planning",
                "high",
            )
        if claim_scope == "planning" and status and status not in unresolved_status:
            _flag(
                "grobid_horizon_review_broken",
                f"{label} status {status!r} closes the stage under claim_scope=planning",
                "high",
            )
        if implementation_pass is True or verification_pass is True or status == "complete":
            if str(stage.get("status", "")) != "complete":
                _flag("grobid_horizon_review_broken", f"{label} claims PASS without status=complete", "high")
            if verification_pass is not True and implementation_pass is True:
                _flag("grobid_horizon_review_broken", f"{label} claims implementation PASS without verification PASS", "high")
            if isinstance(tested_source, Mapping) and tested_source.get("verified") is not True:
                _flag("grobid_horizon_review_broken", f"{label} claims PASS on an unverified tested source", "high")
            completion = stage.get("completion_evidence")
            if not isinstance(completion, Mapping):
                _flag("grobid_horizon_review_broken", f"{label} claims closure without completion_evidence", "high")
            else:
                invalid = next(
                    (
                        detail
                        for detail, bad in (
                            ("mode", str(completion.get("mode", "")) not in _COMPLETION_MODES),
                            ("ref", not _is_non_empty_text(completion.get("ref"))),
                            ("result", completion.get("result") != "pass"),
                            ("artifacts", not _is_non_empty_list(completion.get("artifacts"))),
                            ("observed_at", _DATE_PATTERN.fullmatch(str(completion.get("observed_at", ""))) is None),
                            ("source_revision", not _is_non_empty_text(completion.get("source_revision"))),
                        )
                        if bad
                    ),
                    None,
                )
                if invalid:
                    _flag("grobid_horizon_review_broken", f"{label} completion_evidence.{invalid} invalid", "high")
                else:
                    for artifact in completion.get("artifacts", []):
                        resolved = _resolve(root, artifact)
                        if resolved is None or not resolved.exists():
                            _flag(
                                "grobid_horizon_review_broken",
                                f"{label} completion artifact missing: {artifact!r}",
                                "high",
                            )

    duplicates = sorted({stage_id for stage_id in stage_ids if stage_ids.count(stage_id) > 1})
    if duplicates:
        _flag("grobid_horizon_review_broken", f"duplicate stage IDs: {duplicates}", "high")
    if stage_ids != sorted(stage_ids):
        _flag("grobid_horizon_review_broken", "stage IDs are not in stable sorted order", "medium")

    expected_ids = _expand_stage_range(id_range)
    if expected_ids:
        missing_ids = [stage_id for stage_id in expected_ids if stage_id not in set(stage_ids)]
        extra_ids = [stage_id for stage_id in stage_ids if stage_id not in set(expected_ids)]
        if missing_ids or extra_ids:
            _flag(
                "grobid_horizon_review_broken",
                f"stage coverage mismatch for declared range {id_range}: missing={missing_ids} extra={extra_ids}",
                "high",
            )
    elif id_range:
        _flag("grobid_horizon_review_broken", f"malformed stage_id_range {id_range!r}", "high")

    _flag(
        "grobid_horizon_review_scope_warning",
        "H-stage records are planning control data; they do not create milestones, waves, slices, tasks, success criteria or acceptance conditions, and an unresolved stage is not implementation evidence",
        "medium",
    )
    _flag(
        "grobid_horizon_review_status",
        f"stages={len(stages)} unresolved={len(unresolved)} complete={sum(1 for s in stages if isinstance(s, Mapping) and str(s.get('status')) == 'complete')} claims={claim_count} unassigned_unresolved={len(unresolved_unassigned)}",
        "medium",
    )
    return flags
