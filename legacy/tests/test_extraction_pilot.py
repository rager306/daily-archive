"""M201 S05: five-paper bounded pilot tests."""

from __future__ import annotations

from research_graph.application.extraction_pilot import PilotPaper, run_bounded_pilot
from research_graph.application.paper_extraction import PaperChunk


class _CountingLLM:
    def __init__(self) -> None:
        self.calls = 0

    def extract(self, prompt: str, kind: str, *, context=None) -> dict:
        self.calls += 1
        if kind == "entities":
            return {
                "entities": [
                    {
                        "entity_type": "method",
                        "canonical_name": f"Method{self.calls}",
                        "confidence": 0.9,
                        "evidence_hint": "c",
                    }
                ]
            }
        if kind == "relations":
            return {
                "relations": [
                    {
                        "relation_type": "uses",
                        "from_name": "A",
                        "to_name": "B",
                        "confidence": 0.8,
                    }
                ]
            }
        return {}


def _paper(i: int) -> PilotPaper:
    return PilotPaper(
        source_id=f"arxiv:pilot.{i}",
        chunks=(
            PaperChunk(
                text=f"Paper {i} discusses transformers and attention mechanisms in detail.",
                semantic_chunk_id=f"chunk-{i}-0",
                page_index_node_id=f"node-{i}",
            ),
        ),
    )


def test_bounded_pilot_runs_five_papers() -> None:
    llm = _CountingLLM()
    papers = [_paper(i) for i in range(1, 6)]
    report = run_bounded_pilot(papers, llm_provider=llm, max_papers=5)
    assert report.paper_count == 5
    assert report.done_count == 5
    assert report.failed_count == 0
    assert report.total_entities >= 5
    assert report.total_evidence_linked >= 1
    assert len(report.outcomes) == 5
    assert report.quality["done_rate"] == 1.0
    assert report.safety["graph_writes_authorized"] is False
    assert report.safety["fact_promotion_authorized"] is False
    assert llm.calls >= 5


def test_bounded_pilot_respects_max_papers() -> None:
    llm = _CountingLLM()
    papers = [_paper(i) for i in range(1, 8)]
    report = run_bounded_pilot(papers, llm_provider=llm, max_papers=5)
    assert report.paper_count == 5


def test_bounded_pilot_records_empty_papers() -> None:
    llm = _CountingLLM()
    papers = [
        PilotPaper(source_id="empty.1", chunks=()),
        _paper(2),
    ]
    report = run_bounded_pilot(papers, llm_provider=llm, max_papers=5)
    assert report.empty_count == 1
    assert report.done_count == 1
