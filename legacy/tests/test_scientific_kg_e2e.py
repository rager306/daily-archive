"""End-to-end verification for the LadybugDB scientific KG foundation."""

from datetime import UTC, date, datetime
from typing import Any, cast

import ladybug

from research_graph.cli import DailyAnalysis
from research_graph.infrastructure.corpus.sources.arxiv_client import ArxivPaper
from research_graph.infrastructure.evaluation.analytics import (
    compute_graph_metrics,
    recommend_papers,
)
from research_graph.infrastructure.evaluation.scoring import ScoredPaper
from research_graph.infrastructure.graph.ladybug_client import upsert_daily_analysis


def make_kg_conn() -> ladybug.Connection:
    """Create the current M002 baseline LadybugDB schema in memory."""
    db = ladybug.Database(":memory:")
    conn = ladybug.Connection(db)
    conn.execute(
        "CREATE NODE TABLE Paper("
        "id STRING, title STRING, published DATE, emb FLOAT[512], "
        "score DOUBLE, PRIMARY KEY (id))"
    )
    conn.execute("CREATE NODE TABLE Author(name STRING, PRIMARY KEY (name))")
    conn.execute("CREATE NODE TABLE Keyword(word STRING, PRIMARY KEY (word))")
    conn.execute("CREATE NODE TABLE Category(name STRING, PRIMARY KEY (name))")
    conn.execute("CREATE REL TABLE AUTHORED_BY(FROM Paper TO Author)")
    conn.execute("CREATE REL TABLE TAGGED_WITH(FROM Paper TO Keyword)")
    conn.execute("CREATE REL TABLE BELONGS_TO(FROM Paper TO Category)")
    return conn


def scored_paper(
    *,
    paper_id: str,
    title: str,
    authors: list[str],
    categories: list[str],
    keywords: list[str],
    embedding: list[float],
    score: float,
) -> ScoredPaper:
    """Build a representative scored paper with a 512-dimensional embedding."""
    paper = ArxivPaper(
        id=paper_id,
        title=title,
        abstract=f"{title} abstract about graph retrieval and scientific evidence.",
        authors=authors,
        published=date(2026, 5, 17),
        updated=date(2026, 5, 17),
        categories=categories,
        pdf_url=f"https://arxiv.org/pdf/{paper_id}.pdf",
    )
    return ScoredPaper(
        paper=paper,
        semschol=None,
        keywords=keywords,
        score=score,
        breakdown={"preference": score},
        embedding=embedding,
    )


def test_daily_analysis_flows_to_ladybug_metrics_and_recommendations() -> None:
    """DailyAnalysis -> LadybugDB upsert -> metrics -> recommendations works end to end."""
    conn = make_kg_conn()
    matching_embedding = [0.0] * 512
    matching_embedding[0] = 1.0
    background_embedding = [0.0] * 512
    background_embedding[1] = 1.0

    target = scored_paper(
        paper_id="arxiv:2605.00001",
        title="Scientific Graph Retrieval with Evidence Paths",
        authors=["Ada Lovelace", "Grace Hopper"],
        categories=["cs.IR", "cs.AI"],
        keywords=["graph", "evidence", "retrieval"],
        embedding=matching_embedding,
        score=8.5,
    )
    background = scored_paper(
        paper_id="arxiv:2605.00002",
        title="Unrelated Numerical Optimization",
        authors=["Katherine Johnson"],
        categories=["cs.LG"],
        keywords=["optimization"],
        embedding=background_embedding,
        score=4.0,
    )
    analysis = DailyAnalysis(
        run_date=date(2026, 5, 17),
        status="done",
        papers_fetched=2,
        papers=[target, background],
        top_papers=[target, background],
        analysis_timestamp=datetime.now(UTC),
    )

    upsert_daily_analysis(conn, analysis)
    compute_graph_metrics(conn)
    recommendations = recommend_papers(conn, matching_embedding, top_k=2)

    assert [row["id"] for row in recommendations] == [
        "arxiv:2605.00001",
        "arxiv:2605.00002",
    ]
    first = recommendations[0]
    assert first["title"] == "Scientific Graph Retrieval with Evidence Paths"
    assert first["vector_similarity"] > 0.99
    assert first["hybrid_score"] >= first["vector_similarity"] * 0.8
    assert set(first) == {
        "id",
        "title",
        "published",
        "base_score",
        "vector_similarity",
        "graph_centrality",
        "hybrid_score",
    }

    author_count = cast(Any, conn.execute("MATCH (a:Author) RETURN count(a)"))
    assert author_count.has_next()
    assert author_count.get_next()[0] == 3

    relation_count = cast(
        Any,
        conn.execute("MATCH (:Paper)-[r:TAGGED_WITH]->(:Keyword) RETURN count(r)"),
    )
    assert relation_count.has_next()
    assert relation_count.get_next()[0] == 4
