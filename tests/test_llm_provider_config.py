from __future__ import annotations

import os
from pathlib import Path

import pytest

from research_graph.llm import provider_config as canonical_provider_config
from research_graph.llm.provider_config import (
    COMPRESSION_HEADROOM_CANDIDATE,
    COMPRESSION_NONE,
    LLMProviderConfigError,
    MissingProviderSecret,
    PROVIDER_GLM_ZAI,
    PROVIDER_MINIMAX,
    load_provider_config,
)


SECRET_VALUE = "test-secret-value-should-not-appear-in-diagnostics"


def test_glm_config_loads_namespaced_env_and_maps_to_anthropic_runtime(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("ANTHROPIC_AUTH_TOKEN", raising=False)
    env = {
        "GLM_API_KEY": SECRET_VALUE,
        "GLM_ANTHROPIC_BASE_URL": "https://api.z.ai/api/anthropic",
        "GLM_CHAT_COMPLETIONS_URL": "https://api.z.ai/api/paas/v4/chat/completions",
        "GLM_MODEL": "glm-5.2",
        "GLM_SMALL_FAST_MODEL": "GLM-4.5-Air",
        "GLM_API_TIMEOUT_MS": "3000000",
    }

    config = load_provider_config(PROVIDER_GLM_ZAI, env)

    assert config.provider == PROVIDER_GLM_ZAI
    assert config.api_key_env == "GLM_API_KEY"
    assert config.chat_completions_url == "https://api.z.ai/api/paas/v4/chat/completions"
    assert config.compression_mode == COMPRESSION_NONE
    assert config.to_anthropic_runtime_env() == {
        "ANTHROPIC_AUTH_TOKEN": SECRET_VALUE,
        "ANTHROPIC_BASE_URL": "https://api.z.ai/api/anthropic",
        "ANTHROPIC_MODEL": "glm-5.2",
        "ANTHROPIC_SMALL_FAST_MODEL": "GLM-4.5-Air",
        "API_TIMEOUT_MS": "3000000",
    }
    assert "ANTHROPIC_AUTH_TOKEN" not in os.environ


def test_minimax_config_loads_namespaced_env_and_maps_to_anthropic_runtime() -> None:
    env = {
        "MINIMAX_API_KEY": SECRET_VALUE,
        "MINIMAX_ANTHROPIC_BASE_URL": "https://api.minimax.io/anthropic/v1",
        "MINIMAX_OPENAI_BASE_URL": "https://api.minimax.io/v1",
        "MINIMAX_MODEL": "MiniMax-M2.7-highspeed",
        "MINIMAX_SMALL_FAST_MODEL": "MiniMax-M2.7-highspeed",
        "MINIMAX_API_TIMEOUT_MS": "300000",
    }

    config = load_provider_config(PROVIDER_MINIMAX, env, compression_mode=COMPRESSION_HEADROOM_CANDIDATE)

    assert config.provider == PROVIDER_MINIMAX
    assert config.api_key_env == "MINIMAX_API_KEY"
    assert config.openai_base_url == "https://api.minimax.io/v1"
    assert config.compression_mode == COMPRESSION_HEADROOM_CANDIDATE
    assert config.to_anthropic_runtime_env()["ANTHROPIC_AUTH_TOKEN"] == SECRET_VALUE
    assert config.to_anthropic_runtime_env()["ANTHROPIC_BASE_URL"] == "https://api.minimax.io/anthropic/v1"


def test_sanitized_diagnostics_and_repr_do_not_contain_secret() -> None:
    config = load_provider_config(
        PROVIDER_GLM_ZAI,
        {
            "GLM_API_KEY": SECRET_VALUE,
            "GLM_MODEL": "glm-5.2",
        },
    )

    sanitized = config.to_sanitized_dict()

    assert sanitized["api_key_env"] == "GLM_API_KEY"
    assert sanitized["api_key_present"] is True
    assert SECRET_VALUE not in str(sanitized)
    assert SECRET_VALUE not in repr(config)


def test_missing_secret_is_visible_as_presence_only_and_runtime_mapping_fails() -> None:
    config = load_provider_config(PROVIDER_GLM_ZAI, {})

    assert config.api_key_present is False
    assert config.to_sanitized_dict()["api_key_present"] is False
    with pytest.raises(MissingProviderSecret, match="GLM_API_KEY not set"):
        config.to_anthropic_runtime_env()


def test_invalid_provider_and_compression_mode_are_rejected() -> None:
    with pytest.raises(LLMProviderConfigError, match="unsupported provider"):
        load_provider_config("anthropic", {})

    with pytest.raises(LLMProviderConfigError, match="unsupported compression_mode"):
        load_provider_config(PROVIDER_GLM_ZAI, {}, compression_mode="headroom_enabled")


def test_defaults_are_provider_namespaced_not_generic_anthropic() -> None:
    config = load_provider_config(PROVIDER_GLM_ZAI, {})

    assert config.api_key_env == "GLM_API_KEY"
    assert config.anthropic_base_url_env == "GLM_ANTHROPIC_BASE_URL"
    assert config.model_env == "GLM_MODEL"
    assert config.small_fast_model_env == "GLM_SMALL_FAST_MODEL"
    assert config.api_timeout_ms_env == "GLM_API_TIMEOUT_MS"
    assert config.anthropic_base_url == "https://api.z.ai/api/anthropic"

def test_llm_provider_config_old_module_is_archived_with_canonical_breadcrumb() -> None:
    top_level_archive_path = Path("archive/package-layout-shims/wave-01/src/arxiv_archive/llm_provider_config.py")
    package_archive_path = Path("archive/package-rename-waves/wave-01/src/arxiv_archive/llm/provider_config.py")
    canonical_path = Path("src/research_graph/llm/provider_config.py")

    assert top_level_archive_path.exists()
    assert package_archive_path.exists()
    assert not Path("src/arxiv_archive/llm_provider_config.py").exists()
    assert not Path("src/arxiv_archive/llm/provider_config.py").exists()
    assert "Formerly: src/arxiv_archive/llm/provider_config.py" in canonical_path.read_text(encoding="utf-8")
    assert canonical_provider_config.PROVIDER_GLM_ZAI == PROVIDER_GLM_ZAI
