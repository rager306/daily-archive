from datetime import date

import httpx
import pytest

from arxiv_archive.arxiv_client import ArxivClient, ArxivPaper


def test_arxiv_client_module_exists():
    from arxiv_archive import arxiv_client
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


def fetch_papers_or_skip_on_rate_limit(client: ArxivClient, **kwargs):
    try:
        return client.fetch_papers(**kwargs)
    except httpx.HTTPStatusError as exc:
        if exc.response.status_code == 429:
            pytest.skip("arXiv API rate limit returned HTTP 429")
        raise


def test_fetch_papers_returns_list():
    client = ArxivClient()
    papers = fetch_papers_or_skip_on_rate_limit(
        client,
        start_date=date(2026, 5, 14),
        categories=["cs.AI"],
    )
    assert isinstance(papers, list)
