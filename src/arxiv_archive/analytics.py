import logging
from typing import Any, cast

import ladybug

logger = logging.getLogger(__name__)

def compute_graph_metrics(conn: ladybug.Connection) -> None:
    """Compute structural graph metrics using LadybugDB's algo extension.

    This function projects a subgraph of Papers connected by shared Keywords,
    computes the PageRank of each Paper to identify structurally important
    literature, and persists the PageRank score back to the Paper node.
    """
    logger.info("Computing graph metrics via algo extension...")

    # Check if we already have the pagerank property, if not, add it
    try:
        conn.execute("ALTER TABLE Paper ADD pagerank DOUBLE DEFAULT 0.0")
    except RuntimeError as e:
        # Expected if column already exists
        if "already exists" not in str(e).lower() and "exists" not in str(e).lower():
            logger.warning(f"Alter table warning: {e}")

    # Ladybug/Kuzu projected graph execution for PageRank. The algorithm runs on
    # the Paper -> Keyword bipartite graph and returns both Paper and Keyword
    # nodes; only Paper nodes have `id`, so only those ranks are persisted.
    try:
        conn.execute("CALL project_graph('paper_kw_graph', ['Paper', 'Keyword'], ['TAGGED_WITH'])")

        res = cast(
            Any,
            conn.execute(
                "CALL page_rank('paper_kw_graph') "
                "RETURN node.id, rank "
                "ORDER BY rank DESC"
            ),
        )

        while res.has_next():
            paper_id, rank = res.get_next()
            if paper_id is None:
                continue
            conn.execute(
                "MATCH (p:Paper {id: $id}) SET p.pagerank = $rank",
                {"id": paper_id, "rank": rank},
            )

        conn.execute("CALL drop_projected_graph('paper_kw_graph')")

    except Exception as e:
        logger.error(f"Failed to run algo extension PageRank: {e}")
        # Graceful degradation: fallback to Degree Centrality directly in Cypher.
        conn.execute("""
            MATCH (p:Paper)-[:TAGGED_WITH]->(k:Keyword)
            WITH p, count(k) as degree
            SET p.pagerank = degree * 0.1
        """)


def recommend_papers(conn: ladybug.Connection, profile_embedding: list[float], top_k: int = 10) -> list[dict]:
    """Retrieve top-N personalized paper recommendations for Hermes.

    Performs a hybrid search combining:
    1. Semantic vector similarity via array_cosine_similarity
    2. Structural graph centrality via the previously computed 'pagerank' property
    """
    if not profile_embedding or len(profile_embedding) != 512:
        raise ValueError("Profile embedding must be a list of 512 floats")

    emb_str = "[" + ",".join(map(str, profile_embedding)) + "]"

    # Hybrid Formula:
    # We combine vector similarity [0, 1] and PageRank (normalized or raw)
    # Let's weight vector similarity heavily, and use graph rank as a tie-breaker/booster.
    # Hybrid Score = (cosine_sim * 0.8) + (MIN(pagerank, 5.0)/5.0 * 0.2)

    query = f"""
    MATCH (p:Paper)
    WHERE p.emb IS NOT NULL
    WITH p, array_cosine_similarity(p.emb, {emb_str}) AS vec_sim
    WITH p, vec_sim, (vec_sim * 0.8) + ( (CASE WHEN p.pagerank > 5.0 THEN 5.0 ELSE p.pagerank END)/5.0 * 0.2 ) AS hybrid_score
    RETURN p.id, p.title, p.published, p.score, vec_sim, p.pagerank, hybrid_score
    ORDER BY hybrid_score DESC
    LIMIT {top_k}
    """

    res = cast(Any, conn.execute(query))
    recommendations = []
    while res.has_next():
        row = res.get_next()
        recommendations.append({
            "id": row[0],
            "title": row[1],
            "published": str(row[2]),
            "base_score": row[3],
            "vector_similarity": row[4],
            "graph_centrality": row[5],
            "hybrid_score": row[6]
        })

    return recommendations
