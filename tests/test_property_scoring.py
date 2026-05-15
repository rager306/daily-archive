"""Property-based tests for scoring engine using Hypothesis."""

from datetime import date, timedelta

from hypothesis import Verbosity, given, settings
from hypothesis import strategies as st

from arxiv_archive.arxiv_client import ArxivPaper
from arxiv_archive.scoring import ScoringEngine
from arxiv_archive.semantic_scholar import SemanticScholarPaper


def make_arxiv_paper(
    arxiv_id: str = "arxiv:2310.00001",
    categories: list[str] | None = None,
    days_ago: int = 0,
) -> ArxivPaper:
    """Factory for ArxivPaper with controlled date."""
    if categories is None:
        categories = ["cs.AI"]
    pub_date = date.today() - timedelta(days=days_ago)
    return ArxivPaper(
        id=arxiv_id,
        title="Test Paper Title",
        abstract="Test abstract " * 20,
        authors=["Author One", "Author Two"],
        published=pub_date,
        updated=pub_date,
        categories=categories,
        pdf_url=f"https://arxiv.org/pdf/{arxiv_id.split(':')[1]}.pdf",
    )


def make_semschol(
    arxiv_id: str = "2310.00001",
    citation_count: int = 0,
) -> SemanticScholarPaper:
    return SemanticScholarPaper(
        arxiv_id=arxiv_id,
        title="Test Paper",
        citation_count=citation_count,
        year=2024,
        venue="NeurIPS",
    )


@settings(verbosity=Verbosity.verbose, max_examples=200)
@given(
    citation_count=st.integers(min_value=0, max_value=10000),
    days_ago=st.integers(min_value=0, max_value=365),
    keyword_count=st.integers(min_value=0, max_value=50),
    num_categories=st.integers(min_value=1, max_value=7),
)
def test_score_always_in_valid_range(
    citation_count: int,
    days_ago: int,
    keyword_count: int,
    num_categories: int,
) -> None:
    """Score must always be in [0, 10] range regardless of inputs."""
    engine = ScoringEngine()
    categories = ["cs.AI", "cs.LG", "cs.CL", "cs.CV", "cs.IR", "cs.KG", "cs.SI"][:num_categories]
    paper = make_arxiv_paper(categories=categories, days_ago=days_ago)
    semschol = make_semschol(citation_count=citation_count)
    keywords = [f"keyword{i}" for i in range(keyword_count)]

    scored = engine.score(paper, semschol, keywords)

    assert 0.0 <= scored.score <= 10.0, f"Score {scored.score} out of range [0, 10]"


@settings(verbosity=Verbosity.verbose, max_examples=200)
@given(
    citation_count=st.integers(min_value=0, max_value=10000),
    days_ago=st.integers(min_value=0, max_value=365),
    keyword_count=st.integers(min_value=0, max_value=50),
)
def test_score_breakdown_components_in_range(
    citation_count: int,
    days_ago: int,
    keyword_count: int,
) -> None:
    """Each scoring component must be in [0, 10] range."""
    engine = ScoringEngine()
    paper = make_arxiv_paper(days_ago=days_ago)
    semschol = make_semschol(citation_count=citation_count)
    keywords = [f"keyword{i}" for i in range(keyword_count)]

    scored = engine.score(paper, semschol, keywords)

    for component, value in scored.breakdown.items():
        assert 0.0 <= value <= 10.0, f"Component {component}={value} out of range"


@settings(max_examples=500)
@given(
    n_papers=st.integers(min_value=1, max_value=100),
)
def test_top10_sorting_always_correct(n_papers: int) -> None:
    """Top-10 papers must be sorted by score descending."""
    engine = ScoringEngine()

    papers = [
        make_arxiv_paper(arxiv_id=f"arxiv:2310.{i:05d}", days_ago=i % 30)
        for i in range(n_papers)
    ]

    scored = [
        engine.score(p, make_semschol(citation_count=i * 10), [f"kw{i}"])
        for i, p in enumerate(papers)
    ]

    # Sort by score descending and take top 10
    scored.sort(key=lambda x: x.score, reverse=True)
    top10 = scored[:10]

    # Verify descending order
    for i in range(len(top10) - 1):
        assert top10[i].score >= top10[i + 1].score, (
            f"Top-10 not sorted: score[{i}]={top10[i].score} < score[{i+1}]={top10[i+1].score}"
        )

    # Verify all top-10 scores >= all non-top-10 scores (if n > 10)
    if n_papers > 10:
        min_top10_score = min(s.score for s in top10)
        max_rest_score = max(s.score for s in scored[10:])
        assert min_top10_score >= max_rest_score


@settings(max_examples=100)
@given(keyword_count=st.integers(min_value=0, max_value=100))
def test_empty_keywords_handled(keyword_count: int) -> None:
    """Empty or minimal keyword lists must not crash scoring."""
    engine = ScoringEngine()
    paper = make_arxiv_paper()
    keywords = [] if keyword_count == 0 else [f"kw{i}" for i in range(keyword_count)]

    scored = engine.score(paper, None, keywords)

    assert scored.score >= 0.0
    assert len(scored.breakdown) == 5


@settings(max_examples=100)
@given(
    category=st.sampled_from([
        "cs.AI", "cs.LG", "cs.CL", "cs.CV", "cs.IR", "cs.KG", "cs.SI",
        "cs.NE", "cs.DB", "cs.DS", "cs.DC", "cs.MA", "cs.ST", "cs.RO",
        "unknown.cat", "", "cs.XY",
    ])
)
def test_any_category_does_not_crash(category: str) -> None:
    """Any category string must not crash preference scoring."""
    engine = ScoringEngine()
    paper = make_arxiv_paper(categories=[category])

    scored = engine.score(paper, None, ["test"])

    assert scored.score >= 0.0


@given(
    days_ago_list=st.lists(
        st.integers(min_value=0, max_value=365),
        min_size=1,
        max_size=50,
        unique=True,
    )
)
@settings(max_examples=100)
def test_recency_score_monotonic(days_ago_list: list[int]) -> None:
    """More recent papers must have >= recency score than older papers."""
    engine = ScoringEngine()

    # Create papers for each days_ago value
    scores = {}
    for days_ago in days_ago_list:
        paper = make_arxiv_paper(days_ago=days_ago)
        scored = engine.score(paper, None, [])
        scores[days_ago] = scored.breakdown["recency"]

    # Verify monotonic: smaller days_ago => >= score
    sorted_days = sorted(days_ago_list)
    for i in range(len(sorted_days) - 1):
        smaller = sorted_days[i]
        larger = sorted_days[i + 1]
        assert scores[smaller] >= scores[larger], (
            f"Recency not monotonic: days_ago={smaller} score={scores[smaller]} "
            f"< days_ago={larger} score={scores[larger]}"
        )
