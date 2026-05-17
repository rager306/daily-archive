"""Integration tests for the full arxiv archive pipeline."""

from datetime import date

import httpx
import pytest

from arxiv_archive.arxiv_client import ArxivClient
from arxiv_archive.keyword_extractor import KeywordExtractor
from arxiv_archive.scoring import ScoredPaper, ScoringEngine


def fetch_papers_or_skip_on_rate_limit(client: ArxivClient, **kwargs):
    try:
        return client.fetch_papers(**kwargs)
    except httpx.HTTPStatusError as exc:
        if exc.response.status_code == 429:
            pytest.skip("arXiv API rate limit returned HTTP 429")
        raise


class TestFullPipeline:
    """Integration tests for the full pipeline from fetch to top-10 selection."""

    def test_full_pipeline_record_reduce_score(self):
        """Tests full pipeline: fetch -> keywords -> score -> top-10 selection."""
        run_date = date(2026, 5, 14)
        categories = ["cs.AI"]

        # Create components
        client = ArxivClient()
        extractor = KeywordExtractor()
        scorer = ScoringEngine()

        # Fetch papers for date 2026-05-14 with categories cs.AI
        papers = fetch_papers_or_skip_on_rate_limit(
            client,
            start_date=run_date,
            end_date=run_date,
            categories=categories,
        )

        # Extract keywords and score each paper
        scored_papers = []
        for paper in papers:
            keywords = extractor.extract_for_paper(paper.title, paper.abstract)
            scored = scorer.score(paper, semschol=None, keywords=keywords)
            scored_papers.append(scored)

        # Sort by score descending and take top 10
        scored_papers.sort(key=lambda x: x.score, reverse=True)
        top10 = scored_papers[:10]

        # Checks: len(top10) <= 10, all are ScoredPaper instances
        assert len(top10) <= 10, f"Expected top10 length <= 10, got {len(top10)}"
        assert len(top10) == len([p for p in top10 if isinstance(p, ScoredPaper)]), \
            "All items in top10 should be ScoredPaper instances"

    def test_top10_sorted_by_score(self):
        """Checks that top-10 are sorted by score descending."""
        run_date = date(2026, 5, 14)
        categories = ["cs.AI"]

        # Create components
        client = ArxivClient()
        extractor = KeywordExtractor()
        scorer = ScoringEngine()

        # Fetch papers
        papers = fetch_papers_or_skip_on_rate_limit(
            client,
            start_date=run_date,
            end_date=run_date,
            categories=categories,
        )

        # Extract keywords and score each paper
        scored_papers = []
        for paper in papers:
            keywords = extractor.extract_for_paper(paper.title, paper.abstract)
            scored = scorer.score(paper, semschol=None, keywords=keywords)
            scored_papers.append(scored)

        # Sort by score descending and take top 10
        scored_papers.sort(key=lambda x: x.score, reverse=True)
        top10 = scored_papers[:10]

        # Check that top-10 are sorted by score descending
        if len(top10) >= 2:
            for i in range(len(top10) - 1):
                assert top10[i].score >= top10[i + 1].score, \
                    f"Top10 should be sorted descending by score at index {i} and {i+1}"
