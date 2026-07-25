"""M201 S01: ChunkExtractionUseCase tracer tests."""

from __future__ import annotations

from research_graph.application.chunk_extraction import (
    ChunkExtractionRequest,
    ChunkExtractionUseCase,
)
from research_graph.domain.schema import ExtractionPatch


class _FakeLLM:
    def __init__(self, payload: dict | None = None):
        self.payload = payload or {
            "entities": [
                {
                    "entity_type": "method",
                    "canonical_name": "Sparse Attention",
                    "confidence": 0.91,
                    "evidence_hint": "chunk-1",
                }
            ]
        }
        self.calls: list[tuple] = []

    def extract(self, prompt: str, kind: str, *, context=None) -> dict:
        self.calls.append((kind, prompt[:80], context))
        if kind in ("entities", "EXTRACTION_KIND_ENTITIES") or kind == "entities":
            return self.payload if "entities" in self.payload else {"entities": []}
        if kind == "relations":
            return {"relations": self.payload.get("relations", [])}
        return {}


def test_empty_chunk_returns_empty() -> None:
    uc = ChunkExtractionUseCase(llm_provider=_FakeLLM())
    result = uc.run(ChunkExtractionRequest(source_id="s", text_parts=[]))
    assert result.status == "empty"
    assert result.entity_count == 0
    assert result.safety["graph_writes_authorized"] is False


def test_chunk_tracer_produces_typed_entity_candidates() -> None:
    fake = _FakeLLM()
    uc = ChunkExtractionUseCase(llm_provider=fake)
    result = uc.run(
        ChunkExtractionRequest(
            source_id="arxiv:2605.18747",
            text_parts=[
                "Sparse attention reduces cost of self-attention mechanisms in transformers."
            ],
        )
    )
    assert result.status == "done"
    assert result.entity_count >= 1
    assert fake.calls, "LLMClientPort.extract must be invoked"
    assert any(c[0] == "entities" for c in fake.calls)
    assert "core_entity_extractor" in result.stage_output_keys
    assert result.safety["fact_promotion_authorized"] is False
    assert result.safety["production_import_authorized"] is False
    if result.extraction_patch is not None:
        assert isinstance(result.extraction_patch, ExtractionPatch)
        assert result.extraction_patch.entities


def test_chunk_tracer_fail_closed_when_llm_returns_empty() -> None:
    fake = _FakeLLM(payload={"entities": []})
    uc = ChunkExtractionUseCase(llm_provider=fake)
    result = uc.run(
        ChunkExtractionRequest(
            source_id="arxiv:empty",
            text_parts=["Redacted technical chunk with methods and models."],
        )
    )
    assert result.status == "done"
    assert result.entity_count == 0
