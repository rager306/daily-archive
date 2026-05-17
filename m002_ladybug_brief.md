# M002: Graph-Vector Knowledge Layer with LadybugDB

**Vision:** Transform the daily flat JSON outputs into a queryable semantic knowledge graph using LadybugDB (an embedded, columnar C++ graph database). Enable `Hermes-agent` to perform multi-hop reasoning and advanced graph analytics (Louvain, PageRank) alongside vector semantic search.

## Scope
- Integrate `ladybug` into the daily pipeline.
- Define a formal graph schema using Cypher (`Paper`, `Author`, `Keyword`, `Category`).
- Generate embeddings (`FLOAT[384]`) for paper abstracts using a fast local embedding model.
- Implement parallel fetch/compute in Python, followed by single-writer bulk ingestion (via `COPY` or explicit transactions) into LadybugDB.
- Provide a query interface for Hermes to fetch personalized paper recommendations.

## Non-goals
- Distributed/Cloud deployments (LadybugDB is strictly embedded/local).
- Real-time learning/updating of the user profile.

## Success Criteria
- Daily pipeline uses `asyncio.gather` for fetch/extract, then bulk-inserts into LadybugDB.
- Abstracts are vectorized and stored as `FLOAT[N]` arrays.
- Graph analytics (like Louvain communities or PageRank) run natively via the `algo` extension.
- A query script retrieves top N papers using a combined graph (shared keywords/communities) and vector (semantic similarity) search.
