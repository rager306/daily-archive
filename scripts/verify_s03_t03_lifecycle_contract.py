#!/usr/bin/env python3
"""Verify the narrowly scoped D350/D351 S03/T03 lifecycle contract.

This verifier is intentionally independent of GSD's private database.  It checks
review data plus the immutable fail-closed acceptance manifest; live lifecycle
state comparison belongs to the completion-owned runtime UAT.
"""
from __future__ import annotations

from copy import deepcopy
from pathlib import Path
from typing import Any

import yaml

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "doc/review/s03-t03-lifecycle-contract.yaml"
MANIFEST_PATH = ROOT / "data/parse/acceptance_manifest.yaml"

EXPECTED_IDENTITY = {
    "milestone_id": "M502-2w7y8k",
    "slice_id": "S03",
    "task_id": "T03",
}
EXPECTED_LIFECYCLE = {
    "task_status": "complete",
    "lifecycle_status": "completed",
    "waiver_required": False,
    "waiver_count": 0,
}
EXPECTED_LOCKS = {
    "policy_locks.import_eligible": False,
    "policy_locks.auto_promote": "forbidden",
    "policy_locks.production_route": "forbidden",
    "external_holdout_contract.execution_status": "not_executed",
    "release_decision_package.package_status": "pending_human_review",
    "release_decision_package.quality.status": "blocked",
    "release_decision_package.rights.status": "blocked",
    "release_decision_package.prohibited_authorizations.graph_write_or_import": "blocked",
    "release_decision_package.human_release_review.prose_alone_authorizes_release": False,
}


def fail(message: str) -> None:
    raise ValueError(message)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def mapping(value: Any, label: str) -> dict[str, Any]:
    require(isinstance(value, dict), f"{label}: expected mapping")
    return value


def exact(mapping_value: dict[str, Any], key: str, expected: Any, label: str) -> None:
    require(key in mapping_value, f"{label}: missing {key}")
    actual = mapping_value[key]
    if isinstance(expected, bool):
        require(type(actual) is bool, f"{label}.{key}: expected boolean, got {type(actual).__name__}")
    elif isinstance(expected, int):
        require(type(actual) is int and not isinstance(actual, bool), f"{label}.{key}: expected integer")
    elif isinstance(expected, str):
        require(type(actual) is str, f"{label}.{key}: expected string")
    require(actual == expected, f"{label}.{key}: expected {expected!r}, got {actual!r}")


def dotted(root: dict[str, Any], path: str) -> Any:
    value: Any = root
    for component in path.split("."):
        require(isinstance(value, dict) and component in value, f"manifest: missing {path}")
        value = value[component]
    return value


def validate_contract(contract: dict[str, Any]) -> None:
    exact(contract, "schema_version", "s03-t03-lifecycle-contract.v1", "contract")
    exact(contract, "authority", "D350", "contract")
    exact(contract, "reconciliation_decision", "D351", "contract")

    identity = mapping(contract.get("identity"), "contract.identity")
    for key, expected in EXPECTED_IDENTITY.items():
        exact(identity, key, expected, "contract.identity")

    expected = mapping(contract.get("expected"), "contract.expected")
    for key, value in EXPECTED_LIFECYCLE.items():
        exact(expected, key, value, "contract.expected")

    non_goals = contract.get("non_goals")
    require(isinstance(non_goals, list) and all(isinstance(item, str) for item in non_goals), "contract.non_goals: expected list of strings")
    required_non_goals = {
        "reopen S03",
        "revert T03",
        "change retained S04/T02-T03 rows",
        "authorize new P9 or oracle execution",
        "authorize external bytes, download, or redistribution",
        "authorize release readiness",
    }
    require(set(non_goals) == required_non_goals, "contract.non_goals: exact fail-closed non-goals required")

    contrast = mapping(contract.get("contrast"), "contract.contrast")
    rows = contrast.get("retained_rows")
    require(isinstance(rows, list) and len(rows) == 2, "contract.contrast.retained_rows: expected exactly two rows")
    expected_rows = {
        "M502-2w7y8k/S04/T02",
        "M502-2w7y8k/S04/T03",
    }
    actual_rows = set()
    for row in rows:
        row_map = mapping(row, "contract.contrast.retained_rows[]")
        identity_value = row_map.get("identity")
        require(identity_value in expected_rows, f"contract.contrast: unexpected identity {identity_value!r}")
        require(identity_value not in actual_rows, f"contract.contrast: duplicate identity {identity_value!r}")
        actual_rows.add(identity_value)
        for key, value in {
            "task_status": "skipped",
            "lifecycle_status": "cancelled",
            "waiver_required": True,
            "waiver_count": 1,
        }.items():
            exact(row_map, key, value, "contract.contrast.retained_rows[]")
    require(actual_rows == expected_rows, "contract.contrast: S04/T02 and S04/T03 are required")

    protected = mapping(contract.get("protected_manifest"), "contract.protected_manifest")
    exact(protected, "path", "data/parse/acceptance_manifest.yaml", "contract.protected_manifest")
    fields = mapping(protected.get("fields"), "contract.protected_manifest.fields")
    require(set(fields) == set(EXPECTED_LOCKS), "contract.protected_manifest.fields: lock set drifted")
    for key, value in EXPECTED_LOCKS.items():
        exact(fields, key, value, "contract.protected_manifest.fields")
    exact(
        protected,
        "immutable_reason",
        "Lifecycle reconciliation is not release, import, graph-write, splice, or promotion authorization.",
        "contract.protected_manifest",
    )


def validate_manifest(manifest: dict[str, Any]) -> None:
    exact(manifest, "schema_version", "parse-acceptance.v1", "manifest")
    for path, expected in EXPECTED_LOCKS.items():
        actual = dotted(manifest, path)
        if isinstance(expected, bool):
            require(type(actual) is bool, f"manifest.{path}: expected boolean")
        require(actual == expected, f"manifest.{path}: expected {expected!r}, got {actual!r}")


def mutate(root: dict[str, Any], path: str, value: Any) -> dict[str, Any]:
    result = deepcopy(root)
    parts = path.split(".")
    cursor: dict[str, Any] = result
    for part in parts[:-1]:
        cursor = mapping(cursor[part], path)
    cursor[parts[-1]] = value
    return result


def adversarial_cases(contract: dict[str, Any], manifest: dict[str, Any]) -> None:
    cases = [
        ("obsolete skipped task", mutate(contract, "expected.task_status", "skipped"), "contract"),
        ("obsolete cancelled lifecycle", mutate(contract, "expected.lifecycle_status", "cancelled"), "contract"),
        ("nonzero waiver count", mutate(contract, "expected.waiver_count", 1), "contract"),
        ("import boolean flip", mutate(manifest, "policy_locks.import_eligible", True), "manifest"),
        ("promotion unlock", mutate(manifest, "policy_locks.auto_promote", "allowed"), "manifest"),
        ("production unlock", mutate(manifest, "policy_locks.production_route", "allowed"), "manifest"),
        ("executed holdout", mutate(manifest, "external_holdout_contract.execution_status", "executed"), "manifest"),
        ("release authorization", mutate(manifest, "release_decision_package.package_status", "approved"), "manifest"),
        ("prose authorization", mutate(manifest, "release_decision_package.human_release_review.prose_alone_authorizes_release", True), "manifest"),
        ("rights unlock", mutate(manifest, "release_decision_package.rights.status", "passed"), "manifest"),
    ]
    for name, candidate, kind in cases:
        try:
            if kind == "contract":
                validate_contract(candidate)
            else:
                validate_manifest(candidate)
        except ValueError:
            continue
        fail(f"adversarial case was accepted: {name}")


def main() -> int:
    require(CONTRACT_PATH.is_file(), f"missing contract: {CONTRACT_PATH.relative_to(ROOT)}")
    require(MANIFEST_PATH.is_file(), f"missing manifest: {MANIFEST_PATH.relative_to(ROOT)}")
    with CONTRACT_PATH.open(encoding="utf-8") as handle:
        contract = mapping(yaml.safe_load(handle), "contract")
    with MANIFEST_PATH.open(encoding="utf-8") as handle:
        manifest = mapping(yaml.safe_load(handle), "manifest")
    validate_contract(contract)
    validate_manifest(manifest)
    adversarial_cases(contract, manifest)
    print("PASS: S03/T03 D350 lifecycle contract and fail-closed manifest locks verified")
    print("PASS: 10 in-memory adversarial mutations rejected; no holdout or mutant files written")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, yaml.YAMLError, ValueError) as error:
        print(f"FAIL: {error}")
        raise SystemExit(1)
