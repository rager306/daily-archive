"""M200 S03: AnalyzeDayUseCase port-based day runtime (no infrastructure imports)."""

from __future__ import annotations

from datetime import date
from types import SimpleNamespace

import pytest

from research_graph.application.analysis import DailyAnalysis
from research_graph.application.analyze_day import AnalyzeDayError, AnalyzeDayUseCase


class _FakeFetch:
    def __init__(self, papers: list):
        self.papers = papers
        self.calls: list[tuple] = []

    def fetch_papers(self, start_date, end_date=None, categories=None):
        self.calls.append((start_date, end_date, tuple(categories or ())))
        return list(self.papers)


class _FakeKeywords:
    def extract_for_paper(self, title: str, abstract: str) -> list[str]:
        return ["kw"]


class _FakeScorer:
    def score(self, paper, semschol, keywords, *, run_date=None):
        assert run_date is not None
        return SimpleNamespace(paper=paper, score=float(paper.id.split(".")[-1]), embedding=None)


class _FakeEmbedder:
    last_degraded = None

    def __init__(self, vectors: list[list[float]] | None = None, *, degrade: bool = False):
        self._vectors = vectors
        self.last_degraded = SimpleNamespace(reason="circuit_open") if degrade else None

    async def embed_all(self, texts: list[str]) -> list[list[float]]:
        if self._vectors is not None:
            return self._vectors
        return [[0.1, 0.2, 0.3] for _ in texts]

    async def close(self) -> None:
        return None


def _paper(i: int):
    return SimpleNamespace(
        id=f"2605.{i:05d}",
        title=f"Title {i}",
        abstract=f"Abstract {i}",
    )


@pytest.mark.asyncio
async def test_analyze_day_empty() -> None:
    uc = AnalyzeDayUseCase(
        paper_fetch=_FakeFetch([]),
        keyword_extractor=_FakeKeywords(),
        scorer=_FakeScorer(),
        embedder_factory=lambda: _FakeEmbedder(),
        categories=["cs.AI"],
    )
    result = await uc.run(date(2026, 5, 14))
    assert isinstance(result, DailyAnalysis)
    assert result.status == "empty"
    assert result.papers_fetched == 0


@pytest.mark.asyncio
async def test_analyze_day_done_ranks_and_embeds() -> None:
    papers = [_paper(3), _paper(1), _paper(2)]
    uc = AnalyzeDayUseCase(
        paper_fetch=_FakeFetch(papers),
        keyword_extractor=_FakeKeywords(),
        scorer=_FakeScorer(),
        embedder_factory=lambda: _FakeEmbedder(),
        categories=["cs.AI", "cs.LG"],
        score_concurrency=2,
    )
    result = await uc.run(date(2026, 5, 14))
    assert result.status == "done"
    assert result.papers_fetched == 3
    assert [p.paper.id for p in result.papers] == ["2605.00003", "2605.00002", "2605.00001"]
    assert all(p.embedding == [0.1, 0.2, 0.3] for p in result.papers)
    assert len(result.top_papers) == 3


@pytest.mark.asyncio
async def test_analyze_day_refuses_zero_embeddings() -> None:
    papers = [_paper(1)]
    uc = AnalyzeDayUseCase(
        paper_fetch=_FakeFetch(papers),
        keyword_extractor=_FakeKeywords(),
        scorer=_FakeScorer(),
        embedder_factory=lambda: _FakeEmbedder(vectors=[[0.0, 0.0, 0.0]]),
        categories=["cs.AI"],
    )
    with pytest.raises(AnalyzeDayError) as ei:
        await uc.run(date(2026, 5, 14))
    assert ei.value.stage == "embed"
    assert "FD_DEGRADED_ZERO_VECTORS" in ei.value.diagnostic
