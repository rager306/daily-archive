"""M254 S02: thin local 9router OpenAI-compatible chat client."""

from __future__ import annotations

import pytest

from research_graph.infrastructure.llm.ninerouter_client import (
    NineRouterChatClient,
    NineRouterChatError,
    message_text_from_choice,
)


def test_message_text_prefers_content() -> None:
    msg = {"content": "hello", "reasoning_content": "think"}
    assert message_text_from_choice(msg) == "hello"


def test_message_text_falls_back_to_reasoning() -> None:
    msg = {"content": "", "reasoning_content": "  reason answer  "}
    assert message_text_from_choice(msg) == "reason answer"


def test_message_text_empty() -> None:
    assert message_text_from_choice({}) == ""
    assert message_text_from_choice(None) == ""


def test_chat_success_content(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("NINEROUTER_URL", "http://127.0.0.1:20128")
    monkeypatch.setenv("NINEROUTER_KEY", "test-key-secret")

    def fake_post(method, url, headers, body):
        assert method == "POST"
        assert url.endswith("/v1/chat/completions")
        assert headers.get("Authorization") == "Bearer test-key-secret"
        assert body["model"] == "agnes-ai/agnes-2.0-flash"
        assert body["stream"] is False
        return {
            "choices": [
                {"message": {"role": "assistant", "content": "HEADLINE: ok"}}
            ],
            "usage": {"total_tokens": 12},
        }

    client = NineRouterChatClient(http_post_json=fake_post)
    result = client.chat(
        model="agnes-ai/agnes-2.0-flash",
        messages=[{"role": "user", "content": "summarize"}],
        max_tokens=64,
    )
    assert result.ok is True
    assert result.text == "HEADLINE: ok"
    assert result.content == "HEADLINE: ok"
    assert result.model == "agnes-ai/agnes-2.0-flash"
    assert result.error is None
    assert "test-key-secret" not in repr(client)
    assert "test-key-secret" not in str(client.last_diagnostics)


def test_chat_reasoning_content_fallback(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("NINEROUTER_URL", "http://127.0.0.1:20128")
    monkeypatch.setenv("NINEROUTER_KEY", "k")

    def fake_post(method, url, headers, body):
        del method, url, headers, body
        return {
            "choices": [
                {
                    "message": {
                        "content": "",
                        "reasoning_content": '{"overall": 8}',
                    }
                }
            ]
        }

    client = NineRouterChatClient(http_post_json=fake_post)
    result = client.chat(
        model="glm/glm-5.2",
        messages=[{"role": "user", "content": "judge"}],
    )
    assert result.ok is True
    assert result.text == '{"overall": 8}'
    assert result.reasoning_content == '{"overall": 8}'


def test_chat_http_error_sanitized(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("NINEROUTER_URL", "http://127.0.0.1:20128")
    monkeypatch.setenv("NINEROUTER_KEY", "super-secret-key-xyz")

    def fake_post(method, url, headers, body):
        del method, url, headers, body
        raise RuntimeError("401 unauthorized super-secret-key-xyz")

    client = NineRouterChatClient(http_post_json=fake_post)
    result = client.chat(
        model="agnes-ai/agnes-2.0-flash",
        messages=[{"role": "user", "content": "x"}],
    )
    assert result.ok is False
    assert result.text == ""
    assert result.error is not None
    assert "super-secret-key-xyz" not in result.error
    assert "super-secret-key-xyz" not in str(client.last_diagnostics)


def test_chat_raises_when_configured(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("NINEROUTER_URL", "http://127.0.0.1:20128")
    monkeypatch.delenv("NINEROUTER_KEY", raising=False)

    def fake_post(method, url, headers, body):
        del method, url, headers, body
        raise RuntimeError("boom")

    client = NineRouterChatClient(http_post_json=fake_post, raise_on_error=True)
    with pytest.raises(NineRouterChatError):
        client.chat(
            model="agnes-ai/agnes-2.0-flash",
            messages=[{"role": "user", "content": "x"}],
        )


def test_base_url_normalization(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("NINEROUTER_URL", "http://127.0.0.1:20128/v1")
    monkeypatch.setenv("NINEROUTER_KEY", "k")
    seen: list[str] = []

    def fake_post(method, url, headers, body):
        del method, headers, body
        seen.append(url)
        return {"choices": [{"message": {"content": "pong"}}]}

    client = NineRouterChatClient(http_post_json=fake_post)
    client.chat(model="m", messages=[{"role": "user", "content": "hi"}])
    assert seen[0] == "http://127.0.0.1:20128/v1/chat/completions"
