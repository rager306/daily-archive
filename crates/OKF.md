# crates/ — Rust workspace (гексагональная архитектура)

```
crates/
├── da-domain/        ← чистые типы: Paper, Entity, VID, schema, Agent FSM
├── da-ports/         ← traits: GraphStore, Extractor, Embedder, PathConfig, paths.rs
├── da-application/   ← use cases: Ingest, Extract, Enrich, MaterializeFsm
├── da-adapters/      ← GROBID, fd_api embedder, SamyamaGraphStore, RuleBasedExtractor
├── da-cli/           ← binary `da`: serve-graph, agent-run, ingest, extract, query…
├── da-mcp/           ← MCP server для knowledge base access
├── kg-ontology/      ← универсальные типы (БЕЗ зависимостей от da-*)
├── kg-storage/       ← GraphStore + ReadOnlyServer + RemoteGraphStore
├── kg-embeddings/    ← Embedder порты и адаптеры
├── kg-algorithms/    ← графовые алгоритмы (resolve_temporal_edges)
└── kg-pipeline/      ← pipeline DSL + honest ExecutionEngine FSM

Зависимости: da-domain ← ports ← application ← cli; adapters → ports.
kg-* НЕ зависят от da-* (ADR-050).
```
