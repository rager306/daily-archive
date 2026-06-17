# Formerly: src/arxiv_archive/arxiv_client.py

from collections.abc import Iterator
from dataclasses import dataclass
from datetime import date

import feedparser
import httpx

ARXIV_API_URL = "https://export.arxiv.org/api/query"


@dataclass
class ArxivPaper:
    id: str
    title: str
    abstract: str
    authors: list[str]
    published: date
    updated: date
    categories: list[str]
    pdf_url: str


class ArxivClient:
    def fetch_papers(
        self,
        start_date: date,
        end_date: date | None = None,
        categories: list[str] | None = None,
    ) -> list[ArxivPaper]:
        if categories is None:
            categories = []

        seen_ids = set()
        papers = []
        for category in categories:
            for paper in self._fetch_category(category, start_date, end_date):
                if paper.id not in seen_ids:
                    seen_ids.add(paper.id)
                    papers.append(paper)
        return papers

    def _fetch_category(
        self, category: str, start_date: date, end_date: date | None = None
    ) -> Iterator[ArxivPaper]:
        end = end_date if end_date else start_date
        query_parts = [
            f"cat:{category}",
            f"submittedDate:[{start_date.strftime('%Y%m%d')} TO {end.strftime('%Y%m%d')}]",
        ]
        query = " AND ".join(query_parts)

        client = httpx.Client(timeout=60.0)
        try:
            response = client.get(ARXIV_API_URL, params={"search_query": query, "start": 0, "max_results": 100})
            response.raise_for_status()
            feed = feedparser.parse(response.text)
            for entry in feed.entries:
                yield self._parse_entry(entry)
        finally:
            client.close()

    def _parse_entry(self, entry) -> ArxivPaper:
        paper_id = entry.id.split("/")[-1] if "/" in entry.id else entry.id
        title = entry.title
        abstract = entry.summary
        authors = [author.name for author in entry.authors]
        published = date.fromisoformat(entry.published[:10])
        updated = date.fromisoformat(entry.updated[:10]) if hasattr(entry, "updated") else published
        categories = [tag.term for tag in entry.tags] if hasattr(entry, "tags") else []
        pdf_url = ""
        for link in entry.links:
            if link.type == "application/pdf":
                pdf_url = link.href
                break
        return ArxivPaper(
            id=paper_id,
            title=title,
            abstract=abstract,
            authors=authors,
            published=published,
            updated=updated,
            categories=categories,
            pdf_url=pdf_url,
        )
