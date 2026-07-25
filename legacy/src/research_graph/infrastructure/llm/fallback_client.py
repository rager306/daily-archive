"""Bounded primary→fallback LLMClientPort with provider provenance (M201 S03).

Does not invent a new queue or composition root: wraps two existing
:class:`~research_graph.domain.ports.LLMClientPort` implementations and records
which provider actually answered.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Protocol, runtime_checkable


@runtime_checkable
class _Port(Protocol):
    last_diagnostics: dict[str, Any]

    def extract(
        self, prompt: str, kind: str, *, context: dict[str, Any] | None = None
    ) -> dict[str, Any]: ...


def _provider_name(client: Any, default: str) -> str:
    diag = getattr(client, "last_diagnostics", None)
    if isinstance(diag, dict) and diag.get("provider"):
        return str(diag["provider"])
    return default


def _should_fallback(result: dict[str, Any], diagnostics: dict[str, Any]) -> bool:
    """Fallback when result is empty or primary reported transport/auth failure."""
    if result:
        # Non-empty structured payload — primary succeeded.
        return False
    codes = diagnostics.get("diagnostic_codes") or ()
    code_text = " ".join(str(c) for c in codes)
    if any(
        marker in code_text
        for marker in (
            "transport:",
            "missing_api_key",
            "missing_tool_use",
            "missing_content_blocks",
            "request_build:",
        )
    ):
        return True
    # Empty dict without codes also triggers bounded fallback (quota/soft fail).
    return True


@dataclass
class FallbackLLMClient:
    """Try primary, then secondary; expose provenance in last_diagnostics."""

    primary: _Port
    secondary: _Port
    primary_name: str = "primary"
    secondary_name: str = "secondary"
    last_diagnostics: dict[str, Any] = field(default_factory=dict)

    def extract(
        self, prompt: str, kind: str, *, context: dict[str, Any] | None = None
    ) -> dict[str, Any]:
        primary_result = self.primary.extract(prompt, kind, context=context)
        primary_diag = dict(getattr(self.primary, "last_diagnostics", {}) or {})
        primary_provider = _provider_name(self.primary, self.primary_name)

        if not _should_fallback(primary_result, primary_diag):
            self.last_diagnostics = {
                "provider": primary_provider,
                "primary_provider": primary_provider,
                "used_provider": primary_provider,
                "fallback_used": False,
                "kind": kind,
                "valid": primary_diag.get("valid", True),
                "primary_diagnostic_codes": primary_diag.get("diagnostic_codes", ()),
                "secondary_diagnostic_codes": (),
                "credential_value_logged": False,
            }
            return primary_result

        secondary_result = self.secondary.extract(prompt, kind, context=context)
        secondary_diag = dict(getattr(self.secondary, "last_diagnostics", {}) or {})
        secondary_provider = _provider_name(self.secondary, self.secondary_name)

        used = secondary_provider if secondary_result else primary_provider
        # If secondary also empty, still mark fallback attempted.
        if secondary_result:
            used = secondary_provider
        self.last_diagnostics = {
            "provider": used if secondary_result else primary_provider,
            "primary_provider": primary_provider,
            "used_provider": used if secondary_result else primary_provider,
            "fallback_used": True,
            "fallback_succeeded": bool(secondary_result),
            "kind": kind,
            "valid": bool(secondary_result) or primary_diag.get("valid", False),
            "primary_diagnostic_codes": primary_diag.get("diagnostic_codes", ()),
            "secondary_diagnostic_codes": secondary_diag.get("diagnostic_codes", ()),
            "credential_value_logged": False,
        }
        return secondary_result if secondary_result else primary_result


__all__ = ["FallbackLLMClient"]
