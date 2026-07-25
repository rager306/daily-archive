"""M254 S03: binding-driven PaperSummarizer via 9router."""

from __future__ import annotations

from typing import Any

import pytest

from research_graph.infrastructure.llm.ninerouter_client import NineRouterChatResult
from research_graph.infrastructure.llm.summarizer import (
    BINDING_SUMMARY_DEFAULT,
    BINDING_SUMMARY_FALLBACK,
    BINDING_SUMMARY_QUALITY,
    PaperSummarizer,
    PaperSummary,
    ROLE_DEFAULT,
    ROLE_FALLBACK,
    ROLE_QUALITY,
    parse_paper_summary_text,
)


SAMPLE_TEXT = """HEADLINE: Predictive state representations for dynamical systems
WHAT IT DOES: The paper introduces PSRs as an alternative to HMMs.
It proves theoretical properties of the representation.
WHY IT MATTERS: This gives a compact way to model dynamical systems from observations.
ANALOGY: Think of it like summarizing a movie by what happens next rather than hidden cast lists."""


class _FakeChat:
    def __init__(self, text: str = SAMPLE_TEXT, ok: bool = True) -> None:
        self.text = text
        self.ok = ok
        self.calls: list[dict[str, Any]] = []
        self.last_diagnostics: dict[str, Any] = {"provider": "ninerouter"}

    def chat(
        self,
        *,
        model: str,
        messages: list[dict[str, str]],
        max_tokens: int = 700,
        temperature: float = 0.2,
        extra_body: dict[str, Any] | None = None,
    ) -> NineRouterChatResult:
        self.calls.append(
            {
                "model": model,
                "messages": messages,
                "max_tokens": max_tokens,
                "temperature": temperature,
                "extra_body": extra_body,
            }
        )
        if not self.ok:
            return NineRouterChatResult(
                ok=False,
                text="",
                content="",
                reasoning_content="",
                model=model,
                usage=None,
                error="transport: failed",
            )
        return NineRouterChatResult(
            ok=True,
            text=self.text,
            content=self.text,
            reasoning_content="",
            model=model,
            usage={"total_tokens": 10},
            error=None,
        )


def test_parse_shared_helper() -> None:
    summary = parse_paper_summary_text(SAMPLE_TEXT)
    assert isinstance(summary, PaperSummary)
    assert "Predictive" in summary.headline
    assert summary.analogy.startswith("Think of it like")


def test_default_role_uses_agnes_binding() -> None:
    fake = _FakeChat()
    summarizer = PaperSummarizer(client=fake, role=ROLE_DEFAULT)
    summary = summarizer.summarize("Title", "Abstract body")
    assert "Predictive" in summary.headline
    assert fake.calls, "client must be invoked"
    assert fake.calls[0]["model"] == "agnes-ai/agnes-2.0-flash"
    assert summarizer.last_diagnostics["binding_id"] == BINDING_SUMMARY_DEFAULT
    assert summarizer.last_diagnostics["model_name"] == "agnes-ai/agnes-2.0-flash"
    assert summarizer.last_diagnostics["import_eligible"] is False
    # prompt must stay title+abstract faithful
    prompt = fake.calls[0]["messages"][0]["content"]
    assert "Title" in prompt
    assert "Abstract body" in prompt
    assert "later historical" not in prompt.lower() or "Do not invent" in prompt


def test_quality_role_uses_minimax_binding() -> None:
    fake = _FakeChat()
    summarizer = PaperSummarizer(client=fake, role=ROLE_QUALITY)
    summarizer.summarize("T", "A")
    assert fake.calls[0]["model"] == "minimax/MiniMax-M2.7-highspeed"
    assert summarizer.last_diagnostics["binding_id"] == BINDING_SUMMARY_QUALITY


def test_fallback_role_uses_grok_binding() -> None:
    fake = _FakeChat()
    summarizer = PaperSummarizer(client=fake, role=ROLE_FALLBACK)
    summarizer.summarize("T", "A")
    assert fake.calls[0]["model"] == "xai/grok-code-fast-1"
    assert summarizer.last_diagnostics["binding_id"] == BINDING_SUMMARY_FALLBACK


def test_explicit_model_overrides_binding() -> None:
    fake = _FakeChat()
    summarizer = PaperSummarizer(client=fake, role=ROLE_DEFAULT, model_name="custom/model")
    summarizer.summarize("T", "A")
    assert fake.calls[0]["model"] == "custom/model"
    assert summarizer.last_diagnostics["model_name"] == "custom/model"


def test_chat_failure_raises() -> None:
    fake = _FakeChat(ok=False)
    summarizer = PaperSummarizer(client=fake, role=ROLE_DEFAULT)
    with pytest.raises(ValueError, match="ninerouter_chat_failed"):
        summarizer.summarize("T", "A")


def test_incomplete_parse_raises() -> None:
    fake = _FakeChat(text="HEADLINE: only one field")
    summarizer = PaperSummarizer(client=fake, role=ROLE_DEFAULT)
    with pytest.raises(ValueError, match="Could not parse"):
        summarizer.summarize("T", "A")


def test_no_minimax_cloud_hardcode_on_primary_path() -> None:
    import inspect

    from research_graph.infrastructure.llm import summarizer as mod

    src = inspect.getsource(mod.PaperSummarizer)
    assert "api.minimax.io" not in src
    assert "MiniMax-M3-512k" not in src
