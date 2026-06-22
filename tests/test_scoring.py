"""Tests for the scoring engine."""

from datetime import date, timedelta

from research_graph.infrastructure.corpus.sources.arxiv_client import ArxivPaper
from research_graph.infrastructure.corpus.sources.semantic_scholar import SemanticScholarPaper
from research_graph.infrastructure.evaluation.scoring import ScoredPaper, ScoringEngine


class TestScoredPaperDataclass:
    """Tests for ScoredPaper dataclass."""

    def test_scored_paper_dataclass(self):
        """Creates ScoredPaper, checks fields."""
        paper = ArxivPaper(
            id="2310.00001",
            title="Test Paper",
            abstract="Test abstract",
            authors=["Author One"],
            published=date(2024, 1, 1),
            updated=date(2024, 1, 1),
            categories=["cs.AI"],
            pdf_url="https://arxiv.org/pdf/2310.00001.pdf",
        )
        semschol = SemanticScholarPaper(
            arxiv_id="2310.00001",
            title="Test Paper",
            citation_count=42,
            year=2024,
            venue="Test Venue",
        )
        keywords = ["machine learning", "neural networks"]

        scored = ScoredPaper(
            paper=paper,
            semschol=semschol,
            keywords=keywords,
            score=7.5,
            breakdown={
                "citations": 4.2,
                "recency": 1.0,
                "novelty": 1.0,
                "preference": 1.2,
                "graph_bridge": 0.0,
            },
        )

        assert scored.paper == paper
        assert scored.semschol == semschol
        assert scored.keywords == keywords
        assert scored.score == 7.5
        assert scored.breakdown == {
            "citations": 4.2,
            "recency": 1.0,
            "novelty": 1.0,
            "preference": 1.2,
            "graph_bridge": 0.0,
        }


class TestScoringEngine:
    """Tests for ScoringEngine class."""

    def test_scoring_engine_basic(self):
        """Creates engine, scores a paper with mock ArxivPaper and SemanticScholarPaper, checks score > 0."""
        engine = ScoringEngine()

        paper = ArxivPaper(
            id="2310.00001",
            title="Test Paper",
            abstract="Test abstract",
            authors=["Author One"],
            published=date.today(),
            updated=date.today(),
            categories=["cs.AI"],
            pdf_url="https://arxiv.org/pdf/2310.00001.pdf",
        )
        semschol = SemanticScholarPaper(
            arxiv_id="2310.00001",
            title="Test Paper",
            citation_count=50,  # 50 citations = 5.0 citation score
            year=2024,
            venue="Test Venue",
        )
        keywords = ["machine learning"]

        scored = engine.score(paper, semschol, keywords)

        assert isinstance(scored, ScoredPaper)
        assert scored.paper == paper
        assert scored.semschol == semschol
        assert scored.keywords == keywords
        assert scored.score > 0
        assert "citations" in scored.breakdown
        assert "recency" in scored.breakdown
        assert "novelty" in scored.breakdown
        assert "preference" in scored.breakdown

    def test_recency_score(self):
        """Tests recency scoring with known dates."""
        engine = ScoringEngine()

        today = date.today()

        # today = 10
        today_score = engine._recency_score(today)
        assert today_score == 10.0

        # yesterday = 8
        yesterday_score = engine._recency_score(today - timedelta(days=1))
        assert yesterday_score == 8.0

        # 3 days ago = 5
        three_days_score = engine._recency_score(today - timedelta(days=3))
        assert three_days_score == 5.0

        # 7 days ago (week) = 2
        week_score = engine._recency_score(today - timedelta(days=7))
        assert week_score == 2.0

        # older = 0.5
        older_score = engine._recency_score(today - timedelta(days=30))
        assert older_score == 0.5

    def test_citations_score(self):
        """Tests citations scoring normalization."""
        engine = ScoringEngine()

        # No semschol = 0
        assert engine._citations_score(None) == 0.0

        # 0 citations = 0
        semschol = SemanticScholarPaper("1", "t", 0, 2024, None)
        assert engine._citations_score(semschol) == 0.0

        # 50 citations = 5.0 (50/100 * 10)
        semschol = SemanticScholarPaper("1", "t", 50, 2024, None)
        assert engine._citations_score(semschol) == 5.0

        # 100+ citations = 10 (capped)
        semschol = SemanticScholarPaper("1", "t", 200, 2024, None)
        assert engine._citations_score(semschol) == 10.0

    def test_novelty_score(self):
        """Tests novelty scoring based on keyword count."""
        engine = ScoringEngine()

        # 0 keywords = 0
        assert engine._novelty_score([]) == 0.0

        # 5 keywords = 2.5
        assert engine._novelty_score(["k1", "k2", "k3", "k4", "k5"]) == 2.5

        # 10 keywords = 5.0
        ten_kw = ["k1", "k2", "k3", "k4", "k5", "k6", "k7", "k8", "k9", "k10"]
        assert engine._novelty_score(ten_kw) == 5.0

        # 15 keywords = 5.0 (capped at 10)
        fifteen_kw = ten_kw + ["k11", "k12", "k13", "k14", "k15"]
        assert engine._novelty_score(fifteen_kw) == 5.0

    def test_preference_score(self):
        """Tests preference scoring based on categories."""
        engine = ScoringEngine()

        # cs.AI = 1.2
        assert engine._preference_score(["cs.AI"]) == 1.2

        # cs.SI = 1.5 (highest)
        assert engine._preference_score(["cs.SI"]) == 1.5

        # cs.KG = 1.5 (highest)
        assert engine._preference_score(["cs.KG"]) == 1.5

        # Unknown category = 0.5
        assert engine._preference_score(["unknown.category"]) == 0.5

        # Empty list = 0.5
        assert engine._preference_score([]) == 0.5

        # Multiple categories - takes max
        assert engine._preference_score(["cs.AI", "cs.SI"]) == 1.5
