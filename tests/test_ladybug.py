from datetime import UTC, date

import ladybug
import pytest


@pytest.fixture
def memory_db():
    db = ladybug.Database(":memory:")
    conn = ladybug.Connection(db)

    # Init schema
    conn.execute("CREATE NODE TABLE Paper(id STRING, title STRING, published DATE, emb FLOAT[512], score DOUBLE, PRIMARY KEY (id))")
    conn.execute("CREATE NODE TABLE Author(name STRING, PRIMARY KEY (name))")
    conn.execute("CREATE NODE TABLE Keyword(word STRING, PRIMARY KEY (word))")
    conn.execute("CREATE NODE TABLE Category(name STRING, PRIMARY KEY (name))")

    conn.execute("CREATE REL TABLE AUTHORED_BY(FROM Paper TO Author)")
    conn.execute("CREATE REL TABLE TAGGED_WITH(FROM Paper TO Keyword)")
    conn.execute("CREATE REL TABLE BELONGS_TO(FROM Paper TO Category)")

    return conn

def test_ladybug_schema_and_query(memory_db):
    conn = memory_db

    # Create test data
    emb = [0.1] * 512
    # Note: Kuzu/Ladybug arrays are formatted like [0.1, 0.2, ...] in cypher
    emb_str = "[" + ",".join(map(str, emb)) + "]"

    conn.execute(f"CREATE (p:Paper {{id: 'arxiv:1234', title: 'Test Paper', published: date('2024-05-14'), emb: {emb_str}, score: 8.5}})")
    conn.execute("CREATE (a:Author {name: 'John Doe'})")
    conn.execute("CREATE (k:Keyword {word: 'llm'})")

    conn.execute("MATCH (p:Paper {id: 'arxiv:1234'}), (a:Author {name: 'John Doe'}) CREATE (p)-[:AUTHORED_BY]->(a)")
    conn.execute("MATCH (p:Paper {id: 'arxiv:1234'}), (k:Keyword {word: 'llm'}) CREATE (p)-[:TAGGED_WITH]->(k)")

    # Query test
    res = conn.execute("MATCH (p:Paper)-[:TAGGED_WITH]->(k:Keyword) RETURN p.title, k.word")
    assert res.has_next()
    record = res.get_next()
    assert record[0] == "Test Paper"
    assert record[1] == "llm"

def test_upsert_daily_analysis(memory_db, monkeypatch):
    from datetime import datetime

    from arxiv_archive.arxiv_client import ArxivPaper
    from arxiv_archive.cli import DailyAnalysis
    from arxiv_archive.ladybug_client import upsert_daily_analysis
    from arxiv_archive.scoring import ScoredPaper

    paper = ArxivPaper(
        id="arxiv:test-1",
        title="Test Title",
        abstract="Test abstract",
        authors=["Alice", "Bob O'Brian"], # tests escaping
        published=date(2026, 1, 1),
        updated=date(2026, 1, 1),
        categories=["cs.AI"],
        pdf_url="http://test",
    )

    scored = ScoredPaper(
        paper=paper,
        semschol=None,
        keywords=["ai", "test"],
        score=5.5,
        breakdown={},
        embedding=[0.5] * 512
    )

    analysis = DailyAnalysis(
        run_date=date(2026, 1, 1),
        status="done",
        analysis_timestamp=datetime.now(UTC),
        papers_fetched=1,
        papers=[scored],
        top_papers=[scored],
    )

    upsert_daily_analysis(memory_db, analysis)

    # Verify paper
    res = memory_db.execute("MATCH (p:Paper) RETURN p.id, p.title, p.emb[1], p.score")
    assert res.has_next()
    rec = res.get_next()
    assert rec[0] == "arxiv:test-1"
    assert rec[1] == "Test Title"
    assert rec[2] == 0.5
    assert rec[3] == 5.5

    # Verify authors
    res = memory_db.execute("MATCH (a:Author) RETURN a.name ORDER BY a.name")
    authors = []
    while res.has_next():
        authors.append(res.get_next()[0])
    assert authors == ["Alice", "Bob O'Brian"]

    # Verify keywords
    res = memory_db.execute("MATCH (k:Keyword) RETURN k.word ORDER BY k.word")
    keywords = []
    while res.has_next():
        keywords.append(res.get_next()[0])
    assert keywords == ["ai", "test"]

    # Verify connections
    res = memory_db.execute("MATCH (p:Paper)-[:AUTHORED_BY]->(a:Author) RETURN p.id, a.name ORDER BY a.name")
    assert res.has_next()
    assert res.get_next()[1] == "Alice"
    assert res.get_next()[1] == "Bob O'Brian"
