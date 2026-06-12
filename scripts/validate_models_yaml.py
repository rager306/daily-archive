#!/usr/bin/env python3
"""Validate models.yaml against the registry schema.

Per M049 (Models Registry) and M046 07-2026-assessment Recommendation 6,
plus M048 patterns-review 01 (deterministic work_id foundation).

Schema (required per entry):
    id:              stable identifier (snake_case, must be unique)
    provider:        one of {anthropic, openai}
    endpoint:        full URL (must start with https://)
    model_name:      non-empty string
    tool_version:    semver-like string
    policy_version:  semver-like string

Top-level required:
    schema_version:  string
    models:          list (>= 1)
    bindings:        list (>= 0)

Usage:
    uv run python scripts/validate_models_yaml.py
    uv run python scripts/validate_models_yaml.py --yaml-path models.yaml

Exit codes:
    0: valid
    1: invalid (errors printed to stderr)
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from typing import Any

import yaml

ALLOWED_PROVIDERS = {"anthropic", "openai"}
REQUIRED_MODEL_FIELDS = {
    "id",
    "provider",
    "endpoint",
    "model_name",
    "tool_version",
    "policy_version",
}
REQUIRED_BINDING_FIELDS = {"binding_id", "model_id", "description"}
REQUIRED_TOP_LEVEL = {"schema_version", "models", "bindings"}
# Version-like strings: semver (X.Y.Z) OR date-like (YYYY-MM-DD) OR project-tagged (m049-v0.1)
VERSION_LIKE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._+\-]*$")
# ID pattern: snake_case OR kebab-case OR with dots (e.g., m3-512k)
ID_PATTERN = re.compile(r"^[a-z][a-z0-9_.\-]*$")
ENDPOINT_PATTERN = re.compile(r"^https://")
DEFAULT_YAML_PATH = Path("models.yaml")


def _is_version_like(s: object) -> bool:
    if s is None:
        return False
    if not isinstance(s, str):
        # YAML may parse `2026-05-15` as a date object; coerce to ISO string.
        try:
            s = s.isoformat()
        except AttributeError:
            return False
    return bool(VERSION_LIKE.match(s))


def validate_registry(payload: dict[str, Any]) -> list[str]:
    errors: list[str] = []

    # Top-level required fields.
    missing = REQUIRED_TOP_LEVEL - set(payload)
    if missing:
        errors.append(f"missing top-level fields: {sorted(missing)}")

    if not isinstance(payload.get("models"), list) or len(payload.get("models", [])) == 0:
        errors.append("'models' must be a non-empty list")
        return errors

    if "models" in payload and not isinstance(payload.get("bindings", []), list):
        errors.append("'bindings' must be a list (possibly empty)")

    # Per-model validation.
    seen_ids: set[str] = set()
    for index, model in enumerate(payload.get("models", [])):
        if not isinstance(model, dict):
            errors.append(f"models[{index}]: must be a mapping")
            continue
        prefix = f"models[{index}]"
        missing_fields = REQUIRED_MODEL_FIELDS - set(model)
        if missing_fields:
            errors.append(f"{prefix}: missing fields: {sorted(missing_fields)}")
            continue
        if not ID_PATTERN.match(model["id"]):
            errors.append(f"{prefix}.id='{model['id']}': must be snake_case, start with lowercase letter")
        if model["id"] in seen_ids:
            errors.append(f"{prefix}.id='{model['id']}': duplicate id")
        seen_ids.add(model["id"])
        if model["provider"] not in ALLOWED_PROVIDERS:
            errors.append(f"{prefix}.provider='{model['provider']}': must be one of {sorted(ALLOWED_PROVIDERS)}")
        if not ENDPOINT_PATTERN.match(model["endpoint"]):
            errors.append(f"{prefix}.endpoint='{model['endpoint']}': must start with https://")
        if not isinstance(model["model_name"], str) or not model["model_name"].strip():
            errors.append(f"{prefix}.model_name: must be non-empty string")
        if not _is_version_like(model["tool_version"]):
            errors.append(f"{prefix}.tool_version='{model['tool_version']}': must be version-like (semver X.Y.Z, date YYYY-MM-DD, or tagged m049-v0.1)")
        if not _is_version_like(model["policy_version"]):
            errors.append(f"{prefix}.policy_version='{model['policy_version']}': must be version-like")

    # Per-binding validation.
    valid_model_ids = {m.get("id") for m in payload.get("models", []) if isinstance(m, dict)}
    for index, binding in enumerate(payload.get("bindings", [])):
        if not isinstance(binding, dict):
            errors.append(f"bindings[{index}]: must be a mapping")
            continue
        prefix = f"bindings[{index}]"
        missing_fields = REQUIRED_BINDING_FIELDS - set(binding)
        if missing_fields:
            errors.append(f"{prefix}: missing fields: {sorted(missing_fields)}")
            continue
        if binding["model_id"] not in valid_model_ids:
            errors.append(f"{prefix}.model_id='{binding['model_id']}': no such model in registry")

    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--yaml-path", type=Path, default=DEFAULT_YAML_PATH)
    args = parser.parse_args()

    if not args.yaml_path.exists():
        print(f"error: file not found: {args.yaml_path}", file=sys.stderr)
        return 1

    try:
        payload = yaml.safe_load(args.yaml_path.read_text(encoding="utf-8"))
    except yaml.YAMLError as exc:
        print(f"error: invalid YAML: {exc}", file=sys.stderr)
        return 1

    if not isinstance(payload, dict):
        print("error: top-level must be a mapping", file=sys.stderr)
        return 1

    errors = validate_registry(payload)
    if errors:
        print(f"models.yaml validation failed ({len(errors)} errors):", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        return 1

    models_count = len(payload.get("models", []))
    bindings_count = len(payload.get("bindings", []))
    print(f"models.yaml valid: {models_count} models, {bindings_count} bindings")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
