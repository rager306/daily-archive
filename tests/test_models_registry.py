"""Tests for models.yaml validator and registry schema.

Per M049 (Models Registry). The validator enforces schema; the registry
itself is loaded in S02 (Python registry and helper integration).

Tests cover:
- Valid YAML passes
- Missing required field fails with clear error
- Duplicate id fails
- Invalid endpoint format fails
- Invalid provider fails
- Invalid version format fails
- Binding references unknown model fails
"""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import validate_models_yaml  # noqa: E402

VALID_REGISTRY = {
    "schema_version": "v0.1",
    "models": [
        {
            "id": "minimax-m3-anthropic",
            "provider": "anthropic",
            "endpoint": "https://api.example.com/v1/messages",
            "model_name": "MiniMax-M3",
            "tool_version": "2026-05-15",
            "policy_version": "m049-v0.1",
        }
    ],
    "bindings": [
        {"binding_id": "test-binding", "model_id": "minimax-m3-anthropic", "description": "test"}
    ],
}


def test_valid_registry_passes():
    errors = validate_models_yaml.validate_registry(VALID_REGISTRY)
    assert errors == []


def test_missing_top_level_field_fails():
    payload = {k: v for k, v in VALID_REGISTRY.items() if k != "schema_version"}
    errors = validate_models_yaml.validate_registry(payload)
    assert any("schema_version" in e for e in errors)


def test_models_not_a_list_fails():
    payload = {**VALID_REGISTRY, "models": "not-a-list"}
    errors = validate_models_yaml.validate_registry(payload)
    assert any("models" in e and "non-empty list" in e for e in errors)


def test_models_empty_list_fails():
    payload = {**VALID_REGISTRY, "models": []}
    errors = validate_models_yaml.validate_registry(payload)
    assert any("non-empty list" in e for e in errors)


def test_missing_required_field_in_model_fails():
    bad_model = {k: v for k, v in VALID_REGISTRY["models"][0].items() if k != "endpoint"}
    payload = {**VALID_REGISTRY, "models": [bad_model]}
    errors = validate_models_yaml.validate_registry(payload)
    assert any("endpoint" in e and "missing" in e for e in errors)


def test_duplicate_model_id_fails():
    payload = {
        **VALID_REGISTRY,
        "models": [
            VALID_REGISTRY["models"][0],
            {**VALID_REGISTRY["models"][0], "endpoint": "https://api.example.com/v2/messages"},
        ],
    }
    errors = validate_models_yaml.validate_registry(payload)
    assert any("duplicate id" in e for e in errors)


def test_duplicate_endpoint_fails():
    payload = {
        **VALID_REGISTRY,
        "models": [
            VALID_REGISTRY["models"][0],
            {**VALID_REGISTRY["models"][0], "id": "minimax-m3-openai", "provider": "openai"},
        ],
    }
    errors = validate_models_yaml.validate_registry(payload)
    assert any("duplicate endpoint" in e for e in errors)


def test_invalid_provider_fails():
    bad_model = {**VALID_REGISTRY["models"][0], "provider": "huggingface"}
    payload = {**VALID_REGISTRY, "models": [bad_model]}
    errors = validate_models_yaml.validate_registry(payload)
    assert any("provider" in e for e in errors)


def test_invalid_endpoint_scheme_fails():
    bad_model = {**VALID_REGISTRY["models"][0], "endpoint": "http://insecure.example.com"}
    payload = {**VALID_REGISTRY, "models": [bad_model]}
    errors = validate_models_yaml.validate_registry(payload)
    assert any("endpoint" in e and "https://" in e for e in errors)


def test_invalid_id_format_fails():
    bad_model = {**VALID_REGISTRY["models"][0], "id": "MiniMax-Not-Snake-Case"}
    payload = {**VALID_REGISTRY, "models": [bad_model]}
    errors = validate_models_yaml.validate_registry(payload)
    assert any("id=" in e and "snake_case" in e for e in errors)


def test_invalid_version_format_fails():
    bad_model = {**VALID_REGISTRY["models"][0], "tool_version": "!!!invalid!!!"}
    payload = {**VALID_REGISTRY, "models": [bad_model]}
    errors = validate_models_yaml.validate_registry(payload)
    assert any("tool_version" in e for e in errors)


def test_version_accepts_semver():
    model = {**VALID_REGISTRY["models"][0], "tool_version": "1.2.3", "policy_version": "2.0.0"}
    payload = {**VALID_REGISTRY, "models": [model]}
    errors = validate_models_yaml.validate_registry(payload)
    assert errors == []


def test_version_accepts_date():
    model = {**VALID_REGISTRY["models"][0], "tool_version": "2026-05-15", "policy_version": "m049-v0.1"}
    payload = {**VALID_REGISTRY, "models": [model]}
    errors = validate_models_yaml.validate_registry(payload)
    assert errors == []


def test_version_accepts_yaml_date_object():
    import datetime

    model = {**VALID_REGISTRY["models"][0], "tool_version": datetime.date(2026, 5, 15), "policy_version": "m049-v0.1"}
    payload = {**VALID_REGISTRY, "models": [model]}
    errors = validate_models_yaml.validate_registry(payload)
    assert errors == []


def test_binding_references_unknown_model_fails():
    bad_binding = {**VALID_REGISTRY["bindings"][0], "model_id": "nonexistent-model"}
    payload = {**VALID_REGISTRY, "bindings": [bad_binding]}
    errors = validate_models_yaml.validate_registry(payload)
    assert any("model_id" in e and "nonexistent-model" in e for e in errors)


def test_binding_missing_field_fails():
    bad_binding = {k: v for k, v in VALID_REGISTRY["bindings"][0].items() if k != "description"}
    payload = {**VALID_REGISTRY, "bindings": [bad_binding]}
    errors = validate_models_yaml.validate_registry(payload)
    assert any("description" in e and "missing" in e for e in errors)


def test_empty_bindings_allowed():
    payload = {**VALID_REGISTRY, "bindings": []}
    errors = validate_models_yaml.validate_registry(payload)
    assert errors == []


def test_validator_script_runs_clean_on_default_path():
    """Integration: run validator on actual models.yaml in repo root."""
    import subprocess

    result = subprocess.run(
        ["uv", "run", "python", "scripts/validate_models_yaml.py"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"validator failed: {result.stderr}"
    assert "models.yaml valid" in result.stdout
