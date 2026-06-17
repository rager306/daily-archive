from datetime import UTC, date

import ladybug
import pytest


def _rows(result):
    rows = []
    while result.has_next():
        rows.append(result.get_next())
    return rows


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

    from research_graph.corpus.sources.arxiv_client import ArxivPaper
    from research_graph.cli import DailyAnalysis
    from research_graph.graph.ladybug_client import upsert_daily_analysis
    from research_graph.evaluation.scoring import ScoredPaper

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


def test_vector_extension_hnsw_index_contract():
    conn = ladybug.Connection(ladybug.Database(":memory:"))

    assert _rows(conn.execute("LOAD VECTOR;"))[0][0] == "Extension: vector has been loaded."
    conn.execute("CREATE NODE TABLE Embedding(id STRING, vec FLOAT[3], PRIMARY KEY (id));")
    conn.execute("CREATE (:Embedding {id: 'a', vec: CAST([1.0, 0.0, 0.0], 'FLOAT[3]')});")
    conn.execute("CREATE (:Embedding {id: 'b', vec: CAST([0.0, 1.0, 0.0], 'FLOAT[3]')});")
    conn.execute("CREATE (:Embedding {id: 'c', vec: CAST([0.9, 0.1, 0.0], 'FLOAT[3]')});")
    conn.execute("CALL CREATE_VECTOR_INDEX('Embedding', 'embedding_vec_idx', 'vec', metric := 'l2');")

    rows = _rows(
        conn.execute(
            "CALL QUERY_VECTOR_INDEX('Embedding', 'embedding_vec_idx', "
            "CAST([1.0, 0.0, 0.0], 'FLOAT[3]'), 3) "
            "RETURN node.id, distance ORDER BY distance;"
        )
    )

    assert [row[0] for row in rows] == ["a", "c", "b"]
    assert rows[0][1] == 0.0


def test_fts_extension_index_contract():
    conn = ladybug.Connection(ladybug.Database(":memory:"))

    assert _rows(conn.execute("LOAD FTS;"))[0][0] == "Extension: fts has been loaded."
    conn.execute("CREATE NODE TABLE Doc(id STRING, title STRING, body STRING, PRIMARY KEY (id));")
    conn.execute("CREATE (:Doc {id: 'd1', title: 'Graph retrieval', body: 'graph neural retrieval graph'});")
    conn.execute("CREATE (:Doc {id: 'd2', title: 'Bayes', body: 'bayesian optimization'});")
    result = _rows(conn.execute("CALL CREATE_FTS_INDEX('Doc', 'doc_fts_idx', ['title', 'body']);"))
    assert result == [["Index doc_fts_idx has been created."]]

    rows = _rows(conn.execute("CALL QUERY_FTS_INDEX('Doc', 'doc_fts_idx', 'graph') RETURN node.id, score;"))

    assert rows[0][0] == "d1"
    assert rows[0][1] > 0


def test_ladybug_allows_reads_but_rejects_second_writer_during_write_transaction():
    db = ladybug.Database(":memory:")
    writer = ladybug.Connection(db)
    reader_or_second_writer = ladybug.Connection(db)
    writer.execute("CREATE NODE TABLE Item(id INT64, PRIMARY KEY (id));")

    writer.execute("BEGIN TRANSACTION;")
    writer.execute("CREATE (:Item {id: 1});")

    with pytest.raises(RuntimeError, match="Only one write transaction at a time"):
        reader_or_second_writer.execute("CREATE (:Item {id: 2});")

    assert _rows(reader_or_second_writer.execute("MATCH (i:Item) RETURN count(i);")) == [[0]]

    writer.execute("COMMIT;")
    assert _rows(reader_or_second_writer.execute("MATCH (i:Item) RETURN count(i);")) == [[1]]
