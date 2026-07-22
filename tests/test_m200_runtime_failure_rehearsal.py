"""M200 S04: runtime failure rehearsal — typed failures never become false success."""

from __future__ import annotations

import json
from datetime import date
from pathlib import Path
from types import SimpleNamespace

import pytest

from research_graph.application.analyze_day import AnalyzeDayError, AnalyzeDayUseCase
from research_graph.infrastructure.corpus.sources.arxiv_client import ArxivFetchError
from research_graph.infrastructure.retrieval.embedder import FdAuthError

_RUN = date(2026, 5, 14)


class _FetchBoom:
    def fetch_papers(self, start_date, end_date=None, categories=None):
        raise ArxivFetchError(
            code="ARXIV_5XX",
            message="HTTP 503",
            retry_count=3,
            outcome="exhausted",
            category="cs.AI",
        )


class _FetchOne:
    def fetch_papers(self, start_date, end_date=None, categories=None):
        return [
            SimpleNamespace(
                id="2605.00001",
                title="Title",
                abstract="Abstract text for embedding",
            )
        ]


class _Keywords:
    def extract_for_paper(self, title, abstract):
        return ["kw"]


class _Scorer:
    def score(self, paper, semschol, keywords, *, run_date=None):
        return SimpleNamespace(paper=paper, score=1.0, embedding=None)


class _EmbedDegraded:
    last_degraded = SimpleNamespace(reason="circuit_open")

    async def embed_all(self, texts):
        return [[0.0] * 4 for _ in texts]

    async def close(self):
        return None


class _EmbedAuthBoom:
    last_degraded = None

    async def embed_all(self, texts):
        raise FdAuthError(code="FD_AUTH_MISSING", message="FD_API_KEY is missing")

    async def close(self):
        return None


@pytest.mark.asyncio
async def test_analyze_day_fetch_failure_propagates_typed_error() -> None:
    uc = AnalyzeDayUseCase(
        paper_fetch=_FetchBoom(),
        keyword_extractor=_Keywords(),
        scorer=_Scorer(),
        embedder_factory=lambda: _EmbedDegraded(),
        categories=["cs.AI"],
    )
    with pytest.raises(ArxivFetchError) as ei:
        await uc.run(_RUN)
    assert ei.value.code == "ARXIV_5XX"
    assert "HTTP 503" in ei.value.diagnostic
    # no secret-looking payload markers
    assert "sk-" not in ei.value.diagnostic
    assert "<html" not in ei.value.diagnostic.lower()


@pytest.mark.asyncio
async def test_analyze_day_embed_degrade_is_analyze_day_error_not_done() -> None:
    uc = AnalyzeDayUseCase(
        paper_fetch=_FetchOne(),
        keyword_extractor=_Keywords(),
        scorer=_Scorer(),
        embedder_factory=lambda: _EmbedDegraded(),
        categories=["cs.AI"],
    )
    with pytest.raises(AnalyzeDayError) as ei:
        result = await uc.run(_RUN)
        assert result.status != "done"  # pragma: no cover - must not reach
    assert ei.value.stage == "embed"
    assert "FD_DEGRADED_ZERO_VECTORS" in ei.value.diagnostic


@pytest.mark.asyncio
async def test_cli_maps_arxiv_fetch_error_to_state_json(tmp_path: Path, monkeypatch) -> None:
    import research_graph.cli as cli

    monkeypatch.setattr(cli, "QUEUE_DIR", tmp_path)
    monkeypatch.setattr(cli, "load_preferences", lambda: {})

    class BoomClient:
        def fetch_papers(self, start_date, end_date=None, categories=None):
            raise ArxivFetchError(
                code="ARXIV_TIMEOUT",
                message="TimeoutException",
                retry_count=3,
                outcome="exhausted",
            )

    monkeypatch.setattr(cli, "ArxivClient", BoomClient)
    monkeypatch.setattr(cli, "KeywordExtractor", lambda: _Keywords())
    monkeypatch.setattr(cli, "ScoringEngine", lambda: _Scorer())
    monkeypatch.setattr(cli, "Embedder", lambda: _EmbedDegraded())

    with pytest.raises(ArxivFetchError):
        await cli.run_analysis_async(_RUN)

    state_files = list(tmp_path.glob("*.json"))
    assert state_files, "state.json must be written on fetch failure"
    payload = json.loads(state_files[0].read_text())
    assert payload["status"] == "failed"
    assert payload["stage"] == "fetch"
    assert "ARXIV_TIMEOUT" in payload["error"]
    assert "sk-" not in payload["error"]


@pytest.mark.asyncio
async def test_cli_maps_fd_auth_error_to_state_json(tmp_path: Path, monkeypatch) -> None:
    import research_graph.cli as cli

    monkeypatch.setattr(cli, "QUEUE_DIR", tmp_path)
    monkeypatch.setattr(cli, "load_preferences", lambda: {})
    monkeypatch.setattr(cli, "ArxivClient", lambda: _FetchOne())
    monkeypatch.setattr(cli, "KeywordExtractor", lambda: _Keywords())
    monkeypatch.setattr(cli, "ScoringEngine", lambda: _Scorer())
    monkeypatch.setattr(cli, "Embedder", lambda: _EmbedAuthBoom())

    with pytest.raises(FdAuthError):
        await cli.run_analysis_async(_RUN)

    payload = json.loads(next(tmp_path.glob("*.json")).read_text())
    assert payload["status"] == "failed"
    assert payload["stage"] == "embed"
    assert "FD_AUTH_MISSING" in payload["error"]


@pytest.mark.asyncio
async def test_cli_maps_analyze_day_error_to_state_json(tmp_path: Path, monkeypatch) -> None:
    import research_graph.cli as cli

    monkeypatch.setattr(cli, "QUEUE_DIR", tmp_path)
    monkeypatch.setattr(cli, "load_preferences", lambda: {})
    monkeypatch.setattr(cli, "ArxivClient", lambda: _FetchOne())
    monkeypatch.setattr(cli, "KeywordExtractor", lambda: _Keywords())
    monkeypatch.setattr(cli, "ScoringEngine", lambda: _Scorer())
    monkeypatch.setattr(cli, "Embedder", lambda: _EmbedDegraded())

    with pytest.raises(AnalyzeDayError):
        await cli.run_analysis_async(_RUN)

    payload = json.loads(next(tmp_path.glob("*.json")).read_text())
    assert payload["status"] == "failed"
    assert payload["stage"] == "embed"
    assert "FD_DEGRADED" in payload["error"]
