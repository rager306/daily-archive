"""Tests for Semantic Scholar enricher."""

import asyncio
from unittest.mock import AsyncMock, patch, MagicMock

import pytest
import httpx

from arxiv_archive.semantic_scholar import SemanticScholarPaper, SemanticScholarClient


def test_semantic_scholar_paper_dataclass():
    """Test creating a SemanticScholarPaper instance with all fields."""
    paper = SemanticScholarPaper(
        arxiv_id="2310.00001",
        title="Test Paper Title",
        citation_count=42,
        year=2023,
        venue="Test Conference",
    )

    assert paper.arxiv_id == "2310.00001"
    assert paper.title == "Test Paper Title"
    assert paper.citation_count == 42
    assert paper.year == 2023
    assert paper.venue == "Test Conference"


def test_semantic_scholar_paper_optional_fields():
    """Test creating a SemanticScholarPaper with optional fields as None."""
    paper = SemanticScholarPaper(
        arxiv_id="2310.00002",
        title="Another Paper",
        citation_count=0,
        year=None,
        venue=None,
    )

    assert paper.arxiv_id == "2310.00002"
    assert paper.title == "Another Paper"
    assert paper.citation_count == 0
    assert paper.year is None
    assert paper.venue is None


@pytest.mark.asyncio
async def test_fetch_paper_basic():
    """Test fetching a known arxiv paper and checking citation_count >= 0."""
    mock_response_data = {
        "title": "AsaPy: A Python Library for Aerospace Simulation Analysis",
        "citationCount": 8,
        "year": 2023,
        "venue": "arXiv",
    }

    # Create a mock response that mimics httpx.Response
    mock_response = MagicMock(spec=httpx.Response)
    mock_response.json = AsyncMock(return_value=mock_response_data)
    mock_response.raise_for_status = MagicMock()

    # Create a mock client context manager
    mock_client = MagicMock()
    mock_client.get = AsyncMock(return_value=mock_response)

    mock_client_cm = MagicMock()
    mock_client_cm.__aenter__ = AsyncMock(return_value=mock_client)
    mock_client_cm.__aexit__ = AsyncMock(return_value=None)

    with patch("httpx.AsyncClient", return_value=mock_client_cm):
        client = SemanticScholarClient()
        paper = await client.fetch_paper("2310.00001")

        assert paper is not None
        assert isinstance(paper, SemanticScholarPaper)
        assert paper.arxiv_id == "2310.00001"
        assert paper.title == "AsaPy: A Python Library for Aerospace Simulation Analysis"
        assert paper.citation_count == 8
        assert paper.year == 2023
        assert paper.venue == "arXiv"
        assert paper.citation_count >= 0


@pytest.mark.asyncio
async def test_fetch_batch():
    """Test fetching multiple papers at once."""
    mock_responses = {
        "2310.00001": {
            "title": "Paper One",
            "citationCount": 5,
            "year": 2023,
            "venue": "Journal A",
        },
        "2310.00002": {
            "title": "Paper Two",
            "citationCount": 10,
            "year": 2024,
            "venue": "Conference B",
        },
    }

    def create_mock_response(arxiv_id):
        data = mock_responses.get(arxiv_id, {})
        mock_resp = MagicMock(spec=httpx.Response)
        mock_resp.json = AsyncMock(return_value=data)
        mock_resp.raise_for_status = MagicMock()
        return mock_resp

    async def mock_get(url, params):
        arxiv_id = url.split("ARXIV:")[1]
        return create_mock_response(arxiv_id)

    mock_client = MagicMock()
    mock_client.get = mock_get

    mock_client_cm = MagicMock()
    mock_client_cm.__aenter__ = AsyncMock(return_value=mock_client)
    mock_client_cm.__aexit__ = AsyncMock(return_value=None)

    with patch("httpx.AsyncClient", return_value=mock_client_cm):
        client = SemanticScholarClient()
        results = await client.fetch_batch(["2310.00001", "2310.00002"])

        assert len(results) == 2
        assert results["2310.00001"] is not None
        assert results["2310.00001"].citation_count == 5
        assert results["2310.00002"] is not None
        assert results["2310.00002"].citation_count == 10


@pytest.mark.asyncio
async def test_fetch_paper_error_handling():
    """Test that errors are handled gracefully and None is returned."""
    mock_response = MagicMock(spec=httpx.Response)
    mock_response.json = AsyncMock(return_value={})
    # raise_for_status is synchronous and raises HTTPStatusError
    mock_response.raise_for_status = MagicMock(
        side_effect=httpx.HTTPStatusError(
            "Not found", request=MagicMock(), response=MagicMock(status_code=404)
        )
    )

    mock_client = MagicMock()
    mock_client.get = AsyncMock(return_value=mock_response)

    mock_client_cm = MagicMock()
    mock_client_cm.__aenter__ = AsyncMock(return_value=mock_client)
    mock_client_cm.__aexit__ = AsyncMock(return_value=None)

    with patch("httpx.AsyncClient", return_value=mock_client_cm):
        client = SemanticScholarClient()
        paper = await client.fetch_paper("nonexistent")

        assert paper is None
