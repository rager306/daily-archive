from datetime import date
from email.utils import formatdate

import httpx
import pytest

from research_graph.infrastructure.corpus.sources.arxiv_client import (
    ARXIV_BACKOFF_SECONDS,
    ARXIV_MAX_RETRY_ATTEMPTS,
    ArxivClient,
    ArxivFetchError,
    ArxivPaper,
    parse_retry_after,
)


def test_arxiv_client_module_exists():
    from research_graph.infrastructure.corpus.sources import arxiv_client

    assert arxiv_client is not None


def test_arxiv_paper_dataclass():
    paper = ArxivPaper(
        id="2501.12345",
        title="Test Paper",
        abstract="This is a test abstract.",
        authors=["Author One", "Author Two"],
        published=date(2026, 5, 14),
        updated=date(2026, 5, 14),
        categories=["cs.AI", "cs.LG"],
        pdf_url="https://arxiv.org/pdf/2501.12345.pdf",
    )
    assert paper.id == "2501.12345"
    assert paper.title == "Test Paper"
    assert paper.abstract == "This is a test abstract."
    assert paper.authors == ["Author One", "Author Two"]
    assert paper.published == date(2026, 5, 14)
    assert paper.updated == date(2026, 5, 14)
    assert paper.categories == ["cs.AI", "cs.LG"]
    assert paper.pdf_url == "https://arxiv.org/pdf/2501.12345.pdf"


def test_typed_error_and_retry_policy_exports():
    assert issubclass(ArxivFetchError, RuntimeError)
    assert ARXIV_BACKOFF_SECONDS == (1.0, 5.0, 15.0, 60.0, 300.0)
    assert ARXIV_MAX_RETRY_ATTEMPTS == 3
    assert parse_retry_after("5") == 5.0
    assert parse_retry_after(None) is None
    http_date = formatdate(timeval=None, localtime=False, usegmt=True)
    # HTTP-date parse returns a non-negative float (may be near zero for "now").
    parsed = parse_retry_after(http_date)
    assert parsed is not None
    assert parsed >= 0.0


def test_fetch_papers_returns_list_without_live_network(monkeypatch):
    """The public fetch method should return a list and de-duplicate IDs per run."""
    paper = ArxivPaper(
        id="2501.12345",
        title="Test Paper",
        abstract="This is a test abstract.",
        authors=["Author One"],
        published=date(2026, 5, 14),
        updated=date(2026, 5, 14),
        categories=["cs.AI"],
        pdf_url="https://arxiv.org/pdf/2501.12345.pdf",
    )

    def fake_fetch_category(self, category, start_date, end_date=None):
        assert category == "cs.AI"
        assert start_date == date(2026, 5, 14)
        assert end_date is None
        yield paper
        yield paper

    monkeypatch.setattr(ArxivClient, "_fetch_category", fake_fetch_category)

    client = ArxivClient()
    papers = client.fetch_papers(start_date=date(2026, 5, 14), categories=["cs.AI"])

    assert isinstance(papers, list)
    assert papers == [paper]


def _atom_feed(paper_id: str = "2501.12345") -> str:
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <entry>
    <id>http://arxiv.org/abs/{paper_id}</id>
    <title>Mock Paper</title>
    <summary>Mock abstract.</summary>
    <published>2026-05-14T00:00:00Z</published>
    <updated>2026-05-14T00:00:00Z</updated>
    <author><name>Author One</name></author>
    <link href="https://arxiv.org/pdf/{paper_id}.pdf" type="application/pdf" rel="related"/>
    <category term="cs.AI" scheme="http://arxiv.org/schemas/atom"/>
  </entry>
</feed>
"""


def test_fetch_category_retries_429_then_succeeds():
    sleeps: list[float] = []
    calls = {"n": 0}

    def handler(request: httpx.Request) -> httpx.Response:
        calls["n"] += 1
        if calls["n"] == 1:
            return httpx.Response(429, headers={"Retry-After": "2"}, request=request)
        return httpx.Response(200, text=_atom_feed(), request=request)

    transport = httpx.MockTransport(handler)
    client = ArxivClient(sleep=sleeps.append, transport=transport)
    papers = list(client._fetch_category("cs.AI", date(2026, 5, 14)))

    assert len(papers) == 1
    assert papers[0].id == "2501.12345"
    assert calls["n"] == 2
    assert sleeps == [2.0]
    assert client.last_metrics["cs.AI"].rate_limit_429s == 1
    assert client.last_metrics["cs.AI"].requests_made == 2


def test_fetch_category_exhausts_on_persistent_5xx():
    sleeps: list[float] = []

    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(503, request=request)

    transport = httpx.MockTransport(handler)
    client = ArxivClient(sleep=sleeps.append, transport=transport)

    with pytest.raises(ArxivFetchError) as ei:
        list(client._fetch_category("cs.AI", date(2026, 5, 14)))

    err = ei.value
    assert err.code == "ARXIV_5XX"
    assert err.outcome == "exhausted"
    assert err.service == "arxiv_api"
    assert err.retry_count == ARXIV_MAX_RETRY_ATTEMPTS
    assert "payload" not in err.diagnostic.lower()
    assert "<" not in err.diagnostic  # no raw body
    assert len(sleeps) == ARXIV_MAX_RETRY_ATTEMPTS
    assert client.last_metrics["cs.AI"].failures == 1


def test_fetch_category_fails_fast_on_non_transient_4xx():
    sleeps: list[float] = []

    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(404, request=request)

    transport = httpx.MockTransport(handler)
    client = ArxivClient(sleep=sleeps.append, transport=transport)

    with pytest.raises(ArxivFetchError) as ei:
        list(client._fetch_category("cs.AI", date(2026, 5, 14)))

    err = ei.value
    assert err.code == "ARXIV_4XX"
    assert err.outcome == "exhausted"
    assert err.retry_count == 0
    assert sleeps == []


def test_fetch_category_retries_timeout_then_exhausts():
    sleeps: list[float] = []

    def handler(request: httpx.Request) -> httpx.Response:
        raise httpx.TimeoutException("timeout", request=request)

    transport = httpx.MockTransport(handler)
    client = ArxivClient(sleep=sleeps.append, transport=transport)

    with pytest.raises(ArxivFetchError) as ei:
        list(client._fetch_category("cs.LG", date(2026, 5, 14)))

    err = ei.value
    assert err.code == "ARXIV_TIMEOUT"
    assert err.outcome == "exhausted"
    assert err.retry_count == ARXIV_MAX_RETRY_ATTEMPTS
    assert err.category == "cs.LG"
    assert len(sleeps) == ARXIV_MAX_RETRY_ATTEMPTS
    assert sleeps[0] == ARXIV_BACKOFF_SECONDS[0]


def test_arxiv_fetch_error_diagnostic_is_redacted():
    err = ArxivFetchError(
        code="ARXIV_429",
        message="HTTP 429",
        retry_count=3,
        outcome="exhausted",
        category="cs.AI",
    )
    assert "arxiv_api" in err.diagnostic
    assert "ARXIV_429" in err.diagnostic
    assert "exhausted" in err.diagnostic
    assert "3 retries" in err.diagnostic
    assert "cs.AI" in err.diagnostic


@pytest.mark.asyncio
async def test_run_analysis_async_surfaces_typed_diagnostic(monkeypatch, tmp_path):
    from research_graph.cli import run_analysis_async, write_state_json

    def boom(*_a, **_k):
        raise ArxivFetchError(
            code="ARXIV_5XX",
            message="HTTP 503",
            retry_count=3,
            outcome="exhausted",
            category="cs.AI",
        )

    monkeypatch.setattr(ArxivClient, "fetch_papers", boom)
    monkeypatch.setattr(
        "research_graph.cli.write_state_json",
        lambda *a, **k: write_state_json(*a, **k),
    )
    # Point queue dir at tmp if QUEUE_DIR is patchable
    monkeypatch.setattr("research_graph.cli.QUEUE_DIR", tmp_path, raising=False)

    with pytest.raises(ArxivFetchError) as ei:
        await run_analysis_async(date(2026, 5, 14))

    assert "ARXIV_5XX" in ei.value.diagnostic
    # state file should exist with typed diagnostic
    state_files = list(tmp_path.glob("*.json"))
    if state_files:
        import json

        payload = json.loads(state_files[0].read_text())
        assert payload["status"] == "failed"
        assert "ARXIV_5XX" in payload.get("error", "")
