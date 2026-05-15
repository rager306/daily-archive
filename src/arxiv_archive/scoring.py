"""Scoring engine for ranking arxiv papers."""

from dataclasses import dataclass
from datetime import date

from arxiv_archive.arxiv_client import ArxivPaper
from arxiv_archive.semantic_scholar import SemanticScholarPaper


TOPIC_WEIGHTS = {
    "cs.SI": 1.5,
    "cs.KG": 1.5,
    "cs.IR": 1.3,
    "cs.CL": 1.3,
    "cs.AI": 1.2,
    "cs.LG": 1.0,
    "cs.CV": 1.0,
    "cs.NE": 1.0,
    "cs.ML": 0.9,
    "cs.DB": 0.9,
    "cs.DS": 0.9,
}


@dataclass
class ScoredPaper:
    """A scored paper with breakdown of individual component scores."""

    paper: ArxivPaper
    semschol: SemanticScholarPaper | None
    keywords: list[str]
    score: float
    breakdown: dict[str, float]


@dataclass
class ScoringEngine:
    """Engine for scoring arxiv papers based on multiple factors."""

    weights: dict[str, float] = None

    def __post_init__(self):
        if self.weights is None:
            self.weights = {
                "citations": 0.25,
                "recency": 0.20,
                "novelty": 0.20,
                "preference": 0.20,
                "graph_bridge": 0.15,
            }

    def score(
        self, paper: ArxivPaper, semschol: SemanticScholarPaper | None, keywords: list[str]
    ) -> ScoredPaper:
        """Score a paper based on citations, recency, novelty, preference, and graph bridge."""
        citations = self._citations_score(semschol)
        recency = self._recency_score(paper.published)
        novelty = self._novelty_score(keywords)
        preference = self._preference_score(paper.categories)
        # graph_bridge is not computed here, placeholder at 0
        graph_bridge = 0.0

        breakdown = {
            "citations": citations,
            "recency": recency,
            "novelty": novelty,
            "preference": preference,
            "graph_bridge": graph_bridge,
        }

        total = sum(
            breakdown[k] * self.weights[k] for k in self.weights
        )

        return ScoredPaper(
            paper=paper,
            semschol=semschol,
            keywords=keywords,
            score=total,
            breakdown=breakdown,
        )

    def _citations_score(self, semschol: SemanticScholarPaper | None) -> float:
        """Normalize citations to 0-10 scale, max at 100 citations."""
        if semschol is None:
            return 0.0
        return min(semschol.citation_count / 100.0, 1.0) * 10.0

    def _recency_score(self, published: date) -> float:
        """Score based on publication date: today=10, yesterday=8, 3days=5, week=2, older=0.5."""
        today = date.today()
        delta = (today - published).days

        if delta == 0:
            return 10.0
        elif delta == 1:
            return 8.0
        elif delta <= 3:
            return 5.0
        elif delta <= 7:
            return 2.0
        else:
            return 0.5

    def _novelty_score(self, keywords: list[str]) -> float:
        """Score based on keyword count: min(len(keywords), 10) * 0.5."""
        return min(len(keywords), 10) * 0.5

    def _preference_score(self, categories: list[str]) -> float:
        """Score based on topic preference: max weight from predefined map."""
        if not categories:
            return TOPIC_WEIGHTS.get("others", 0.5)

        max_weight = 0.0
        for cat in categories:
            weight = TOPIC_WEIGHTS.get(cat, 0.5)
            if weight > max_weight:
                max_weight = weight

        return max_weight if max_weight > 0 else 0.5
