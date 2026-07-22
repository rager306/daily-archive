"""M201 S03: FallbackLLMClient provenance tests."""

from __future__ import annotations

from research_graph.application.chunk_extraction import (
    ChunkExtractionRequest,
    ChunkExtractionUseCase,
)
from research_graph.infrastructure.llm.fallback_client import FallbackLLMClient


class _StubPort:
    def __init__(self, name: str, result: dict | None = None, codes: tuple = ()):
        self.name = name
        self.result = result if result is not None else {}
        self.codes = codes
        self.calls = 0
        self.last_diagnostics: dict = {
            "provider": name,
            "valid": bool(result),
            "diagnostic_codes": codes,
            "credential_value_logged": False,
        }

    def extract(self, prompt: str, kind: str, *, context=None) -> dict:
        self.calls += 1
        self.last_diagnostics = {
            "provider": self.name,
            "valid": bool(self.result),
            "diagnostic_codes": self.codes,
            "credential_value_logged": False,
            "kind": kind,
        }
        return dict(self.result)


def test_primary_success_no_fallback() -> None:
    primary = _StubPort(
        "minimax",
        result={"entities": [{"entity_type": "m", "canonical_name": "A", "confidence": 0.9}]},
    )
    secondary = _StubPort("glm", result={"entities": []})
    client = FallbackLLMClient(primary=primary, secondary=secondary)
    out = client.extract("chunk", "entities")
    assert out["entities"][0]["canonical_name"] == "A"
    assert primary.calls == 1
    assert secondary.calls == 0
    assert client.last_diagnostics["fallback_used"] is False
    assert client.last_diagnostics["used_provider"] == "minimax"


def test_primary_empty_triggers_fallback() -> None:
    primary = _StubPort("minimax", result={}, codes=("transport:TimeoutError",))
    secondary = _StubPort(
        "glm",
        result={"entities": [{"entity_type": "m", "canonical_name": "B", "confidence": 0.8}]},
    )
    client = FallbackLLMClient(primary=primary, secondary=secondary)
    out = client.extract("chunk", "entities")
    assert out["entities"][0]["canonical_name"] == "B"
    assert primary.calls == 1
    assert secondary.calls == 1
    assert client.last_diagnostics["fallback_used"] is True
    assert client.last_diagnostics["fallback_succeeded"] is True
    assert client.last_diagnostics["used_provider"] == "glm"
    assert client.last_diagnostics["primary_provider"] == "minimax"


def test_both_empty_records_fallback_attempt() -> None:
    primary = _StubPort("minimax", result={}, codes=("missing_api_key",))
    secondary = _StubPort("glm", result={}, codes=("transport:ConnectionError",))
    client = FallbackLLMClient(primary=primary, secondary=secondary)
    out = client.extract("chunk", "entities")
    assert out == {}
    assert client.last_diagnostics["fallback_used"] is True
    assert client.last_diagnostics["fallback_succeeded"] is False


def test_chunk_extraction_with_fallback_client() -> None:
    primary = _StubPort("minimax", result={}, codes=("transport:TimeoutError",))
    secondary = _StubPort(
        "glm",
        result={
            "entities": [
                {
                    "entity_type": "method",
                    "canonical_name": "Attention",
                    "confidence": 0.9,
                    "evidence_hint": "c",
                }
            ]
        },
    )
    client = FallbackLLMClient(primary=primary, secondary=secondary)
    uc = ChunkExtractionUseCase(llm_provider=client)
    result = uc.run(
        ChunkExtractionRequest(
            source_id="arxiv:fb.1",
            text_parts=["Attention mechanisms improve transformer efficiency."],
        )
    )
    assert result.status == "done"
    assert result.entity_count >= 1
    assert client.last_diagnostics["used_provider"] == "glm"
