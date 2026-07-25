"""M201 S01: MiniMaxLLMClient contract tests (mock HTTP, no live network)."""

from __future__ import annotations

from research_graph.domain.ports import EXTRACTION_KIND_ENTITIES, EXTRACTION_KIND_RELATIONS
from research_graph.infrastructure.llm.minimax_client import MiniMaxLLMClient


def _tool_response(tool_name: str, tool_input: dict) -> dict:
    return {
        "content": [
            {"type": "thinking", "thinking": "omitted"},
            {"type": "tool_use", "name": tool_name, "input": tool_input},
        ]
    }


def test_extract_entities_valid_tool_use() -> None:
    captured: dict = {}

    def fake_post(method, url, headers, body):
        captured["method"] = method
        captured["url"] = url
        captured["headers"] = dict(headers)
        captured["body"] = dict(body)
        return _tool_response(
            "extract_entities",
            {
                "entities": [
                    {
                        "entity_type": "method",
                        "canonical_name": "attention",
                        "confidence": 0.9,
                        "evidence_hint": "chunk",
                    }
                ]
            },
        )

    client = MiniMaxLLMClient(
        api_key="test-key-not-real-xxxxxxxxxxxx",
        model="MiniMax-M2.7-highspeed",
        endpoint="https://api.minimax.io/anthropic/v1/messages",
        http_post_json=fake_post,
    )
    result = client.extract(
        "Extract entities from redacted chunk about attention mechanisms.",
        EXTRACTION_KIND_ENTITIES,
    )
    assert "entities" in result
    assert result["entities"][0]["canonical_name"] == "attention"
    assert captured["method"] == "POST"
    assert captured["headers"]["X-Api-Key"] == "test-key-not-real-xxxxxxxxxxxx"
    assert captured["body"]["tool_choice"]["name"] == "extract_entities"
    assert client.last_diagnostics["valid"] is True
    assert client.last_diagnostics["credential_value_logged"] is False


def test_extract_relations_valid_tool_use() -> None:
    def fake_post(method, url, headers, body):
        return _tool_response(
            "extract_relations",
            {
                "relations": [
                    {
                        "relation_type": "uses",
                        "from_name": "model",
                        "to_name": "attention",
                        "confidence": 0.8,
                    }
                ]
            },
        )

    client = MiniMaxLLMClient(
        api_key="k" * 20,
        http_post_json=fake_post,
    )
    result = client.extract("redacted chunk", EXTRACTION_KIND_RELATIONS)
    assert result["relations"][0]["from_name"] == "model"


def test_extract_fail_closed_on_invalid_tool() -> None:
    def fake_post(method, url, headers, body):
        return {"content": [{"type": "text", "text": "not a tool"}]}

    client = MiniMaxLLMClient(api_key="k" * 20, http_post_json=fake_post)
    assert client.extract("redacted chunk", "entities") == {}
    assert client.last_diagnostics["valid"] is False
    assert "missing_tool_use" in client.last_diagnostics["diagnostic_codes"]


def test_extract_fail_closed_on_transport_error() -> None:
    def fake_post(method, url, headers, body):
        raise TimeoutError("network")

    client = MiniMaxLLMClient(api_key="k" * 20, http_post_json=fake_post)
    assert client.extract("redacted chunk", "entities") == {}
    assert any("transport" in c for c in client.last_diagnostics["diagnostic_codes"])


def test_extract_fail_closed_missing_api_key() -> None:
    client = MiniMaxLLMClient(api_key=None, http_post_json=lambda *a, **k: {})
    # force empty key even if env has one
    client.api_key = None
    assert client.extract("redacted chunk", "entities") == {}
    assert "missing_api_key" in client.last_diagnostics["diagnostic_codes"]


def test_unsupported_kind_returns_empty() -> None:
    client = MiniMaxLLMClient(api_key="k" * 20, http_post_json=lambda *a, **k: {})
    assert client.extract("redacted", "unknown_kind") == {}
