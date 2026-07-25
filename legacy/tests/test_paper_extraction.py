"""M201 S04: one-paper semantic extraction pilot tests."""

from __future__ import annotations

from research_graph.application.paper_extraction import (
    PaperChunk,
    PaperExtractionRequest,
    PaperExtractionUseCase,
)
from research_graph.domain.schema import ExtractionPatch


class _FakeLLM:
    def __init__(self) -> None:
        self.calls: list[str] = []

    def extract(self, prompt: str, kind: str, *, context=None) -> dict:
        self.calls.append(kind)
        if kind == "entities":
            return {
                "entities": [
                    {
                        "entity_type": "method",
                        "canonical_name": "Sparse Attention",
                        "confidence": 0.92,
                        "evidence_hint": "c1",
                    },
                    {
                        "entity_type": "model",
                        "canonical_name": "Transformer",
                        "confidence": 0.9,
                        "evidence_hint": "c1",
                    },
                ]
            }
        if kind == "relations":
            return {
                "relations": [
                    {
                        "relation_type": "uses",
                        "from_name": "Transformer",
                        "to_name": "Sparse Attention",
                        "confidence": 0.85,
                    }
                ]
            }
        return {}


def test_empty_paper_chunks() -> None:
    uc = PaperExtractionUseCase(llm_provider=_FakeLLM())
    result = uc.run(PaperExtractionRequest(source_id="p", chunks=[]))
    assert result.status == "empty"
    assert result.safety["graph_writes_authorized"] is False


def test_one_paper_produces_entities_relations_and_evidence_paths() -> None:
    fake = _FakeLLM()
    uc = PaperExtractionUseCase(llm_provider=fake)
    result = uc.run(
        PaperExtractionRequest(
            source_id="arxiv:2605.18747",
            chunks=[
                PaperChunk(
                    text=(
                        "Transformers use sparse attention to reduce quadratic cost "
                        "of self-attention on long sequences."
                    ),
                    semantic_chunk_id="chunk-1",
                    page_index_node_id="node-1",
                    node_path=("Introduction", "Method"),
                ),
                PaperChunk(
                    text="Experiments show sparse attention improves efficiency.",
                    semantic_chunk_id="chunk-2",
                    page_index_node_id="node-2",
                ),
            ],
        )
    )
    assert result.status == "done"
    assert result.entity_count >= 1
    assert "entities" in fake.calls
    assert "evidence_linker" in result.stage_output_keys
    assert result.evidence_linked_count >= 1
    assert result.safety["fact_promotion_authorized"] is False
    assert result.safety["production_import_authorized"] is False
    if result.extraction_patch is not None:
        assert isinstance(result.extraction_patch, ExtractionPatch)
        assert any(e.evidence_path is not None for e in result.extraction_patch.entities)


def test_paper_extraction_fail_closed_on_llm_empty() -> None:
    class EmptyLLM:
        def extract(self, prompt, kind, *, context=None):
            return {}

    uc = PaperExtractionUseCase(llm_provider=EmptyLLM())
    result = uc.run(
        PaperExtractionRequest(
            source_id="arxiv:empty",
            chunks=[
                PaperChunk(
                    text="Redacted methods discussion with models and attention.",
                    semantic_chunk_id="c0",
                )
            ],
        )
    )
    assert result.status == "done"
    assert result.entity_count == 0
