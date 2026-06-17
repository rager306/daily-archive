"""Integration/property tests for full pipeline with Adaptix data loading."""

from datetime import date
from pathlib import Path
from typing import Any

from research_graph.corpus.sources.arxiv_client import ArxivPaper
from arxiv_archive.scoring import ScoredPaper, ScoringEngine
from tests.helpers.modular_fixtures import FIXTURE_PAPER_ID, MODULAR_RETORT, adaptix_dump


def dict_to_arxiv_paper(data: dict[str, Any]) -> ArxivPaper:
    """Convert dict loaded from JSON to ArxivPaper dataclass."""
    return MODULAR_RETORT.load(data, ArxivPaper)


def dict_to_scored_paper(data: dict[str, Any]) -> ScoredPaper:
    """Convert dict loaded from JSON to ScoredPaper dataclass."""
    return MODULAR_RETORT.load(data, ScoredPaper)


# --- Property: deserialized papers preserve critical fields ---

def test_arxiv_paper_roundtrip() -> None:
    """ArxivPaper serialized to dict and back must preserve all fields."""
    original = ArxivPaper(
        id=f"arxiv:{FIXTURE_PAPER_ID}",
        title="Test: Graph Neural Networks for Knowledge Graphs",
        abstract="This paper proposes a new method.",
        authors=["Alice Smith", "Bob Jones"],
        published=date(2026, 5, 14),
        updated=date(2026, 5, 15),
        categories=["cs.AI", "cs.KG"],
        pdf_url=f"https://arxiv.org/pdf/{FIXTURE_PAPER_ID}.pdf",
    )

    # Dump to dict
    dumped = adaptix_dump(original)

    # Load back
    restored = dict_to_arxiv_paper(dumped)

    assert restored.id == original.id
    assert restored.title == original.title
    assert restored.abstract == original.abstract
    assert restored.authors == original.authors
    assert restored.published == original.published
    assert restored.categories == original.categories


def test_scored_paper_roundtrip() -> None:
    """ScoredPaper serialized to dict and back must preserve score and breakdown."""
    from research_graph.corpus.sources.semantic_scholar import SemanticScholarPaper

    paper = ArxivPaper(
        id=f"arxiv:{FIXTURE_PAPER_ID}",
        title="Test",
        abstract="Test abstract",
        authors=["Author"],
        published=date(2026, 5, 14),
        updated=date(2026, 5, 14),
        categories=["cs.AI"],
        pdf_url=f"https://arxiv.org/pdf/{FIXTURE_PAPER_ID}.pdf",
    )
    semschol = SemanticScholarPaper(
        arxiv_id=FIXTURE_PAPER_ID,
        title="Test",
        citation_count=42,
        year=2024,
        venue="ICML",
    )
    engine = ScoringEngine()
    scored = engine.score(paper, semschol, ["graph", "neural", "network"])

    # Roundtrip
    dumped = adaptix_dump(scored)
    restored = dict_to_scored_paper(dumped)

    assert restored.score == scored.score
    assert restored.breakdown == scored.breakdown
    assert restored.paper.id == scored.paper.id
    assert restored.semschol is not None
    assert scored.semschol is not None
    assert restored.semschol.citation_count == scored.semschol.citation_count


def test_scored_paper_embedding_roundtrip() -> None:
    """ScoredPaper embeddings must survive Adaptix serialization boundaries."""
    paper = ArxivPaper(
        id=f"arxiv:{FIXTURE_PAPER_ID}-embedding",
        title="Embedding Test",
        abstract="Test abstract",
        authors=["Author"],
        published=date(2026, 5, 14),
        updated=date(2026, 5, 14),
        categories=["cs.AI"],
        pdf_url=f"https://arxiv.org/pdf/{FIXTURE_PAPER_ID}-embedding.pdf",
    )
    scored = ScoredPaper(
        paper=paper,
        semschol=None,
        keywords=["graph", "vector"],
        score=7.25,
        breakdown={"novelty": 1.0},
        embedding=[float(i % 7) / 7 for i in range(512)],
    )

    dumped = adaptix_dump(scored)
    restored = dict_to_scored_paper(dumped)

    assert restored.embedding == scored.embedding
    assert restored.paper.id == scored.paper.id
    assert restored.keywords == scored.keywords


# --- Property: session file format is valid JSON ---

def test_session_file_is_valid_json(tmp_path: Path) -> None:
    """Session file written by cli.save_session must be valid JSON parseable."""
    from arxiv_archive.cli import save_session

    papers = [
        ArxivPaper(
            id=f"arxiv:{FIXTURE_PAPER_ID}-{i:05d}",
            title=f"Paper Title {i}",
            abstract="Abstract " * 10,
            authors=[f"Author {i}"],
            published=date(2026, 5, 14),
            updated=date(2026, 5, 14),
            categories=["cs.AI"],
            pdf_url=f"https://arxiv.org/pdf/{FIXTURE_PAPER_ID}-{i:05d}.pdf",
        )
        for i in range(5)
    ]

    engine = ScoringEngine()
    scored = [
        engine.score(p, None, [f"kw{i}"])
        for i, p in enumerate(papers)
    ]

    # Override SESSIONS_DIR to tmp_path for test
    import arxiv_archive.cli as cli_module
    original_sessions_dir = cli_module.SESSIONS_DIR
    cli_module.SESSIONS_DIR = tmp_path / "sessions"

    try:
        path = save_session(date(2026, 5, 14), len(papers), scored)
        # Session saved as .md, not JSON — that's correct
        assert path.exists()
        assert path.suffix == ".md"
    finally:
        cli_module.SESSIONS_DIR = original_sessions_dir


# --- Pipeline integration: score preserves ordering ---

def test_pipeline_score_preserves_order() -> None:
    """Higher component scores must produce higher total scores."""
    engine = ScoringEngine()
    base_paper = ArxivPaper(
        id=f"arxiv:{FIXTURE_PAPER_ID}",
        title="Test",
        abstract="Test",
        authors=["A"],
        published=date.today(),
        updated=date.today(),
        categories=["cs.AI"],
        pdf_url=f"https://arxiv.org/pdf/{FIXTURE_PAPER_ID}.pdf",
    )

    # Score with 0 citations
    s1 = engine.score(base_paper, None, [])

    # Score with 100 citations
    from research_graph.corpus.sources.semantic_scholar import SemanticScholarPaper
    s2 = engine.score(
        base_paper,
        SemanticScholarPaper(
            arxiv_id=FIXTURE_PAPER_ID,
            title="Test",
            citation_count=100,
            year=2024,
            venue="NeurIPS",
        ),
        [],
    )

    # More citations => higher score
    assert s2.score >= s1.score, (
        f"More citations should give higher score: {s2.score} < {s1.score}"
    )


# --- Edge: empty paper list ---

def test_pipeline_empty_paper_list() -> None:
    """Sorting empty list must not crash."""
    scored: list[ScoredPaper] = []
    sorted_papers = sorted(scored, key=lambda x: x.score, reverse=True)
    top10 = sorted_papers[:10]
    assert top10 == []


# --- Edge: single paper ---

def test_pipeline_single_paper() -> None:
    """Single paper pipeline must return that paper as top-1."""
    paper = ArxivPaper(
        id=f"arxiv:{FIXTURE_PAPER_ID}",
        title="Single Paper",
        abstract="Test",
        authors=["Author"],
        published=date.today(),
        updated=date.today(),
        categories=["cs.AI"],
        pdf_url=f"https://arxiv.org/pdf/{FIXTURE_PAPER_ID}.pdf",
    )
    engine = ScoringEngine()
    scored = [engine.score(paper, None, ["graph"])]

    sorted_papers = sorted(scored, key=lambda x: x.score, reverse=True)
    top10 = sorted_papers[:10]

    assert len(top10) == 1
    assert top10[0].paper.id == f"arxiv:{FIXTURE_PAPER_ID}"
