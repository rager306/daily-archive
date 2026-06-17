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


def test_duplicate_endpoint_is_allowed_for_provider_compatible_models():
    payload = {
        **VALID_REGISTRY,
        "models": [
            VALID_REGISTRY["models"][0],
            {**VALID_REGISTRY["models"][0], "id": "minimax-m3-openai", "provider": "openai"},
        ],
    }
    errors = validate_models_yaml.validate_registry(payload)
    assert errors == []


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


# ---------------------------------------------------------------------------
# S02: Python registry + helper integration tests
# ---------------------------------------------------------------------------


def test_load_models_registry_returns_models_and_bindings():
    from research_graph.llm.models_registry import load_models_registry, reset_cache
    reset_cache()
    registry = load_models_registry()
    assert len(registry.models) >= 2
    assert "minimax-m3-512k-anthropic" in registry.models
    assert "minimax-m3-openai" in registry.models
    assert len(registry.bindings) >= 1
    assert "article-artifact-classify" in registry.bindings


def test_get_model_raises_keyerror_on_unknown():
    from research_graph.llm.models_registry import get_model, load_models_registry, reset_cache
    reset_cache()
    registry = load_models_registry()
    with pytest.raises(KeyError, match="nonexistent"):
        get_model(registry, "nonexistent-model-id")


def test_get_model_for_binding_resolves_to_correct_model():
    from research_graph.llm.models_registry import get_model_for_binding, load_models_registry, reset_cache
    reset_cache()
    registry = load_models_registry()
    resolved = get_model_for_binding(registry, "article-artifact-classify")
    assert resolved.id == "minimax-m3-512k-anthropic"
    assert resolved.provider == "anthropic"
    assert resolved.endpoint.startswith("https://")


def test_compute_work_id_is_deterministic():
    from research_graph.llm.models_registry import compute_work_id, reset_cache
    reset_cache()
    args = dict(
        model_id="minimax-m3-512k-anthropic",
        binding_id="article-artifact-classify",
        input_data={"paper_id": "2507.19457", "version": 1},
        prompt_data={"task": "classify", "max_tokens": 1024},
    )
    w1 = compute_work_id(**args)
    w2 = compute_work_id(**args)
    assert w1 == w2
    assert len(w1) == 64  # sha256 hex digest length


def test_compute_work_id_changes_with_input():
    from research_graph.llm.models_registry import compute_work_id, reset_cache
    reset_cache()
    base = dict(
        model_id="minimax-m3-512k-anthropic",
        binding_id="article-artifact-classify",
        input_data={"paper_id": "2507.19457"},
        prompt_data={"task": "classify"},
    )
    w1 = compute_work_id(**base)
    w2 = compute_work_id(**{**base, "input_data": {"paper_id": "2507.99999"}})
    assert w1 != w2


def test_compute_work_id_changes_with_prompt():
    from research_graph.llm.models_registry import compute_work_id, reset_cache
    reset_cache()
    base = dict(
        model_id="minimax-m3-512k-anthropic",
        binding_id="article-artifact-classify",
        input_data={"paper_id": "2507.19457"},
        prompt_data={"task": "classify"},
    )
    w1 = compute_work_id(**base)
    w2 = compute_work_id(**{**base, "prompt_data": {"task": "summarize"}})
    assert w1 != w2


def test_compute_work_id_changes_with_run_id():
    from research_graph.llm.models_registry import compute_work_id, reset_cache
    reset_cache()
    base = dict(
        model_id="minimax-m3-512k-anthropic",
        binding_id="article-artifact-classify",
        input_data={"paper_id": "2507.19457"},
        prompt_data={"task": "classify"},
    )
    w1 = compute_work_id(**base, run_id="run-A")
    w2 = compute_work_id(**base, run_id="run-B")
    assert w1 != w2


def test_compute_work_id_uses_registry_defaults_when_omitted():
    from research_graph.llm.models_registry import compute_work_id, reset_cache
    reset_cache()
    # Without tool_version/policy_version, should pull from registry
    w = compute_work_id(
        model_id="minimax-m3-512k-anthropic",
        binding_id="article-artifact-classify",
        input_data={"x": 1},
        prompt_data={"y": 2},
    )
    assert isinstance(w, str) and len(w) == 64
    # Same call should match
    w2 = compute_work_id(
        model_id="minimax-m3-512k-anthropic",
        binding_id="article-artifact-classify",
        input_data={"x": 1},
        prompt_data={"y": 2},
    )
    assert w == w2


def test_build_minimax_structured_request_uses_registry_by_default():
    from research_graph.llm.minimax_structured import build_minimax_structured_request
    from research_graph.llm.models_registry import get_model_for_binding, load_models_registry, reset_cache
    reset_cache()
    registry = load_models_registry()
    expected = get_model_for_binding(registry, "article-artifact-classify")

    request = build_minimax_structured_request(
        prompt="Classify this synthetic paper.",
        tool_name="classify_artifact",
        tool_description="Tool description",
        input_schema={"type": "object"},
        payload_class="synthetic",
    )
    assert request.body["model"] == expected.model_name
    assert request.endpoint == expected.endpoint
    assert request.body["model"] == "MiniMax-M3-512k"
    assert request.endpoint == "https://api.minimax.io/anthropic/v1/messages"


def test_build_minimax_structured_request_explicit_model_bypasses_registry():
    from research_graph.llm.minimax_structured import build_minimax_structured_request
    request = build_minimax_structured_request(
        prompt="test",
        tool_name="tool",
        tool_description="desc",
        input_schema={"type": "object"},
        payload_class="synthetic",
        model="custom-model-name",
        endpoint="https://custom.example.com/v1/messages",
    )
    assert request.body["model"] == "custom-model-name"
    assert request.endpoint == "https://custom.example.com/v1/messages"
