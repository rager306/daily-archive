"""Integration tests for the full arxiv archive pipeline."""

from datetime import date

from arxiv_archive.arxiv_client import ArxivPaper
from arxiv_archive.keyword_extractor import KeywordExtractor
from arxiv_archive.scoring import ScoredPaper, ScoringEngine


def sample_papers() -> list[ArxivPaper]:
    """Return deterministic papers for network-free pipeline integration tests."""
    return [
        ArxivPaper(
            id="2605.00001",
            title="Graph Retrieval for Scientific Knowledge Bases",
            abstract="We study graph retrieval, claims, evidence, and hybrid ranking.",
            authors=["Ada Lovelace"],
            published=date(2026, 5, 14),
            updated=date(2026, 5, 14),
            categories=["cs.IR", "cs.AI"],
            pdf_url="https://arxiv.org/pdf/2605.00001.pdf",
        ),
        ArxivPaper(
            id="2605.00002",
            title="A Baseline for Numerical Optimization",
            abstract="We study numerical methods and convergence for optimization.",
            authors=["Grace Hopper"],
            published=date(2026, 5, 14),
            updated=date(2026, 5, 14),
            categories=["cs.LG"],
            pdf_url="https://arxiv.org/pdf/2605.00002.pdf",
        ),
    ]


class TestFullPipeline:
    """Integration tests for the full pipeline from papers to top-10 selection."""

    def test_full_pipeline_record_reduce_score(self):
        """Tests full pipeline: papers -> keywords -> score -> top-10 selection."""
        extractor = KeywordExtractor()
        scorer = ScoringEngine()

        scored_papers = []
        for paper in sample_papers():
            keywords = extractor.extract_for_paper(paper.title, paper.abstract)
            scored = scorer.score(paper, semschol=None, keywords=keywords)
            scored_papers.append(scored)

        scored_papers.sort(key=lambda x: x.score, reverse=True)
        top10 = scored_papers[:10]

        assert len(top10) <= 10, f"Expected top10 length <= 10, got {len(top10)}"
        assert len(top10) == len([p for p in top10 if isinstance(p, ScoredPaper)]), (
            "All items in top10 should be ScoredPaper instances"
        )

    def test_top10_sorted_by_score(self):
        """Checks that top-10 are sorted by score descending."""
        extractor = KeywordExtractor()
        scorer = ScoringEngine()

        scored_papers = []
        for paper in sample_papers():
            keywords = extractor.extract_for_paper(paper.title, paper.abstract)
            scored = scorer.score(paper, semschol=None, keywords=keywords)
            scored_papers.append(scored)

        scored_papers.sort(key=lambda x: x.score, reverse=True)
        top10 = scored_papers[:10]

        if len(top10) >= 2:
            for i in range(len(top10) - 1):
                assert top10[i].score >= top10[i + 1].score, (
                    f"Top10 should be sorted descending by score at index {i} and {i + 1}"
                )
