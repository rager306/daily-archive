from datetime import UTC, date, datetime
from typing import Any, cast

import ladybug
import pytest
from hypothesis import given, settings
from hypothesis import strategies as st

from research_graph.cli import DailyAnalysis
from research_graph.infrastructure.corpus.sources.arxiv_client import ArxivPaper
from research_graph.infrastructure.evaluation.scoring import ScoredPaper
from research_graph.infrastructure.graph.ladybug_client import init_db, upsert_daily_analysis
from tests.helpers.modular_fixtures import FIXTURE_PAPER_ID

SAFE_TEXT = st.text(
    alphabet=st.characters(
        blacklist_categories=("Cs", "Cc"),
        blacklist_characters=("\x00",),
    ),
    min_size=1,
    max_size=24,
).filter(lambda value: value.strip() != "")


def make_conn() -> ladybug.Connection:
    db = ladybug.Database(":memory:")
    conn = ladybug.Connection(db)
    conn.execute(
        "CREATE NODE TABLE Paper(id STRING, title STRING, published DATE, emb FLOAT[512], score DOUBLE, PRIMARY KEY (id))"
    )
    conn.execute("CREATE NODE TABLE Author(name STRING, PRIMARY KEY (name))")
    conn.execute("CREATE NODE TABLE Keyword(word STRING, PRIMARY KEY (word))")
    conn.execute("CREATE NODE TABLE Category(name STRING, PRIMARY KEY (name))")
    conn.execute("CREATE REL TABLE AUTHORED_BY(FROM Paper TO Author)")
    conn.execute("CREATE REL TABLE TAGGED_WITH(FROM Paper TO Keyword)")
    conn.execute("CREATE REL TABLE BELONGS_TO(FROM Paper TO Category)")
    return conn


def make_analysis(
    *,
    paper_id: str = f"arxiv:{FIXTURE_PAPER_ID}",
    title: str = "Test Title",
    authors: list[str] | None = None,
    categories: list[str] | None = None,
    keywords: list[str] | None = None,
    embedding: list[float] | None = None,
) -> DailyAnalysis:
    paper = ArxivPaper(
        id=paper_id,
        title=title,
        abstract="Graph-vector archive test abstract",
        authors=authors or ["Alice"],
        published=date(2026, 1, 1),
        updated=date(2026, 1, 1),
        categories=categories or ["cs.AI"],
        pdf_url="https://arxiv.org/pdf/test.pdf",
    )
    scored = ScoredPaper(
        paper=paper,
        semschol=None,
        keywords=keywords or ["graph"],
        score=5.5,
        breakdown={},
        embedding=embedding or [0.5] * 512,
    )
    return DailyAnalysis(
        run_date=date(2026, 1, 1),
        status="done",
        papers_fetched=1,
        papers=[scored],
        top_papers=[scored],
        analysis_timestamp=datetime.now(UTC),
    )


def collect_first_column(conn: ladybug.Connection, query: str) -> list[Any]:
    result = cast(Any, conn.execute(query))
    values = []
    while result.has_next():
        values.append(result.get_next()[0])
    return values


def scalar(conn: ladybug.Connection, query: str) -> Any:
    result = cast(Any, conn.execute(query))
    assert result.has_next()
    return result.get_next()[0]


@given(
    author=SAFE_TEXT,
    category=SAFE_TEXT,
    keyword=SAFE_TEXT,
    title=SAFE_TEXT,
)
@settings(max_examples=8, deadline=None)
def test_upsert_daily_analysis_handles_parameterized_text(
    author: str,
    category: str,
    keyword: str,
    title: str,
) -> None:
    """Parameterized Cypher must preserve punctuation/unicode without string escaping bugs."""
    conn = make_conn()
    analysis = make_analysis(
        title=title,
        authors=[author, author],
        categories=[category, category],
        keywords=[keyword, keyword],
    )

    upsert_daily_analysis(conn, analysis)

    assert collect_first_column(conn, "MATCH (a:Author) RETURN a.name") == [author]
    assert collect_first_column(conn, "MATCH (c:Category) RETURN c.name") == [category]
    assert collect_first_column(conn, "MATCH (k:Keyword) RETURN k.word") == [keyword]
    assert collect_first_column(conn, "MATCH (p:Paper) RETURN p.title") == [title]


def test_upsert_daily_analysis_is_idempotent_for_duplicate_reruns() -> None:
    """Cron reruns for the same archive day must not duplicate nodes or relationships."""
    conn = make_conn()
    analysis = make_analysis(
        authors=["Alice", "Bob O'Brian"],
        categories=["cs.AI", "cs.KG"],
        keywords=["graph", "vector"],
    )

    upsert_daily_analysis(conn, analysis)
    upsert_daily_analysis(conn, analysis)

    assert scalar(conn, "MATCH (p:Paper) RETURN count(p)") == 1
    assert scalar(conn, "MATCH (a:Author) RETURN count(a)") == 2
    assert scalar(conn, "MATCH (c:Category) RETURN count(c)") == 2
    assert scalar(conn, "MATCH (k:Keyword) RETURN count(k)") == 2
    assert scalar(conn, "MATCH (:Paper)-[r:AUTHORED_BY]->(:Author) RETURN count(r)") == 2
    assert scalar(conn, "MATCH (:Paper)-[r:BELONGS_TO]->(:Category) RETURN count(r)") == 2
    assert scalar(conn, "MATCH (:Paper)-[r:TAGGED_WITH]->(:Keyword) RETURN count(r)") == 2


def test_upsert_daily_analysis_empty_status_is_noop() -> None:
    """Empty arXiv days should not open write transactions."""
    analysis = DailyAnalysis(
        run_date=date(2026, 1, 1),
        status="empty",
        papers_fetched=0,
        papers=[],
        top_papers=[],
        analysis_timestamp=datetime.now(UTC),
    )

    class FailingConn:
        def execute(self, query: str, params: dict[str, Any] | None = None) -> None:
            raise AssertionError(f"empty analysis should not execute query: {query}")

    upsert_daily_analysis(cast(ladybug.Connection, FailingConn()), analysis)


def test_upsert_daily_analysis_rolls_back_on_write_failure() -> None:
    """A mid-transaction write failure must issue ROLLBACK and re-raise."""
    analysis = make_analysis(keywords=["will-fail"])

    class FailingConn:
        def __init__(self) -> None:
            self.queries: list[str] = []

        def execute(self, query: str, params: dict[str, Any] | None = None) -> None:
            self.queries.append(query)
            if "MERGE (k:Keyword" in query:
                raise RuntimeError("simulated keyword failure")

    conn = FailingConn()

    with pytest.raises(RuntimeError, match="simulated keyword failure"):
        upsert_daily_analysis(cast(ladybug.Connection, conn), analysis)

    assert conn.queries[0] == "BEGIN TRANSACTION"
    assert conn.queries[-1] == "ROLLBACK"
    assert "COMMIT" not in conn.queries


def test_init_db_creates_schema_and_is_idempotent(tmp_path) -> None:
    """Database initialization should create schema and tolerate reruns."""
    db_path = tmp_path / "archive_graph"

    conn = init_db(db_path)
    second_conn = init_db(db_path)

    for active_conn in (conn, second_conn):
        result = cast(Any, active_conn.execute("MATCH (p:Paper) RETURN count(p)"))
        assert result.has_next()
        assert result.get_next()[0] == 0


def test_init_db_continues_when_algo_extension_fails(monkeypatch, tmp_path) -> None:
    """Algo extension loading is optional; schema creation should still proceed."""
    import research_graph.infrastructure.graph.ladybug_client as module

    class FakeDatabase:
        def __init__(self, path: str) -> None:
            self.path = path

    class FakeConnection:
        def __init__(self, db: FakeDatabase) -> None:
            self.db = db
            self.queries: list[str] = []

        def execute(self, query: str):
            self.queries.append(query)
            if query == "INSTALL algo;":
                raise RuntimeError("extension unavailable")
            return None

    created: list[FakeConnection] = []

    def connection_factory(db: FakeDatabase) -> FakeConnection:
        conn = FakeConnection(db)
        created.append(conn)
        return conn

    monkeypatch.setattr(module.ladybug, "Database", FakeDatabase)
    monkeypatch.setattr(module.ladybug, "Connection", connection_factory)

    conn = init_db(tmp_path / "fake_graph")

    assert conn is created[0]
    assert "INSTALL algo;" in conn.queries
    assert any("CREATE NODE TABLE Paper" in query for query in conn.queries)
    assert any("CREATE REL TABLE TAGGED_WITH" in query for query in conn.queries)


def test_init_db_raises_unexpected_schema_errors(monkeypatch, tmp_path) -> None:
    """Only already-exists schema errors are swallowed."""
    import research_graph.infrastructure.graph.ladybug_client as module

    class FakeDatabase:
        def __init__(self, path: str) -> None:
            self.path = path

    class FakeConnection:
        def __init__(self, db: FakeDatabase) -> None:
            self.db = db

        def execute(self, query: str):
            if query.startswith("CREATE NODE TABLE Paper"):
                raise RuntimeError("permission denied")
            return None

    monkeypatch.setattr(module.ladybug, "Database", FakeDatabase)
    monkeypatch.setattr(module.ladybug, "Connection", FakeConnection)

    with pytest.raises(RuntimeError, match="permission denied"):
        init_db(tmp_path / "bad_graph")
