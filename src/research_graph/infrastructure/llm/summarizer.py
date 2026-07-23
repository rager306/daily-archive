# Formerly: src/research_graph/retrieval/summarizer.py
# Formerly: src/arxiv_archive/summarizer.py

"""Paper summarizers: 9router primary path + legacy MiniMax cloud helper.

M254 S03:
  * :class:`PaperSummarizer` — binding-driven via local 9router
    (``paper-summary-generate-default|quality|fallback``).
  * :class:`MiniMaxSummarizer` — legacy Anthropic-compatible MiniMax cloud path
    kept for backward-compat parse tests; not the ETL primary.

Import eligibility is never set by this module.
"""

from __future__ import annotations

import os
from dataclasses import dataclass, field
from typing import Any, Protocol

import anthropic

from research_graph.infrastructure.llm.models_registry import (
    get_model_for_binding,
    load_models_registry,
)
from research_graph.infrastructure.llm.ninerouter_client import NineRouterChatClient

ROLE_DEFAULT = "default"
ROLE_QUALITY = "quality"
ROLE_FALLBACK = "fallback"

BINDING_SUMMARY_DEFAULT = "paper-summary-generate-default"
BINDING_SUMMARY_QUALITY = "paper-summary-generate-quality"
BINDING_SUMMARY_FALLBACK = "paper-summary-generate-fallback"

_ROLE_TO_BINDING = {
    ROLE_DEFAULT: BINDING_SUMMARY_DEFAULT,
    ROLE_QUALITY: BINDING_SUMMARY_QUALITY,
    ROLE_FALLBACK: BINDING_SUMMARY_FALLBACK,
}

PROMPT_TEMPLATE = """You are a research assistant summarizing research papers. Format EXACTLY as:
HEADLINE: [one sentence]
WHAT IT DOES: [2 sentences]
WHY IT MATTERS: [1 sentence]
ANALOGY: [starts with "Think of it like"]

Use ONLY facts supported by the title and abstract. Do not invent later historical impact or unrelated model lineages.

Paper to summarize:
Title: {title}
Abstract: {abstract}"""

# Legacy MiniMax cloud constants (not primary path).
LEGACY_MINIMAX_MODEL = "MiniMax-M3-512k"
MAX_TOKENS = 1024
TEMPERATURE = 0.7


@dataclass
class PaperSummary:
    """Structured summary of a research paper."""

    headline: str
    what_it_does: str
    why_it_matters: str
    analogy: str


class _ChatPort(Protocol):
    def chat(
        self,
        *,
        model: str,
        messages: Any,
        max_tokens: int = 700,
        temperature: float = 0.2,
        extra_body: Any | None = None,
    ) -> Any: ...


def parse_paper_summary_text(text: str) -> PaperSummary:
    """Parse HEADLINE / WHAT IT DOES / WHY IT MATTERS / ANALOGY blocks."""
    lines = text.split("\n")
    result: dict[str, str] = {}
    current_field: str | None = None
    current_value_parts: list[str] = []

    def clean_field_value(value: str) -> str:
        value = value.strip()
        while True:
            old = value
            for marker in ["**", "*", "_"]:
                if value.startswith(marker):
                    value = value[len(marker) :].strip()
                if value.endswith(marker):
                    value = value[: -len(marker)].strip()
            if value == old:
                return value.strip()

    for line in lines:
        stripped = line.strip()
        while True:
            old = stripped
            for marker in ["**", "*", "_"]:
                if stripped.startswith(marker):
                    stripped = stripped[len(marker) :].strip()
                if stripped.endswith(marker):
                    stripped = stripped[: -len(marker)].strip()
            if stripped == old:
                break
        stripped = stripped.strip()

        if stripped.upper().startswith("HEADLINE:"):
            if current_field and current_value_parts:
                result[current_field] = " ".join(current_value_parts).strip()
            current_field = "headline"
            current_value_parts = [
                clean_field_value(stripped.split(":", 1)[1] if ":" in stripped else "")
            ]
        elif stripped.upper().startswith("WHAT IT DOES:"):
            if current_field and current_value_parts:
                result[current_field] = " ".join(current_value_parts).strip()
            current_field = "what_it_does"
            current_value_parts = [
                clean_field_value(stripped.split(":", 1)[1] if ":" in stripped else "")
            ]
        elif stripped.upper().startswith("WHY IT MATTERS:"):
            if current_field and current_value_parts:
                result[current_field] = " ".join(current_value_parts).strip()
            current_field = "why_it_matters"
            current_value_parts = [
                clean_field_value(stripped.split(":", 1)[1] if ":" in stripped else "")
            ]
        elif stripped.upper().startswith("ANALOGY:"):
            if current_field and current_value_parts:
                result[current_field] = " ".join(current_value_parts).strip()
            current_field = "analogy"
            current_value_parts = [
                clean_field_value(stripped.split(":", 1)[1] if ":" in stripped else "")
            ]
        elif current_field and stripped:
            while True:
                old = stripped
                for marker in ["**", "*", "_"]:
                    if stripped.startswith(marker):
                        stripped = stripped[len(marker) :]
                    if stripped.endswith(marker):
                        stripped = stripped[: -len(marker)]
                if stripped == old:
                    break
            current_value_parts.append(stripped.strip() or "")

    if current_field and current_value_parts:
        result[current_field] = " ".join(current_value_parts).strip()

    headline = result.get("headline", "")
    what_it_does = result.get("what_it_does", "")
    why_it_matters = result.get("why_it_matters", "")
    analogy = result.get("analogy", "")

    if not headline or not what_it_does or not why_it_matters or not analogy:
        raise ValueError(f"Could not parse all required fields from response: {text!r}")

    return PaperSummary(
        headline=headline,
        what_it_does=what_it_does,
        why_it_matters=why_it_matters,
        analogy=analogy,
    )


@dataclass
class PaperSummarizer:
    """Primary paper summarizer via local 9router + models.yaml bindings.

    Roles:
      * default  → paper-summary-generate-default (agnes)
      * quality  → paper-summary-generate-quality (MiniMax-M2.7-highspeed)
      * fallback → paper-summary-generate-fallback (grok-code-fast-1)
    """

    client: _ChatPort | None = None
    role: str = ROLE_DEFAULT
    model_name: str | None = None
    binding_id: str | None = None
    max_tokens: int = MAX_TOKENS
    temperature: float = TEMPERATURE
    last_diagnostics: dict[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if self.client is None:
            self.client = NineRouterChatClient()
        if self.role not in _ROLE_TO_BINDING and self.binding_id is None:
            raise ValueError(
                f"unknown role={self.role!r}; expected one of {sorted(_ROLE_TO_BINDING)}"
            )
        if self.binding_id is None:
            self.binding_id = _ROLE_TO_BINDING[self.role]

    def resolve_model_name(self) -> str:
        if self.model_name:
            return self.model_name
        registry = load_models_registry()
        assert self.binding_id is not None
        return get_model_for_binding(registry, self.binding_id).model_name

    def summarize(self, title: str, abstract: str) -> PaperSummary:
        model = self.resolve_model_name()
        prompt = PROMPT_TEMPLATE.format(title=title, abstract=abstract)
        assert self.client is not None
        result = self.client.chat(
            model=model,
            messages=[{"role": "user", "content": prompt}],
            max_tokens=self.max_tokens,
            temperature=self.temperature,
        )
        self.last_diagnostics = {
            "provider": "ninerouter",
            "binding_id": self.binding_id,
            "role": self.role,
            "model_name": model,
            "ok": bool(getattr(result, "ok", False)),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "credential_value_logged": False,
        }
        if not getattr(result, "ok", False):
            err = getattr(result, "error", None) or "unknown"
            raise ValueError(f"ninerouter_chat_failed: {err}")
        text = str(getattr(result, "text", "") or "").strip()
        if not text:
            raise ValueError("No text content in model response")
        return parse_paper_summary_text(text)


class MiniMaxSummarizer:
    """Legacy MiniMax Anthropic-compatible summarizer (not ETL primary).

    Kept for backward-compat unit tests and optional direct MiniMax use.
    Prefer :class:`PaperSummarizer` for 9router-bound generation.
    """

    def __init__(self, api_key: str | None = None) -> None:
        api_key = api_key or os.environ.get("ANTHROPIC_API_KEY")
        self._client = anthropic.Anthropic(
            api_key="unused-minimax-compat-key",
            auth_token=None,
            default_headers={"X-Api-Key": api_key or ""},
            base_url="https://api.minimax.io/anthropic",
        )

    def summarize(self, title: str, abstract: str) -> PaperSummary:
        prompt = PROMPT_TEMPLATE.format(title=title, abstract=abstract)
        response = self._client.messages.create(
            model=LEGACY_MINIMAX_MODEL,
            max_tokens=MAX_TOKENS,
            temperature=TEMPERATURE,
            messages=[{"role": "user", "content": prompt}],
        )
        text = ""
        for block in response.content:
            if getattr(block, "type", None) == "text":
                text = str(getattr(block, "text", ""))
                break
        if not text:
            raise ValueError("No text content in model response")
        return self._parse(text)

    def _parse(self, text: str) -> PaperSummary:
        return parse_paper_summary_text(text)


# Back-compat alias used by older imports/tests.
MODEL = LEGACY_MINIMAX_MODEL

__all__ = [
    "BINDING_SUMMARY_DEFAULT",
    "BINDING_SUMMARY_FALLBACK",
    "BINDING_SUMMARY_QUALITY",
    "LEGACY_MINIMAX_MODEL",
    "MODEL",
    "MiniMaxSummarizer",
    "PROMPT_TEMPLATE",
    "PaperSummarizer",
    "PaperSummary",
    "ROLE_DEFAULT",
    "ROLE_FALLBACK",
    "ROLE_QUALITY",
    "parse_paper_summary_text",
]
