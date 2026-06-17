"""Provider-neutral LLM configuration helpers.

This module is intentionally pure: it reads provider-specific configuration
from a supplied environment mapping, exposes sanitized diagnostics, and can
produce short-lived Anthropic-compatible runtime environment mappings for a
single provider client/subprocess. It never performs network I/O and never
mutates ``os.environ``.

Formerly: src/arxiv_archive/llm_provider_config.py
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from typing import Mapping, TypeAlias, get_args

LLMProvider: TypeAlias = str
CompressionMode: TypeAlias = str

PROVIDER_GLM_ZAI = "glm_zai"
PROVIDER_MINIMAX = "minimax"
SUPPORTED_PROVIDERS: tuple[str, ...] = (PROVIDER_GLM_ZAI, PROVIDER_MINIMAX)

COMPRESSION_NONE = "none"
COMPRESSION_PROVIDER_NATIVE = "provider_native"
COMPRESSION_HEADROOM_CANDIDATE = "headroom_candidate"
SUPPORTED_COMPRESSION_MODES: tuple[str, ...] = (
    COMPRESSION_NONE,
    COMPRESSION_PROVIDER_NATIVE,
    COMPRESSION_HEADROOM_CANDIDATE,
)

GLM_DEFAULT_ANTHROPIC_BASE_URL = "https://api.z.ai/api/anthropic"
GLM_DEFAULT_CHAT_COMPLETIONS_URL = "https://api.z.ai/api/paas/v4/chat/completions"
GLM_DEFAULT_MODEL = "glm-5.2"
GLM_DEFAULT_SMALL_FAST_MODEL = "GLM-4.5-Air"
GLM_DEFAULT_TIMEOUT_MS = "3000000"

MINIMAX_DEFAULT_ANTHROPIC_BASE_URL = "https://api.minimax.io/anthropic/v1"
MINIMAX_DEFAULT_OPENAI_BASE_URL = "https://api.minimax.io/v1"
MINIMAX_DEFAULT_MODEL = "MiniMax-M2.7-highspeed"
MINIMAX_DEFAULT_SMALL_FAST_MODEL = "MiniMax-M2.7-highspeed"
MINIMAX_DEFAULT_TIMEOUT_MS = "300000"


class LLMProviderConfigError(ValueError):
    """Base error for invalid LLM provider configuration."""


class MissingProviderSecret(LLMProviderConfigError):
    """Raised when a runtime mapping needs a missing provider secret."""


@dataclass(frozen=True, repr=False)
class LLMProviderConfig:
    """Provider config loaded from namespaced environment variables.

    ``api_key`` is intentionally excluded from repr and sanitized output. Use
    ``to_anthropic_runtime_env`` only at the boundary where a short-lived
    provider-specific client/subprocess is launched.
    """

    provider: LLMProvider
    api_key_env: str
    api_key: str | None
    anthropic_base_url_env: str
    anthropic_base_url: str
    model_env: str
    model: str
    small_fast_model_env: str
    small_fast_model: str | None
    api_timeout_ms_env: str
    api_timeout_ms: str | None
    compression_mode: CompressionMode = COMPRESSION_NONE
    chat_completions_url_env: str | None = None
    chat_completions_url: str | None = None
    openai_base_url_env: str | None = None
    openai_base_url: str | None = None

    @property
    def api_key_present(self) -> bool:
        return bool(self.api_key)

    def __repr__(self) -> str:
        sanitized = self.to_sanitized_dict()
        return f"LLMProviderConfig({sanitized!r})"

    def to_sanitized_dict(self) -> dict[str, object]:
        """Return diagnostics without secret values."""

        payload: dict[str, object] = {
            "provider": self.provider,
            "api_key_env": self.api_key_env,
            "api_key_present": self.api_key_present,
            "anthropic_base_url_env": self.anthropic_base_url_env,
            "anthropic_base_url": self.anthropic_base_url,
            "model_env": self.model_env,
            "model": self.model,
            "small_fast_model_env": self.small_fast_model_env,
            "small_fast_model": self.small_fast_model,
            "api_timeout_ms_env": self.api_timeout_ms_env,
            "api_timeout_ms": self.api_timeout_ms,
            "compression_mode": self.compression_mode,
        }
        if self.chat_completions_url_env:
            payload["chat_completions_url_env"] = self.chat_completions_url_env
            payload["chat_completions_url"] = self.chat_completions_url
        if self.openai_base_url_env:
            payload["openai_base_url_env"] = self.openai_base_url_env
            payload["openai_base_url"] = self.openai_base_url
        return payload

    def to_anthropic_runtime_env(self) -> dict[str, str]:
        """Build a short-lived Anthropic-compatible runtime env mapping.

        The returned dict is suitable for passing to a subprocess/client env. It
        does not mutate global process environment.
        """

        if not self.api_key:
            raise MissingProviderSecret(
                f"{self.api_key_env} not set; cannot build Anthropic runtime env"
            )
        runtime_env = {
            "ANTHROPIC_AUTH_TOKEN": self.api_key,
            "ANTHROPIC_BASE_URL": self.anthropic_base_url,
            "ANTHROPIC_MODEL": self.model,
        }
        if self.small_fast_model:
            runtime_env["ANTHROPIC_SMALL_FAST_MODEL"] = self.small_fast_model
        if self.api_timeout_ms:
            runtime_env["API_TIMEOUT_MS"] = self.api_timeout_ms
        return runtime_env


def _get(environ: Mapping[str, str], key: str, default: str | None = None) -> str | None:
    value = environ.get(key)
    if value is None or value == "":
        return default
    return value


def _validate_provider(provider: str) -> None:
    if provider not in SUPPORTED_PROVIDERS:
        supported = ", ".join(SUPPORTED_PROVIDERS)
        raise LLMProviderConfigError(f"unsupported provider {provider!r}; expected one of: {supported}")


def _validate_compression_mode(compression_mode: str) -> None:
    if compression_mode not in SUPPORTED_COMPRESSION_MODES:
        supported = ", ".join(SUPPORTED_COMPRESSION_MODES)
        raise LLMProviderConfigError(
            f"unsupported compression_mode {compression_mode!r}; expected one of: {supported}"
        )


def load_provider_config(
    provider: str,
    environ: Mapping[str, str] | None = None,
    *,
    compression_mode: str = COMPRESSION_NONE,
) -> LLMProviderConfig:
    """Load provider config from namespaced environment variables.

    Args:
        provider: ``glm_zai`` or ``minimax``.
        environ: Environment mapping. Defaults to ``os.environ``.
        compression_mode: One of ``none``, ``provider_native``, or
            ``headroom_candidate``. This is represented only; no compression
            dependency is imported or invoked.
    """

    _validate_provider(provider)
    _validate_compression_mode(compression_mode)
    env = os.environ if environ is None else environ

    if provider == PROVIDER_GLM_ZAI:
        return LLMProviderConfig(
            provider=PROVIDER_GLM_ZAI,
            api_key_env="GLM_API_KEY",
            api_key=_get(env, "GLM_API_KEY"),
            anthropic_base_url_env="GLM_ANTHROPIC_BASE_URL",
            anthropic_base_url=_get(
                env, "GLM_ANTHROPIC_BASE_URL", GLM_DEFAULT_ANTHROPIC_BASE_URL
            )
            or GLM_DEFAULT_ANTHROPIC_BASE_URL,
            chat_completions_url_env="GLM_CHAT_COMPLETIONS_URL",
            chat_completions_url=_get(
                env, "GLM_CHAT_COMPLETIONS_URL", GLM_DEFAULT_CHAT_COMPLETIONS_URL
            ),
            model_env="GLM_MODEL",
            model=_get(env, "GLM_MODEL", GLM_DEFAULT_MODEL) or GLM_DEFAULT_MODEL,
            small_fast_model_env="GLM_SMALL_FAST_MODEL",
            small_fast_model=_get(env, "GLM_SMALL_FAST_MODEL", GLM_DEFAULT_SMALL_FAST_MODEL),
            api_timeout_ms_env="GLM_API_TIMEOUT_MS",
            api_timeout_ms=_get(env, "GLM_API_TIMEOUT_MS", GLM_DEFAULT_TIMEOUT_MS),
            compression_mode=compression_mode,
        )

    return LLMProviderConfig(
        provider=PROVIDER_MINIMAX,
        api_key_env="MINIMAX_API_KEY",
        api_key=_get(env, "MINIMAX_API_KEY"),
        anthropic_base_url_env="MINIMAX_ANTHROPIC_BASE_URL",
        anthropic_base_url=_get(
            env, "MINIMAX_ANTHROPIC_BASE_URL", MINIMAX_DEFAULT_ANTHROPIC_BASE_URL
        )
        or MINIMAX_DEFAULT_ANTHROPIC_BASE_URL,
        openai_base_url_env="MINIMAX_OPENAI_BASE_URL",
        openai_base_url=_get(env, "MINIMAX_OPENAI_BASE_URL", MINIMAX_DEFAULT_OPENAI_BASE_URL),
        model_env="MINIMAX_MODEL",
        model=_get(env, "MINIMAX_MODEL", MINIMAX_DEFAULT_MODEL) or MINIMAX_DEFAULT_MODEL,
        small_fast_model_env="MINIMAX_SMALL_FAST_MODEL",
        small_fast_model=_get(
            env, "MINIMAX_SMALL_FAST_MODEL", MINIMAX_DEFAULT_SMALL_FAST_MODEL
        ),
        api_timeout_ms_env="MINIMAX_API_TIMEOUT_MS",
        api_timeout_ms=_get(env, "MINIMAX_API_TIMEOUT_MS", MINIMAX_DEFAULT_TIMEOUT_MS),
        compression_mode=compression_mode,
    )
