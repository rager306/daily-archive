# Graph Database Selection Validation Spike

## Reader and post-read action

Reader: a future project maintainer deciding whether the scientific knowledge graph should continue on LadybugDB or reopen HelixDB as a candidate.

Post-read action: run the validation checklist below, update the evidence table, and make a database decision based on observed behavior rather than vendor claims.

## Current decision posture

LadybugDB remains the working baseline. HelixDB is an interesting candidate, but the old project HelixDB branch only proved a schema sketch and wrapper shape, not a working integration.

This spike is not a rollback to HelixDB. It is a validation pass over database choice risk.

## Why this spike exists

The project needs graph-vector storage for scientific papers, authors, keywords, categories, embeddings, structural metrics, and hybrid recommendations. A wrong database choice will affect ingestion, retrieval quality, local operation, testing, and future Hermes integration.

Recent evidence showed:

- LadybugDB can run embedded in-process and supports Cypher, fixed-size vector arrays, graph relations, and hybrid recommendation queries.
- The project implementation initially called `pagerank`, but LadybugDB/Kuzu-compatible docs and local execution show the correct native algorithm call is `page_rank`.
- The old HelixDB implementation in project history did not include real writes; it returned input nodes/edges from wrapper methods and had no test coverage.
- Current HelixDB docs describe Helix Cloud as a different architecture from open-source v1: object-storage-backed, stored Rust DSL query model, single writer plus scalable readers, vector and full-text search.

## Candidates

### LadybugDB baseline

Observed local behavior:

- Installs as Python dependency `ladybug`.
- Runs in-memory and as a temporary persistent database.
- Creates node tables for papers, authors, keywords, and categories.
- Creates relations for authorship, keyword tagging, and category membership.
- Upserts `DailyAnalysis` data inside an explicit transaction.
- Stores 512-dimensional embeddings in `FLOAT[512]`.
- Runs vector similarity with `array_cosine_similarity` without loading an extension.
- Runs HNSW-style vector indexes after `LOAD VECTOR` with `CREATE_VECTOR_INDEX`, `QUERY_VECTOR_INDEX`, and `DROP_VECTOR_INDEX`.
- `QUERY_VECTOR_INDEX` returns `node` and `distance`; callers must `ORDER BY distance` if deterministic nearest-first output is required.
- Runs full-text search after `LOAD FTS` with `CREATE_FTS_INDEX(table, index, [columns])`, `QUERY_FTS_INDEX(table, index, query)`, and `DROP_FTS_INDEX`.
- `QUERY_FTS_INDEX` returns `node` and `score`, where score behaves as a BM25-style relevance value in smoke probes.
- Runs native graph PageRank through the `algo` extension with `page_rank`.
- Manages projected graphs with `project_graph`, `show_projected_graphs`, `projected_graph_info`, and `drop_projected_graph`.
- Produces hybrid recommendations combining vector similarity and graph centrality.
- Python bindings expose `Database` and `Connection`; source comments state a connection is thread-safe and multiple connections can share a database instance.
- Write concurrency is single-writer by default: concurrent writes are rejected with `Only one write transaction at a time`, not queued by the engine. Reads are allowed during an open write transaction and do not see uncommitted data.

Known limitations to validate further:

- PageRank over the current Paper -> Keyword bipartite graph gives equal paper ranks in a small two-paper smoke fixture. That may be mathematically expected for this projection, but we need a better projection for useful structural ranking.
- Current ingestion loops over records. For larger daily/corpus-scale ingestion, compare explicit transaction loops against bulk import.
- Application code should serialize or retry writes around LadybugDB, especially if future CLI/server paths can ingest concurrently.
- Vector and FTS indexes are extension-gated. Startup code or tests that depend on indexed retrieval should load required extensions explicitly and fail clearly if unavailable.

### HelixDB candidate

Observed from current docs and old project history:

- Current docs describe Helix Cloud as graph + vector + text storage over object storage with ACID transactions and stored query execution.
- Current docs state the Cloud architecture is fundamentally different from the old open-source v1 LMDB architecture.
- The old project branch used `helix`, `Client`, `Hnode`, `Hedge`, and `Schema`, but did not declare a working Python dependency in project metadata.
- The old wrapper did not perform real writes; it returned nodes/edges locally when a client existed and returned empty lists when disconnected.
- The old project branch had no HelixDB tests.

Unknowns to validate:

- Is there a supported local/community runtime suitable for this project, or is practical use now Cloud/enterprise-oriented?
- Is there a maintained Python SDK matching the current Cloud/stored-query architecture?
- Can HelixDB run fully local in CI without secrets or a daemon?
- Can it express the paper graph, 512-dim embeddings, vector search, graph traversal, and full-text search in one reproducible fixture?
- Can its query model support dynamic exploratory agent workflows, or only pre-deployed stored procedures?

## Validation matrix

| Capability | LadybugDB current evidence | HelixDB evidence required |
|---|---|---|
| Local install | `ladybug` installs and imports under `uv` | Identify supported install path and Python/Rust/HTTP client |
| In-memory CI tests | Works with `Database(':memory:')` | Prove equivalent local test mode or containerized fixture |
| Persistent local DB | Temp filesystem smoke works | Prove local persistent fixture |
| Schema creation | Cypher node/rel tables work | Prove node/edge/vector schema creation |
| DailyAnalysis ingestion | Explicit transaction upsert works | Prove real upsert, not wrapper echo |
| Idempotent rerun | Tests cover duplicate reruns | Prove duplicate-safe writes |
| Vector storage | `FLOAT[512]` works | Prove 512-dim vector storage |
| Vector retrieval | `array_cosine_similarity` works; `LOAD VECTOR` enables `CREATE_VECTOR_INDEX` and `QUERY_VECTOR_INDEX` returning `node, distance` | Prove vector nearest-neighbor or similarity query |
| Graph traversal | Cypher relations work | Prove traversal over paper-author-keyword graph |
| Graph analytics | Native `page_rank` works; projection quality still needs evaluation | Prove built-in graph algorithm or acceptable alternative |
| Full-text search | `LOAD FTS` enables `CREATE_FTS_INDEX(table, index, [columns])` and `QUERY_FTS_INDEX` returning `node, score` | Prove BM25/text index if candidate claims it |
| Write concurrency | Default behavior is single-writer; concurrent writes raise instead of queueing; reads can continue without seeing uncommitted writes | Prove write/read concurrency behavior and retry/queue requirements |
| Agent workflow fit | Dynamic Cypher queries work in-process | Determine whether stored-query model blocks exploratory agents |
| Operational fit | Embedded, no daemon | Determine daemon/cloud/container requirement |
| License/support risk | MIT, successor to Kuzu per docs | Clarify Cloud/open-source split and license constraints |

## Required probes

### Probe A: LadybugDB baseline

Run a synthetic fixture with at least 20 papers, shared authors, shared keywords, overlapping categories, and 512-dimensional embeddings.

Must prove:

1. Schema creation works from a clean database.
2. Re-running ingestion is idempotent.
3. Vector similarity retrieves the known nearest paper.
4. Graph traversal retrieves papers sharing a keyword and author path.
5. Native `page_rank` completes without fallback.
6. Ranking changes when the graph topology changes.
7. Query results are deterministic enough for tests.

### Probe B: HelixDB feasibility

Run only after identifying the current supported runtime and SDK.

Must prove:

1. Install path is reproducible in a clean environment.
2. Local runtime can be started without cloud credentials, or cloud requirement is explicit.
3. Python or HTTP client can create schema and insert data.
4. Vector query works on 512-dimensional embeddings.
5. Graph traversal works on paper-author-keyword relations.
6. Hybrid graph-vector query can be expressed without prebuilding every possible query as a stored procedure.
7. Test fixture can run in CI without external mutable state.

### Probe C: Retrieval quality fit

Use the same synthetic corpus against every candidate.

Questions:

1. Can the database retrieve semantically similar papers?
2. Can it explain recommendations through graph paths?
3. Can it combine vector similarity, graph centrality, keyword/category overlap, and recency?
4. Can it preserve evidence needed by future PageIndex and RLM workflows?

## Decision criteria

Stay with LadybugDB if:

- It continues to pass the baseline probes.
- Native graph algorithms and vector search are good enough for the near-term corpus scale.
- Its embedded model keeps tests and local Hermes workflows simple.

Reopen HelixDB seriously only if:

- It has a reproducible local or acceptable managed runtime.
- It proves real graph-vector-text functionality with maintained client APIs.
- Its stored-query model does not block exploratory agent traversal.
- It offers a clear advantage that offsets migration and operational complexity.

## Current recommendation

Continue with LadybugDB as the baseline and run the validation probes before expanding the scientific KG schema. Treat HelixDB as a candidate for comparison, not as an adopted dependency, until Probe B is proven.

## Evidence commands already run

```bash
uv run pytest tests/test_analytics.py tests/test_scientific_kg_e2e.py -q
# 10 passed
```

```bash
uv run pytest tests/test_ladybug.py -q
# 5 passed
```

```bash
uv run ruff check src/arxiv_archive/analytics.py tests/test_analytics.py
# All checks passed
```

A temporary persistent LadybugDB smoke produced two recommendations and persisted PageRank values for two papers using native `page_rank`. Source inspection of the indexed LadybugDB repo confirmed the projected graph lifecycle functions: `project_graph`, `show_projected_graphs`, `projected_graph_info`, and `drop_projected_graph`.

Additional LadybugDB integration probes confirmed:

- `ladybug==0.16.1` has no extensions loaded by default.
- `LOAD VECTOR` enables `CREATE_VECTOR_INDEX`, `QUERY_VECTOR_INDEX`, and `DROP_VECTOR_INDEX`.
- `QUERY_VECTOR_INDEX` returns `node` and `distance`; using `nn` or `_distance` fails.
- `LOAD FTS` enables `CREATE_FTS_INDEX`, `QUERY_FTS_INDEX`, and `DROP_FTS_INDEX`.
- `CREATE_FTS_INDEX` expects the indexed properties as a list, e.g. `['title', 'body']`.
- `QUERY_FTS_INDEX` returns `node` and `score`.
- Source comments in `src/include/c_api/lbug.h` state each connection is thread-safe and multiple connections may share one database instance.
- Source code in `TransactionManager::beginTransaction` rejects a second write transaction when `enableMultiWrites` is false.
- Python probes confirmed the default: while one connection has an open write transaction, another connection can read but a second write raises `Only one write transaction at a time`; concurrent autocommit writes can fail rather than queue.

## Next work items

1. Build a reusable synthetic corpus generator for database candidate probes.
2. Add a LadybugDB validation test that proves topology changes affect graph centrality.
3. Decide whether the production retrieval path should use brute-force `array_cosine_similarity`, `LOAD VECTOR` plus vector indexes, or both behind a capability check.
4. Add an application-level write gate or retry policy before introducing concurrent ingestion/server workflows.
5. Investigate current HelixDB local development docs and determine whether a no-secret local probe is possible.
6. Replace or mark the old HelixDB M002 brief as superseded by the LadybugDB baseline plus this validation spike.
