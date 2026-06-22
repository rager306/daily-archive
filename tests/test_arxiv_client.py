from datetime import date

from research_graph.infrastructure.corpus.sources.arxiv_client import ArxivClient, ArxivPaper


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
