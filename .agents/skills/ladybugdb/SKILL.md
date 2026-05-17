---
name: ladybugdb
description: Master LadybugDB (Kuzu fork) for graph-vector workloads, including schema creation, bulk ingestion, concurrency rules, and graph algorithms.
---

# LadybugDB Architecture & Best Practices

LadybugDB is an embedded, columnar graph-vector database (a fork of Kuzu) optimized for analytical workloads, RAG, and heavy graph queries.

## 1. Concurrency Model (Crucial)
- **Multiple Readers, Single Writer:** LadybugDB uses a strict single-writer lock. You **cannot** execute `CREATE`, `MERGE`, or `COPY` queries concurrently from multiple threads or async tasks. Doing so will throw a transaction error.
- **Never Auto-commit in Loops:** Running thousands of individual `conn.execute("CREATE ...")` queries sequentially will be incredibly slow.
- **Solution A - Explicit Transactions:** Wrap batch writes in `BEGIN TRANSACTION` and `COMMIT`. This is 10-100x faster than auto-commit.
- **Solution B - Bulk Copy (Preferred):** The idiomatic way to ingest data is to write it to a Parquet/CSV file or an Arrow/Polars table in Python, then use `COPY NodeTable FROM 'data.parquet'`. The database's C++ engine will automatically parallelize the file reading and insertion.

## 2. Schema Definition (DDL)
LadybugDB uses Cypher for data definition. Every node table MUST have a `PRIMARY KEY`.
```cypher
CREATE NODE TABLE Paper(id STRING, title STRING, published DATE, PRIMARY KEY (id));
CREATE NODE TABLE Keyword(word STRING, PRIMARY KEY (word));
CREATE REL TABLE TaggedWith(FROM Paper TO Keyword);
```

## 3. Vector Embeddings
Vectors are treated as first-class fixed-size arrays (`FLOAT[N]`).
```cypher
CREATE NODE TABLE Document(id STRING, emb FLOAT[512], PRIMARY KEY (id));
```
- Similarity search is done natively via `array_cosine_similarity(a, b)`.
- You can create an HNSW index using the `vector` extension: `CREATE_VECTOR_INDEX(...)`.

## 4. Graph Algorithms (`algo` extension)
Do not export the entire graph to Python (e.g., NetworkX) for heavy analytics! LadybugDB has a native C++ `algo` extension that runs vectorized algorithms directly on the graph.
```python
conn.execute("INSTALL algo; LOAD EXTENSION algo;")
```
Available algorithms include **PageRank**, **Louvain Community Detection**, **Shortest Path**, and **Connected Components**.

## 5. Python Connection Lifecycle
```python
import ladybug
db = ladybug.Database('./my_graph_db')
conn = ladybug.Connection(db)
```
- Keep the `Database` instance alive for the lifetime of the application.
- Create `Connection` instances per thread (for read concurrency).
- The database is completely local and stored in the specified directory. No daemon or background server is required.
