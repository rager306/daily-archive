# M002: Graph-Vector Knowledge Layer with HelixDB

**Vision:** Transform the daily flat JSON outputs into a queryable semantic knowledge graph using HelixDB. Enable `Hermes-agent` to perform multi-hop reasoning (e.g., finding papers connected to specific authors/keywords, ranked by semantic similarity to a user's interest profile).

## Scope
- Integrate `helix-py` into the daily pipeline.
- Define a formal graph schema (`Hnode`: Paper, Author, Keyword, Category; `Hedge`: authored_by, tagged_with, belongs_to).
- Generate embeddings (`Hvector`) for paper abstracts using a fast local embedding model.
- Store the daily analyzed papers into a local HelixDB instance.
- Provide a query interface for Hermes to fetch personalized paper recommendations.

## Non-goals
- Distributed/Cloud HelixDB deployment (stick to local instances for M002).
- Real-time learning/updating of the user profile (just static querying based on a provided profile for now).

## Success Criteria
- Daily pipeline successfully upserts papers, authors, and keywords as nodes in HelixDB.
- Abstracts are vectorized and stored alongside paper nodes.
- A query script successfully retrieves top N papers using a combined graph (shared keywords) and vector (semantic similarity) search.
