# Formerly: src/arxiv_archive/scoring.py

"""Scoring engine for ranking arxiv papers.

Recency contract (M199 S03)
---------------------------
``ScoringEngine.score(..., run_date=)`` is the same-day-run contract:

* ``run_date`` is the analysis day against which ``paper.published`` is compared.
* Daily CLI always passes the pipeline ``run_date`` so retrospective replays
  score recency relative to that day (not wall-clock ``date.today()``).
* If ``run_date`` is omitted, ``date.today()`` is used for backward-compatible
  unit tests and ad-hoc scripts — production paths must pass it explicitly.

Semantic Scholar integration (M199 S03)
---------------------------------------
``SEMANTIC_SCHOLAR_INTEGRATION = "disabled_not_wired_in_cli"``.

The daily CLI currently scores with ``semschol=None`` and does not call
``SemanticScholarClient``. Default weight for ``citations`` is therefore
**0.0** so the dead integration cannot silently dominate ranking. The client
module remains available for future wiring (retry/rate-limit) under a separate
slice; do not re-enable the weight without a live CLI fetch path.
"""

from dataclasses import dataclass
from datetime import date

from research_graph.infrastructure.corpus.sources.arxiv_client import ArxivPaper
from research_graph.infrastructure.corpus.sources.semantic_scholar import SemanticScholarPaper

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

# M199 S03: citations path not wired in daily CLI (always semschol=None).
SEMANTIC_SCHOLAR_INTEGRATION = "disabled_not_wired_in_cli"

DEFAULT_WEIGHTS: dict[str, float] = {
    "citations": 0.0,  # disabled until SemanticScholar is wired in CLI
    "recency": 0.30,
    "novelty": 0.30,
    "preference": 0.25,
    "graph_bridge": 0.15,  # still placeholder 0.0 value; weight kept for schema stability
}


@dataclass
class ScoredPaper:
    """A scored paper with breakdown of individual component scores."""

    paper: ArxivPaper
    semschol: SemanticScholarPaper | None
    keywords: list[str]
    score: float
    breakdown: dict[str, float]
    embedding: list[float] | None = None


@dataclass
class ScoringEngine:
    """Engine for scoring arxiv papers based on multiple factors.

    See module docstring for recency same-day-run contract and Semantic Scholar
    integration status.
    """

    weights: dict[str, float] | None = None

    def __post_init__(self):
        if self.weights is None:
            self.weights = dict(DEFAULT_WEIGHTS)

    def score(
        self,
        paper: ArxivPaper,
        semschol: SemanticScholarPaper | None,
        keywords: list[str],
        *,
        run_date: date | None = None,
    ) -> ScoredPaper:
        """Score a paper; pass ``run_date`` for the recency same-day contract."""
        citations = self._citations_score(semschol)
        recency = self._recency_score(paper.published, as_of=run_date)
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

        total = sum(breakdown[k] * (self.weights or {})[k] for k in (self.weights or {}))

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

    def _recency_score(self, published: date, *, as_of: date | None = None) -> float:
        """Score based on publication date relative to ``as_of`` (default: today).

        Buckets: same day=10, 1 day=8, <=3 days=5, <=7 days=2, older=0.5.
        """
        reference = as_of if as_of is not None else date.today()
        delta = (reference - published).days

        if delta <= 0:
            # same day or future-dated relative to run_date → full credit
            return 10.0
        if delta == 1:
            return 8.0
        if delta <= 3:
            return 5.0
        if delta <= 7:
            return 2.0
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
