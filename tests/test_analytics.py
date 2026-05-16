import pytest
import ladybug
from arxiv_archive.analytics import compute_graph_metrics, recommend_papers

@pytest.fixture
def test_db():
    db = ladybug.Database(":memory:")
    conn = ladybug.Connection(db)
    conn.execute("INSTALL algo;")
    conn.execute("LOAD EXTENSION algo;")
    
    conn.execute("CREATE NODE TABLE Paper(id STRING, title STRING, published DATE, emb FLOAT[512], score DOUBLE, PRIMARY KEY (id))")
    conn.execute("CREATE NODE TABLE Keyword(word STRING, PRIMARY KEY (word))")
    conn.execute("CREATE REL TABLE TAGGED_WITH(FROM Paper TO Keyword)")
    
    # Insert paper 1: High graph connectivity, low similarity
    emb1 = [0.1] * 512
    conn.execute(f"CREATE (p:Paper {{id: '1', title: 'Graph Heavy', published: date('2024-01-01'), emb: {emb1}, score: 1.0}})")
    conn.execute("CREATE (k1:Keyword {word: 'a'})")
    conn.execute("CREATE (k2:Keyword {word: 'b'})")
    conn.execute("CREATE (k3:Keyword {word: 'c'})")
    conn.execute("MATCH (p:Paper {id: '1'}), (k:Keyword) WHERE k.word IN ['a','b','c'] CREATE (p)-[:TAGGED_WITH]->(k)")
    
    # Insert paper 2: High vector similarity, low graph connectivity
    emb2 = [0.9] * 512
    conn.execute(f"CREATE (p:Paper {{id: '2', title: 'Vector Heavy', published: date('2024-01-01'), emb: {emb2}, score: 1.0}})")
    conn.execute("MATCH (p:Paper {id: '2'}), (k:Keyword {word: 'a'}) CREATE (p)-[:TAGGED_WITH]->(k)")
    
    return conn

def test_compute_graph_metrics(test_db):
    compute_graph_metrics(test_db)
    
    # Verify pagerank column exists and is populated
    res = test_db.execute("MATCH (p:Paper) RETURN p.id, p.pagerank ORDER BY p.id")
    assert res.has_next()
    
    results = {}
    while res.has_next():
        row = res.get_next()
        results[row[0]] = row[1]
        
    assert "1" in results
    assert "2" in results
    # Paper 1 has 3 keywords, Paper 2 has 1. With the fallback degree centrality, 
    # Paper 1 should have higher rank (0.3 vs 0.1)
    assert results["1"] > results["2"]

def test_recommend_papers(test_db):
    compute_graph_metrics(test_db)
    
    # The user profile is [0.9] * 512
    # Paper 2 is an exact match for this profile, so vec_sim = 1.0
    # Paper 1 has [0.1], so vec_sim = 1.0 (wait, cosine sim of flat vectors is 1.0)
    # Let's change user profile to [0.9, 0.0, ...] to separate them.
    # Ah, array_cosine_similarity of [0.1, 0.1] and [0.9, 0.9] is 1.0 because they are collinear!
    
    # Let's create Paper 3 with an orthogonal vector
    emb3 = [0.0] * 512
    emb3[0] = 1.0
    test_db.execute(f"CREATE (p:Paper {{id: '3', title: 'Orthogonal', published: date('2024-01-01'), emb: {emb3}, score: 1.0}})")
    
    profile = [0.0] * 512
    profile[0] = 1.0
    
    recs = recommend_papers(test_db, profile)
    
    assert len(recs) == 3
    # Paper 3 should have vec_sim = 1.0
    assert recs[0]["id"] == "3"
    assert recs[0]["vector_similarity"] > 0.99
    
    # Check hybrid math
    for r in recs:
        assert "hybrid_score" in r
        assert "graph_centrality" in r
        
def test_recommend_papers_invalid_profile(test_db):
    with pytest.raises(ValueError):
        recommend_papers(test_db, [0.1] * 10) # Wrong dimension
