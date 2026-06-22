# Formerly: src/arxiv_archive/semantic_scholar.py

"""Semantic Scholar API client for enriching arxiv papers with citation data."""

import asyncio
from dataclasses import dataclass
from typing import Any

import httpx


@dataclass
class SemanticScholarPaper:
    """Represents a paper enriched with Semantic Scholar metadata."""

    arxiv_id: str
    title: str
    citation_count: int
    year: int | None
    venue: str | None


class SemanticScholarClient:
    """Async client for fetching paper metadata from Semantic Scholar API."""

    BASE_URL = "https://api.semanticscholar.org/graph/v1/paper"
    FIELDS = "title,citationCount,year,venue"

    def __init__(self, timeout: float = 30.0):
        """Initialize the client with an optional timeout.

        Args:
            timeout: Request timeout in seconds (default: 30.0).
        """
        self.timeout = timeout

    async def fetch_paper(self, arxiv_id: str) -> SemanticScholarPaper | None:
        """Fetch metadata for a single arxiv paper.

        Args:
            arxiv_id: The arxiv paper ID (e.g., "2310.00001").

        Returns:
            SemanticScholarPaper if successful, None if the paper is not found
            or an error occurs.
        """
        url = f"{self.BASE_URL}/ARXIV:{arxiv_id}"
        params = {"fields": self.FIELDS}

        try:
            async with httpx.AsyncClient(timeout=self.timeout) as client:
                response = await client.get(url, params=params)
                response.raise_for_status()
                data: dict[str, Any] = await response.json()

                return SemanticScholarPaper(
                    arxiv_id=arxiv_id,
                    title=data.get("title", ""),
                    citation_count=data.get("citationCount", 0),
                    year=data.get("year"),
                    venue=data.get("venue"),
                )
        except (httpx.HTTPError, httpx.TimeoutException):
            return None

    async def fetch_batch(self, arxiv_ids: list[str]) -> dict[str, SemanticScholarPaper | None]:
        """Fetch metadata for multiple arxiv papers.

        Args:
            arxiv_ids: List of arxiv paper IDs.

        Returns:
            Dictionary mapping arxiv_id to SemanticScholarPaper if successful,
            None if the paper is not found or an error occurs.
        """
        results: dict[str, SemanticScholarPaper | None] = {}

        async def fetch_one(client: httpx.AsyncClient, arxiv_id: str) -> None:
            url = f"{self.BASE_URL}/ARXIV:{arxiv_id}"
            params = {"fields": self.FIELDS}
            try:
                response = await client.get(url, params=params)
                response.raise_for_status()
                data: dict[str, Any] = await response.json()
                results[arxiv_id] = SemanticScholarPaper(
                    arxiv_id=arxiv_id,
                    title=data.get("title", ""),
                    citation_count=data.get("citationCount", 0),
                    year=data.get("year"),
                    venue=data.get("venue"),
                )
            except (httpx.HTTPError, httpx.TimeoutException):
                results[arxiv_id] = None

        async with httpx.AsyncClient(timeout=self.timeout) as client:
            tasks = [fetch_one(client, aid) for aid in arxiv_ids]
            await asyncio.gather(*tasks)

        return results
