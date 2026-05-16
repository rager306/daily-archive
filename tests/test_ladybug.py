import pytest
import ladybug
from datetime import date

@pytest.fixture
def memory_db():
    db = ladybug.Database(":memory:")
    conn = ladybug.Connection(db)
    
    # Init schema
    conn.execute("CREATE NODE TABLE Paper(id STRING, title STRING, published DATE, emb FLOAT[1024], score DOUBLE, PRIMARY KEY (id))")
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
    emb = [0.1] * 1024
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
