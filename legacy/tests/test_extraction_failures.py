"""M201 S07: extraction failure taxonomy tests."""

from __future__ import annotations

from research_graph.application.extraction_failures import classify_extraction_failure
from research_graph.application.paper_extraction import (
    PaperChunk,
    PaperExtractionRequest,
    PaperExtractionUseCase,
)


def test_classify_empty_candidates() -> None:
    rec = classify_extraction_failure(
        status="done",
        entity_count=0,
        relation_count=0,
        client_diagnostics={"provider": "minimax", "diagnostic_codes": ()},
    )
    assert rec is not None
    assert rec.code == "EMPTY_CANDIDATES"


def test_classify_malformed_output() -> None:
    rec = classify_extraction_failure(
        status="done",
        entity_count=0,
        client_diagnostics={
            "provider": "minimax",
            "diagnostic_codes": ("missing_tool_use",),
        },
    )
    assert rec is not None
    assert rec.code == "MALFORMED_OUTPUT"


def test_classify_transport() -> None:
    rec = classify_extraction_failure(
        status="failed",
        diagnostic="paper_extraction_failed:TimeoutError",
        client_diagnostics={"diagnostic_codes": ("transport:TimeoutError",)},
    )
    assert rec is not None
    assert rec.code == "TRANSPORT_FAILURE"


def test_classify_missing_evidence() -> None:
    rec = classify_extraction_failure(
        status="done",
        entity_count=2,
        relation_count=0,
        evidence_linked_count=0,
    )
    assert rec is not None
    assert rec.code == "MISSING_EVIDENCE"


def test_classify_success_returns_none() -> None:
    rec = classify_extraction_failure(
        status="done",
        entity_count=2,
        relation_count=1,
        evidence_linked_count=3,
    )
    assert rec is None


def test_paper_extraction_attaches_empty_candidates_failure() -> None:
    class EmptyLLM:
        last_diagnostics = {
            "provider": "minimax",
            "diagnostic_codes": (),
            "valid": False,
        }

        def extract(self, prompt, kind, *, context=None):
            return {}

    uc = PaperExtractionUseCase(llm_provider=EmptyLLM())
    result = uc.run(
        PaperExtractionRequest(
            source_id="arxiv:fail.empty",
            chunks=[
                PaperChunk(
                    text="Redacted methods with models and attention mechanisms.",
                    semantic_chunk_id="c0",
                    page_index_node_id="n0",
                )
            ],
        )
    )
    assert result.status == "done"
    assert result.entity_count == 0
    assert result.failure is not None
    assert result.failure.code == "EMPTY_CANDIDATES"
    assert result.failure.to_sanitized_dict()["credential_value_logged"] is False
