"""Schema/Manifest hierarchy checker — M451 / ADR-068.

Generic engine. Roles: SCHEMA = norm, FS = truth, Governor = direct
validator, MANIFEST = derived cache. The engine validates the filesystem
against rules declared IN EACH SCHEMA (valid/forbidden/ignore) — no
structural knowledge is hardcoded here. It never compares manifests to
the filesystem (that error class was deleted by the ADR revision).
"""
from __future__ import annotations

import fnmatch
from pathlib import Path
from typing import Any

import yaml

from governor.domain.trajectory import DriftFlag


def check_schema_hierarchy(root: Path) -> list[DriftFlag]:
    contract_path = root / "scripts/governor/checks/schema_hierarchy.yaml"
    if not contract_path.exists():
        return []

    try:
        contract = yaml.safe_load(contract_path.read_text()) or {}
    except (OSError, yaml.YAMLError):
        return []

    if not contract:
        return []

    dimension = str(contract.get("dimension", "schema_hierarchy"))
    severity = str(contract.get("severity", "medium"))

    def _flag(code: str, evidence: str) -> DriftFlag:
        return DriftFlag(flag=code, severity=severity, evidence=evidence, dimension=dimension)

    flags: list[DriftFlag] = []

    # 1. Structural directories: SCHEMA.yaml exists, versioned, FS conforms
    for req_dir in contract.get("structural_directories", []):
        dir_path = root / req_dir if req_dir else root
        label = req_dir or "repo root"
        flags.extend(_check_schema_rules(dir_path, label, _flag))

    # 2. Aggregate levels: MANIFEST.yaml cache exists (derived, cheap)
    for agg_dir in contract.get("aggregate_directories", []):
        dir_path = root / agg_dir if agg_dir else root
        label = agg_dir or "repo root"
        manifest_file = dir_path / "MANIFEST.yaml"
        if not manifest_file.exists():
            flags.append(_flag(
                "manifest_cache_missing",
                f"{label}/ has no MANIFEST.yaml cache (ADR-068 §2.2)",
            ))
        else:
            data = _load_yaml(manifest_file)
            if not isinstance(data, dict) or "generated_at" not in data:
                flags.append(_flag(
                    "manifest_not_timestamped",
                    f"{label}/MANIFEST.yaml missing generated_at",
                ))

    # 3. Forbidden top-level names (data-driven patterns)
    for pattern in contract.get("forbidden_top_level", []):
        for child in root.iterdir():
            if fnmatch.fnmatch(child.name, pattern):
                flags.append(_flag(
                    "forbidden_directory",
                    f"{child.name}/ matches forbidden pattern '{pattern}' (ADR-068)",
                ))

    return flags


# ── internals ────────────────────────────────────────────────────────

def _check_schema_rules(dir_path: Path, label: str, _flag) -> list[DriftFlag]:
    flags: list[DriftFlag] = []

    schema_file = dir_path / "SCHEMA.yaml"
    if not schema_file.exists():
        flags.append(_flag(
            "schema_missing",
            f"{label}/ has no SCHEMA.yaml — structure undefined (ADR-068)",
        ))
        return flags

    schema = _load_yaml(schema_file)
    if not isinstance(schema, dict) or "schema_version" not in schema:
        flags.append(_flag(
            "schema_not_versioned",
            f"{label}/SCHEMA.yaml missing schema_version",
        ))
        return flags

    if not dir_path.exists():
        flags.append(_flag("directory_missing", f"{label}/ declared but absent"))
        return flags

    ignore = _as_patterns(schema.get("ignore", []))
    tolerated = _as_patterns(schema.get("tolerated", []))
    forbidden = _as_patterns(schema.get("forbidden", []) or schema.get("invalid_directories", []))

    # 3a. Subdirectories must be valid (declared) — data-driven from schema
    valid_subdirs = schema.get("valid_subdirs") or schema.get("valid_directories")
    subdir_pattern = _as_patterns(schema.get("subdir_pattern", []))
    if subdir_pattern and valid_subdirs is None:
        # Pattern mode: every non-ignored subdir must match a declared pattern
        for child in sorted(dir_path.iterdir()):
            name = child.name
            if not child.is_dir() or name.startswith("."):
                continue
            if any(fnmatch.fnmatch(name, pat) for pat in ignore + tolerated):
                continue
            if not any(fnmatch.fnmatch(name, pat) for pat in subdir_pattern):
                flags.append(_flag(
                    "subdir_not_in_schema",
                    f"{label}/{name}/ does not match SCHEMA subdir_pattern (ADR-068)",
                ))
    if valid_subdirs is not None:
        valid_patterns = _as_patterns(valid_subdirs)
        for child in sorted(dir_path.iterdir()):
            name = child.name
            if not child.is_dir() or name.startswith("."):
                continue
            if any(fnmatch.fnmatch(name, pat) for pat in ignore + tolerated):
                continue
            if not any(fnmatch.fnmatch(name, pat) for pat in valid_patterns):
                flags.append(_flag(
                    "subdir_not_in_schema",
                    f"{label}/{name}/ exists but is not in SCHEMA valid list "
                    f"(ADR-068: FS must conform to norm)",
                ))

    # 3b. Locally forbidden names
    for child in sorted(dir_path.iterdir()):
        name = child.name
        if any(fnmatch.fnmatch(name, pat) for pat in ignore):
            continue
        if any(fnmatch.fnmatch(name, pat) for pat in forbidden):
            flags.append(_flag(
                "forbidden_entry",
                f"{label}/{name}/ forbidden by SCHEMA (ADR-068)",
            ))

    # 3c. Top-level file patterns (fast: non-recursive)
    valid_files = schema.get("valid_files")
    if valid_files:
        valid_file_patterns = _as_patterns(valid_files)
        allowed_named = {"SCHEMA.yaml", "MANIFEST.yaml", "OKF.md", ".gitkeep"}
        for child in sorted(dir_path.iterdir()):
            if not child.is_file() or child.name.startswith("."):
                continue
            if child.name in allowed_named:
                continue
            if any(fnmatch.fnmatch(child.name, pat) for pat in ignore + tolerated):
                continue
            if not any(fnmatch.fnmatch(child.name, pat) for pat in valid_file_patterns):
                flags.append(_flag(
                    "file_not_in_schema",
                    f"{label}/{child.name} does not match SCHEMA valid_files",
                ))

    return flags


def _as_patterns(value: Any) -> list[str]:
    if value is None:
        return []
    if isinstance(value, str):
        return [value]
    return [str(v) for v in value]


def _load_yaml(path: Path) -> Any:
    try:
        return yaml.safe_load(path.read_text())
    except (OSError, yaml.YAMLError):
        return None
