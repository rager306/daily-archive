"""M201 S02: GLMLLMClient contract tests (mock HTTP)."""

from __future__ import annotations

from research_graph.application.chunk_extraction import (
    ChunkExtractionRequest,
    ChunkExtractionUseCase,
)
from research_graph.domain.ports import EXTRACTION_KIND_ENTITIES
from research_graph.infrastructure.llm.glm_client import GLMLLMClient


def _tool_response(tool_name: str, tool_input: dict) -> dict:
    return {
        "content": [
            {"type": "tool_use", "name": tool_name, "input": tool_input},
        ]
    }


def test_glm_extract_entities_valid() -> None:
    captured: dict = {}

    def fake_post(method, url, headers, body):
        captured.update(method=method, url=url, headers=dict(headers), body=dict(body))
        return _tool_response(
            "extract_entities",
            {
                "entities": [
                    {
                        "entity_type": "method",
                        "canonical_name": "attention",
                        "confidence": 0.88,
                    }
                ]
            },
        )

    client = GLMLLMClient(
        api_key="glm-test-key-xxxxxxxxxxxx",
        model="glm-5.2",
        endpoint="https://api.z.ai/api/anthropic/v1/messages",
        http_post_json=fake_post,
    )
    result = client.extract("redacted chunk about attention", EXTRACTION_KIND_ENTITIES)
    assert result["entities"][0]["canonical_name"] == "attention"
    assert captured["headers"]["Authorization"].startswith("Bearer ")
    assert "glm-test-key" in captured["headers"]["Authorization"]
    assert captured["body"]["tool_choice"]["name"] == "extract_entities"
    assert client.last_diagnostics["provider"] == "glm"
    assert client.last_diagnostics["valid"] is True
    assert client.last_diagnostics["credential_value_logged"] is False


def test_glm_fail_closed_invalid_response() -> None:
    client = GLMLLMClient(
        api_key="k" * 20,
        http_post_json=lambda *a, **k: {"content": [{"type": "text", "text": "x"}]},
    )
    assert client.extract("redacted", "entities") == {}
    assert client.last_diagnostics["valid"] is False


def test_glm_fail_closed_transport() -> None:
    def boom(*a, **k):
        raise ConnectionError("down")

    client = GLMLLMClient(api_key="k" * 20, http_post_json=boom)
    assert client.extract("redacted", "entities") == {}
    assert any("transport" in c for c in client.last_diagnostics["diagnostic_codes"])


def test_chunk_extraction_works_with_glm_client_mock() -> None:
    def fake_post(method, url, headers, body):
        return _tool_response(
            "extract_entities",
            {
                "entities": [
                    {
                        "entity_type": "model",
                        "canonical_name": "Transformer",
                        "confidence": 0.95,
                        "evidence_hint": "chunk",
                    }
                ]
            },
        )

    client = GLMLLMClient(api_key="k" * 20, http_post_json=fake_post)
    uc = ChunkExtractionUseCase(llm_provider=client)
    result = uc.run(
        ChunkExtractionRequest(
            source_id="arxiv:glm.chunk",
            text_parts=["Transformers use self-attention for sequence modeling."],
        )
    )
    assert result.status == "done"
    assert result.entity_count >= 1
