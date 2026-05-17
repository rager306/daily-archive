---
name: ladybugdb
description: Use when building, reviewing, or debugging LadybugDB graph-vector integrations, including Cypher schema, vector and full-text indexes, Python bindings, write concurrency, graph algorithms, and scientific KG retrieval workflows.
---

<objective>
Use LadybugDB safely as an embedded graph-vector database for local scientific knowledge graph workloads. Prefer executable probes and the local LadybugDB source over assumptions from outdated docs.
</objective>

<source_of_truth>
- Project baseline: `daily-archive` uses `ladybug>=0.16.1` as the working graph database baseline.
- Vendor source: `/root/vendor-vault/ladybug`.
- GitNexus repo for internals: `ladybug`.
- Use `gitnexus_query(..., repo="ladybug")` for implementation questions before guessing API behavior.
- HelixDB is only a comparison candidate unless it passes reproducible local probes.
</source_of_truth>

<core_model>
- LadybugDB is embedded and in-process; no daemon is required for normal local use.
- Python entrypoint:
  ```python
  import ladybug
  db = ladybug.Database("./my_graph_db")  # or ":memory:" for tests
  conn = ladybug.Connection(db)
  ```
- Keep the `Database` object alive for the application lifetime.
- Create separate `Connection` objects for concurrent readers or scoped work.
- C API comments state connections are thread-safe and multiple connections can share a database instance.
</core_model>

<schema_rules>
- Use Cypher DDL.
- Every node table should declare a primary key.
- Model papers/authors/keywords/categories as node tables and edges as relationship tables.

```cypher
CREATE NODE TABLE Paper(id STRING, title STRING, published DATE, emb FLOAT[512], PRIMARY KEY (id));
CREATE NODE TABLE Author(name STRING, PRIMARY KEY (name));
CREATE NODE TABLE Keyword(word STRING, PRIMARY KEY (word));
CREATE REL TABLE AUTHORED_BY(FROM Paper TO Author);
CREATE REL TABLE TAGGED_WITH(FROM Paper TO Keyword);
```
</schema_rules>

<vector_retrieval>
Use two tiers:

1. Brute-force vector similarity, no extension required:
```cypher
MATCH (p:Paper)
RETURN p.id, array_cosine_similarity(CAST([0.1, 0.2], 'FLOAT[2]'), p.emb) AS sim
ORDER BY sim DESC
LIMIT 10;
```

2. Indexed nearest-neighbor search, extension required:
```cypher
LOAD VECTOR;
CALL CREATE_VECTOR_INDEX('Paper', 'paper_emb_idx', 'emb', metric := 'l2');
CALL QUERY_VECTOR_INDEX('Paper', 'paper_emb_idx', CAST([0.1, 0.2], 'FLOAT[2]'), 10)
RETURN node.id, distance
ORDER BY distance;
```

Verified contract for `ladybug==0.16.1`:
- No extensions are loaded by default.
- `LOAD VECTOR` enables `CREATE_VECTOR_INDEX`, `QUERY_VECTOR_INDEX`, and `DROP_VECTOR_INDEX`.
- `QUERY_VECTOR_INDEX` returns `node` and `distance`.
- Do **not** use `nn`, `_node`, or `_distance`; those aliases are not in scope.
- Always add `ORDER BY distance` when tests or callers require deterministic nearest-first results.
</vector_retrieval>

<full_text_search>
Full-text search is extension-gated.

```cypher
LOAD FTS;
CALL CREATE_FTS_INDEX('Paper', 'paper_fts_idx', ['title', 'abstract']);
CALL QUERY_FTS_INDEX('Paper', 'paper_fts_idx', 'graph retrieval')
RETURN node.id, score
ORDER BY score DESC;
CALL DROP_FTS_INDEX('Paper', 'paper_fts_idx');
```

Verified contract for `ladybug==0.16.1`:
- `LOAD FTS` enables `CREATE_FTS_INDEX`, `QUERY_FTS_INDEX`, and `DROP_FTS_INDEX`.
- `CREATE_FTS_INDEX(table, index, [columns])` expects the indexed properties as a list.
- `QUERY_FTS_INDEX` returns `node` and `score`.
- The score behaves as a BM25-style relevance value in smoke probes.
</full_text_search>

<graph_algorithms>
Use native algorithms instead of exporting large graphs to Python.

```cypher
LOAD ALGO;
CALL project_graph('paper_kw_graph', ['Paper', 'Keyword'], ['TAGGED_WITH']);
CALL page_rank('paper_kw_graph') RETURN node.id, rank ORDER BY rank DESC;
CALL drop_projected_graph('paper_kw_graph');
```

Verified projected graph lifecycle:
```cypher
CALL show_projected_graphs() RETURN *;
CALL project_graph('Graph', ['Person'], ['KNOWS']);
CALL projected_graph_info('Graph') RETURN *;
CALL drop_projected_graph('Graph');
```

Important:
- Correct PageRank function is `page_rank`, not `pagerank`.
- Projected graph cleanup is `CALL drop_projected_graph('name')`, not `DROP GRAPH IF EXISTS`.
- A simple Paper -> Keyword bipartite projection may produce equal paper ranks in small fixtures; validate projection design before relying on centrality as a recommendation signal.
</graph_algorithms>

<write_concurrency>
Treat writes as single-writer by default.

Verified behavior:
- Concurrent writes are rejected, not queued, when `enableMultiWrites` is false.
- Error text includes: `Only one write transaction at a time`.
- Reads can continue while a write transaction is open.
- Reads do not see uncommitted writes.

Required integration rule:
- Batch writes inside explicit transactions for CLI/batch ingestion.
- Add an application-level write gate, queue, or retry/backoff before introducing concurrent ingestion, daemon, or server workflows.
- Do not assume multiple Python connections make concurrent writes safe.

Batch write pattern:
```python
conn.execute("BEGIN TRANSACTION;")
try:
    # many CREATE/MERGE/COPY statements
    conn.execute("COMMIT;")
except Exception:
    conn.execute("ROLLBACK;")
    raise
```
</write_concurrency>

<ingestion_guidance>
- Avoid thousands of autocommit single-row writes for larger corpora.
- Prefer explicit transactions for current batch ingestion.
- For larger daily/corpus-scale loads, evaluate `COPY` from CSV/Parquet/Arrow-style staging data.
- Ensure reruns are idempotent before adding scheduled or Hermes-driven ingestion.
</ingestion_guidance>

<testing_guidance>
Use executable probes for every claimed capability.

Minimum local checks:
```bash
uv run pytest tests/test_ladybug.py -q
uv run pytest tests/test_analytics.py tests/test_scientific_kg_e2e.py -q
uv run ruff check src/ tests/
uv run pyrefly check src/
```

Known project tests encode these contracts:
- `tests/test_ladybug.py` covers schema, upsert, vector extension, FTS extension, and single-writer/read visibility behavior.
- `tests/test_analytics.py` covers native `page_rank` and projected graph cleanup.

After running tests, clean tracked `__pycache__` noise before committing:
```bash
git checkout -- src/arxiv_archive/__pycache__ tests/__pycache__
```
</testing_guidance>

<project_decision>
Current posture:
- Continue with LadybugDB as the working baseline.
- Use brute-force vector similarity until indexed retrieval is needed, then load and verify `VECTOR` explicitly.
- Use FTS only behind explicit `LOAD FTS` and tests.
- Add write serialization/retry before any concurrent write workflow.
- Keep HelixDB as an unproven candidate until it proves local install, schema, writes, vector search, graph traversal, full-text search, and CI feasibility.
</project_decision>

<success_criteria>
A LadybugDB change is ready when:
- Required extensions are explicitly loaded or deliberately avoided.
- Queries use verified aliases: `node,distance` for vector index and `node,score` for FTS.
- Writes are batched or serialized according to the concurrency model.
- PageRank uses `page_rank` and projected graph cleanup uses `drop_projected_graph`.
- Relevant tests and lint/type checks pass with fresh output.
</success_criteria>
