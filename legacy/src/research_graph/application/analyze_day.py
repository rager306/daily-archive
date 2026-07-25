"""Application-owned daily analysis use case (M200 S03 / D114).

``AnalyzeDayUseCase`` owns the day-level runtime composition that previously
lived in the CLI adapter. Infrastructure clients are injected through ports so
this module never imports ``research_graph.infrastructure`` (onion guard).

CLI / cron remain adapters: they construct concrete ports, catch typed I/O
errors for ``state.json``, and call :meth:`AnalyzeDayUseCase.run`.
"""

from __future__ import annotations

import asyncio
from collections.abc import Callable, Sequence
from datetime import UTC, date, datetime
from typing import Any, Protocol, runtime_checkable

from research_graph.application.analysis import DailyAnalysis

DEFAULT_SCORE_CONCURRENCY = 8


class AnalyzeDayError(RuntimeError):
    """Fail-closed day-analysis error with redacted diagnostic for adapters."""

    def __init__(self, *, stage: str, diagnostic: str) -> None:
        self.stage = stage
        self.diagnostic = diagnostic
        super().__init__(diagnostic)


@runtime_checkable
class PaperFetchPort(Protocol):
    """Fetch papers for a date window (infra: ArxivClient)."""

    def fetch_papers(
        self,
        start_date: date,
        end_date: date | None = None,
        categories: list[str] | None = None,
    ) -> list[Any]: ...


@runtime_checkable
class KeywordExtractPort(Protocol):
    """Extract keywords for one paper title/abstract."""

    def extract_for_paper(self, title: str, abstract: str) -> list[str]: ...


@runtime_checkable
class PaperScorePort(Protocol):
    """Score one paper; must accept ``run_date`` for recency contract."""

    def score(
        self,
        paper: Any,
        semschol: Any,
        keywords: list[str],
        *,
        run_date: date | None = None,
    ) -> Any: ...


@runtime_checkable
class EmbedderPort(Protocol):
    """Async embedder with optional degrade mark."""

    last_degraded: Any

    async def embed_all(self, texts: list[str]) -> list[list[float]]: ...

    async def close(self) -> None: ...


EmbedderFactory = Callable[[], EmbedderPort]


def _is_zero_embedding_batch(embeddings: Sequence[Sequence[float]], *, eps: float = 1e-12) -> bool:
    """Local zero-batch predicate (no infrastructure import)."""
    if not embeddings:
        return False
    return all(all(abs(float(x)) <= eps for x in vec) for vec in embeddings)


class AnalyzeDayUseCase:
    """Day-level no-write analysis: fetch → score → embed → rank."""

    def __init__(
        self,
        *,
        paper_fetch: PaperFetchPort,
        keyword_extractor: KeywordExtractPort,
        scorer: PaperScorePort,
        embedder_factory: EmbedderFactory,
        categories: Sequence[str],
        score_concurrency: int = DEFAULT_SCORE_CONCURRENCY,
    ) -> None:
        self.paper_fetch = paper_fetch
        self.keyword_extractor = keyword_extractor
        self.scorer = scorer
        self.embedder_factory = embedder_factory
        self.categories = categories
        self.score_concurrency = score_concurrency

    async def run(self, run_date: date) -> DailyAnalysis:
        """Execute day analysis. Raises port exceptions or :class:`AnalyzeDayError`."""
        papers = self.paper_fetch.fetch_papers(
            start_date=run_date,
            end_date=run_date,
            categories=list(self.categories),
        )

        if not papers:
            return DailyAnalysis(
                run_date=run_date,
                status="empty",
                analysis_timestamp=datetime.now(UTC),
                papers_fetched=0,
                papers=[],
                top_papers=[],
            )

        scored_papers = await self._score_papers_bounded(papers, run_date=run_date)

        embedder = self.embedder_factory()
        try:
            abstracts = [p.paper.abstract for p in scored_papers]
            embeddings = await embedder.embed_all(abstracts)
            degraded = getattr(embedder, "last_degraded", None)
            if degraded is not None or _is_zero_embedding_batch(embeddings):
                reason = getattr(degraded, "reason", None) if degraded is not None else None
                reason = reason or "all_zero_batch"
                raise AnalyzeDayError(
                    stage="embed",
                    diagnostic=(
                        f"fd_embedder:FD_DEGRADED_ZERO_VECTORS exhausted after 0 retries "
                        f"reason={reason}: refusing unmarked or degraded zero embeddings"
                    ),
                )
            for scored, emb in zip(scored_papers, embeddings, strict=True):
                scored.embedding = emb
        finally:
            await embedder.close()

        scored_papers.sort(key=lambda x: x.score, reverse=True)
        top_papers = scored_papers[:10]

        return DailyAnalysis(
            run_date=run_date,
            status="done",
            analysis_timestamp=datetime.now(UTC),
            papers_fetched=len(papers),
            papers=scored_papers,
            top_papers=top_papers,
        )

    async def _score_papers_bounded(
        self, papers: list[Any], *, run_date: date
    ) -> list[Any]:
        semaphore = asyncio.Semaphore(self.score_concurrency)

        async def _one(paper: Any) -> Any:
            async with semaphore:
                return await self._process_paper(paper, run_date=run_date)

        return list(await asyncio.gather(*(_one(p) for p in papers)))

    async def _process_paper(self, paper: Any, *, run_date: date) -> Any:
        loop = asyncio.get_running_loop()

        def _extract_and_score() -> Any:
            if not isinstance(paper.title, str) or not isinstance(paper.abstract, str):
                raise TypeError("fetched paper title and abstract must be strings")
            keywords = self.keyword_extractor.extract_for_paper(paper.title, paper.abstract)
            return self.scorer.score(paper, None, keywords, run_date=run_date)

        return await loop.run_in_executor(None, _extract_and_score)


__all__ = [
    "AnalyzeDayError",
    "AnalyzeDayUseCase",
    "DEFAULT_SCORE_CONCURRENCY",
    "EmbedderFactory",
    "EmbedderPort",
    "KeywordExtractPort",
    "PaperFetchPort",
    "PaperScorePort",
]
