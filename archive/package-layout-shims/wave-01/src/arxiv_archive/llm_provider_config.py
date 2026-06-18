"""Compatibility shim for the LLM provider config module.

The canonical import path is now ``arxiv_archive.llm.provider_config``.
This module remains to preserve imports created before M079.
"""

from arxiv_archive.llm.provider_config import (
    COMPRESSION_HEADROOM_CANDIDATE,
    COMPRESSION_NONE,
    COMPRESSION_PROVIDER_NATIVE,
    LLMProviderConfig,
    LLMProviderConfigError,
    MissingProviderSecret,
    PROVIDER_GLM_ZAI,
    PROVIDER_MINIMAX,
    SUPPORTED_COMPRESSION_MODES,
    SUPPORTED_PROVIDERS,
    load_provider_config,
)

__all__ = [
    "COMPRESSION_HEADROOM_CANDIDATE",
    "COMPRESSION_NONE",
    "COMPRESSION_PROVIDER_NATIVE",
    "LLMProviderConfig",
    "LLMProviderConfigError",
    "MissingProviderSecret",
    "PROVIDER_GLM_ZAI",
    "PROVIDER_MINIMAX",
    "SUPPORTED_COMPRESSION_MODES",
    "SUPPORTED_PROVIDERS",
    "load_provider_config",
]
