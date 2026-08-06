# ADR-054: MCP Server for Knowledge Base External Access

**Status:** Proposed
**Date:** 2026-08-07
**Deciders:** collaborative
**Related:**
- ADR-040 (Samyama sole KG+vector+persist store — schemaless, Cypher)
- ADR-041 (embedded Cypher + HOT path)
- ADR-042 (query-local evidence activation over reified EvidenceBundle)
- ADR-043 (Research Process Plane — Execution-Grounded Scientific Memory)
- ADR-050 (kg-* crate family — ontology as DATA)
- ADR-051 §8.1 (semantic graph task suite — case-study DoD)
- ADR-052 (structured logging — tracing dual-layer JSON+stderr)
- ADR-053 (Article Preparation 2.0 — multi-layer document model for agent readiness)
- D127 (import gate — locked until explicit user yes)
- D137 (6-Wave KB Roadmap)
- Reference: Consensus MCP (https://docs.consensus.app/docs/mcp) — search + fetch tools over 200M papers
- Reference: Neo4j MCP (https://github.com/neo4j/mcp) — get-schema, read-cypher, write-cypher
- Reference: mcp-knowledge-graph (PyPI) — 10 resources, 7 tools, 4 prompts for KG retrieval
- Reference: Samyama OpenAPI (https://github.com/samyama-ai/samyama-graph/blob/main/api/openapi.yaml) — /api/query, /api/schema, /api/status

## Context

daily-archive has reached the stage where the knowledge base needs to be
accessible from external tools — AI assistants (Claude, Cursor, ChatGPT),
testing harnesses, and other MCP-compatible clients. The graph contains
2571+ nodes (Paper, Section, Author, Topic, Entity, Reference, EvidenceBundle,
Decision) and 3678+ edges (CITES, hasPart, MENTIONS, FROM_SOURCE, hasSummary,
first_child, next_sibling, parent_section, MEMBER_OF_CLUSTER) on the hub
corpus alone, with ADR-053 Waves 1-3 adding Figures, Tables, Datapoints,
Typed Entities, Summaries, and Cross-paper patterns.

Currently the only way to query the knowledge base is:
1. `da query --snapshot <file.sgsnap> --kind "MATCH ..."` — CLI, process-local
2. `da serve-graph --port 16379` + `da graph-ping` — multi-process RESP health
3. Direct Samyama Rust API (EmbeddedEngine) — requires linking the crate

None of these expose the knowledge base to external MCP-compatible clients.
An MCP server bridges this gap: any AI assistant or tool that speaks MCP can
discover the graph schema, run Cypher queries, search by vector, and traverse
the document tree — all read-only, D127-safe.

### Why MCP (not just REST API)

- **MCP is the standard** for AI assistant ↔ tool communication (Claude,
  Cursor, VS Code, ChatGPT, Copilot all support it).
- **Tools + Resources + Prompts** is a richer contract than REST endpoints:
  - Tools: parameterized queries the LLM can call
  - Resources: static/dynamic schema, ontology, vocabulary the LLM reads
  - Prompts: multi-step workflows (literature review, critical reading)
- **Consensus MCP proves the pattern**: 200M papers searchable from any
  conversation via `search` + `fetch` tools.
- **Neo4j MCP proves the graph pattern**: `get-schema` + `read-cypher` +
  `write-cypher` tools. daily-archive needs the same but Samyama-backed.
- **Testing**: MCP server enables integration tests that verify the knowledge
  base from outside the Rust process — black-box, protocol-level.

### What is NOT changing

- **D127:** MCP server is **read-only**. No write tools. import_eligible stays
  false. The governor rule `no_import_eligible_true_in_artifacts` (MEM642)
  applies to MCP response payloads too.
- **ADR-040:** Samyama remains the sole KG. MCP server is a thin adapter over
  `DirectGraphStore` / Samyama `EmbeddedEngine`, not a parallel store.
- **ADR-052:** All MCP server operations use structured tracing.
- **No LLM in the server.** The MCP server provides data access; the LLM
  (in the client) generates Cypher and reasons over results. D136 governance
  is not affected — MCP server does not extract or promote.

## Decision

Build an MCP server (`da-mcp` crate) that exposes the daily-archive knowledge
base to external MCP-compatible clients via stdio and Streamable HTTP transports.

### Architecture (hexagonal)

```
┌─────────────────────────────────────────────────────┐
│  MCP Client (Claude / Cursor / VS Code / test)      │
│  JSON-RPC over stdio or HTTP                        │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│  da-mcp crate (new, hexagonal adapter)              │
│  ┌────────────────────────────────────────────────┐ │
│  │ MCP protocol layer (JSON-RPC, tools/resources) │ │
│  ├────────────────────────────────────────────────┤ │
│  │ Tool handlers → DirectGraphStore trait         │ │
│  │ Resource handlers → ontology YAML + schema     │ │
│  │ Prompt handlers → reading-path templates       │ │
│  └────────────────────────────────────────────────┘ │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│  kg-storage (DirectGraphStore / SamyamaGraphStore)  │
│  Samyama EmbeddedEngine (in-process) OR             │
│  snapshot file (.sgsnap, loaded into process)       │
└─────────────────────────────────────────────────────┘
```

**Crate placement:** `crates/da-mcp/` — depends on `kg-storage`, `kg-ontology`,
`da-domain`, `da-ports`. Does NOT depend on `da-application` or `da-adapters`
(no write path, no GROBID, no OpenAlex). This keeps it hexagonal: the MCP
server is infrastructure that consumes ports, not application logic.

### Transport

| Transport | When | Config |
|-----------|------|--------|
| **stdio** | Local development, Claude Desktop, IDE integration | `da-mcp --stdio` |
| **Streamable HTTP** | Remote access, testing, multi-client | `da-mcp --http --port 18080 --path /mcp` |

Default: stdio (most MCP clients expect this). HTTP for testing and remote.

### Tools (parameterized queries the LLM can call)

| Tool | Input | Returns | D127-safe |
|------|-------|---------|-----------|
| `search_papers` | `query: string, limit?: int` | `[{arxiv_id, title, doi, cited_by_count, topics, abstract?}]` | ✅ read-only |
| `get_paper` | `arxiv_id: string` | Paper node + linked Section/Author/Topic/Summary nodes | ✅ |
| `get_sections` | `arxiv_id: string, path_id?: string` | Section tree (navigation edges) for one paper | ✅ |
| `get_references` | `arxiv_id: string, direction?: "forward"\|"backward"\|"both"` | CITES edges (papers this cites + papers citing this) | ✅ |
| `get_entities` | `arxiv_id: string, entity_type?: string` | Entity nodes for one paper (Layer C typed: Method/Dataset/Baseline) | ✅ |
| `get_summary` | `arxiv_id: string, summary_type?: string` | Summary nodes (Layer B: abstract/methods/results/limitations/reproducibility) | ✅ |
| `read_cypher` | `query: string, params?: object` | Raw Cypher result (read-only, enforced by Samyama `query_readonly`) | ✅ read-only |
| `vector_search` | `query_embedding: [f32], k?: int` | Top-k nodes by vector similarity | ✅ |
| `get_schema` | — | Node labels, edge types, property keys (from YAML ontology) | ✅ |
| `get_graph_stats` | — | Node/edge counts by label/type | ✅ |
| `validate_node` | `label: string, properties: object` | Schema violations (structural + semantic vocabulary) | ✅ |
| `get_reading_path` | `arxiv_id: string, path: string` | Nodes + edges along a reading-path template (ADR-053 Layer D) | ✅ |

**Safety:** `read_cypher` uses `DirectGraphStore::query_readonly` — Samyama
enforces read-only at the engine level. No `write_cypher` tool. No tool can
set `import_eligible=true`.

**Rate limiting:** optional `--rate-limit N` (requests/sec). Default: unlimited
for stdio (local), 10 req/s for HTTP (polite).

### Resources (static/dynamic data the LLM reads)

| URI | Type | Content |
|-----|------|---------|
| `graph://schema` | static | Node labels + edge types + property keys (from YAML) |
| `graph://ontology/node-schemas` | static | `data/ontology/node_schemas.yaml` content |
| `graph://ontology/edge-contracts` | static | `data/ontology/edge_contracts.yaml` content |
| `graph://ontology/decision-categories` | static | `data/ontology/decision_categories.yaml` content |
| `graph://ontology/vocabulary` | static | `data/ontology/node_vocabulary.yaml` content |
| `graph://stats` | dynamic | Current node/edge counts, label distribution |
| `graph://adr-index` | static | ADR list (titles + status) for architecture context |
| `graph://graph-tasks` | static | Available reading-path templates (Layer D) |

### Prompts (multi-step workflows)

| Prompt | Parameters | Workflow |
|--------|-----------|----------|
| `literature_review` | `topic: string, depth?: "quick"\|"standard"` | RECON → PLAN → SEARCH → SYNTHESIZE (Consensus-inspired) |
| `critical_reading` | `arxiv_id: string` | Abstract → Method → Results → Limitations traversal |
| `reproducibility_check` | `arxiv_id: string` | Method → Setup → Hyperparams → Code/Data availability |
| `citation_lineage` | `arxiv_id: string, depth?: int` | Forward + backward citation traversal with abstracts |
| `compare_methods` | `method_name: string` | Find all papers proposing this method, compare results |
| `generate_cypher` | `question: string` | Schema context + question → Cypher query template |

### Snapshot mode (for testing)

For integration tests and local development, the MCP server loads a snapshot
file into an in-process Samyama engine:

```bash
da-mcp --stdio --snapshot artifacts/d127_unlock/hub_unlocked_t04.sgsnap
```

This allows tests to verify the knowledge base content via MCP protocol
without a running daemon. Snapshot mode is process-local (dies on exit).

For live mode (future, when Samyama daemon exists):

```bash
da-mcp --http --port 18080 --remote samyama://localhost:8080
```

### Crate dependencies (hexagonal)

```toml
[dependencies]
kg-storage = { path = "../kg-storage" }   # DirectGraphStore trait + MockGraphStore
kg-ontology = { path = "../kg-ontology" } # TemporalEdge, Violation, registry
da-domain = { path = "../da-domain" }     # schema labels, relation constants
da-ports = { path = "../da-ports" }       # Clock, GraphStore port
# MCP protocol implementation (TBD: rmcp crate or hand-rolled JSON-RPC)
```

**No dependency on da-application or da-adapters.** The MCP server never
ingests, enriches, or extracts — it only reads.

### Testing strategy

1. **Unit tests:** tool handlers with `MockGraphStore` (same pattern as
   da-application tests)
2. **Integration tests:** load `hub_unlocked_t04.sgsnap`, call each tool via
   MCP protocol, assert results
3. **Graph task suite integration:** MCP `get_reading_path` tool calls the
   same Cypher templates as `graph_task_suite` runner — consistent results
4. **Protocol tests:** verify JSON-RPC compliance (tool discovery, resource
   reads, prompt rendering)

### Implementation waves (thin slices)

| Wave | Slice | Scope | Milestone |
|------|-------|-------|-----------|
| 1 | MCP-1 | `da-mcp` crate skeleton + stdio transport + `get_schema` + `get_graph_stats` tools | M363 S06 or new M365 |
| 1 | MCP-2 | `read_cypher` + `search_papers` + `get_paper` tools | same |
| 1 | MCP-3 | `get_sections` + `get_references` + `get_entities` tools | same |
| 2 | MCP-4 | Resources (ontology YAML, schema, stats) | M364 or M365 |
| 2 | MCP-5 | Prompts (literature_review, critical_reading) | M364 or M365 |
| 2 | MCP-6 | `vector_search` tool + embedding query support | M364 or M365 |
| 3 | MCP-7 | HTTP transport + rate limiting | M365+ |
| 3 | MCP-8 | `get_summary` + `get_reading_path` (depends on ADR-053 Layer B/D) | M363+ |
| 3 | MCP-9 | Integration test suite (snapshot-based) | M365+ |

## Consequences

### Positive

- **AI assistants can query the knowledge base directly** — Claude, Cursor,
  VS Code can `read_cypher`, `search_papers`, `get_sections` without shell
  commands or snapshot file management.
- **Testing from outside the Rust process** — MCP protocol-level integration
  tests verify the graph content is correct and accessible.
- **Consensus-inspired search + fetch** — `search_papers` + `get_paper` tools
  mirror the Consensus MCP pattern, familiar to any MCP client.
- **Neo4j-inspired schema + Cypher** — `get_schema` + `read_cypher` mirror
  the Neo4j MCP pattern, familiar to graph database users.
- **ADR-053 Layer D reading paths** become accessible via `get_reading_path`
  tool — agents can traverse critical_reading / reproducibility_check / etc.
- **Ontology as DATA** (ADR-050) — resources expose YAML ontology directly;
  clients always see the current schema.
- **D127-safe by design** — read-only tools, no write path, no import unlock.

### Negative

- **New crate + MCP protocol dependency.** Need to choose MCP implementation
  (hand-rolled JSON-RPC, or `rmcp` Rust crate if it exists).
- **Snapshot mode is process-local.** Each MCP session loads the snapshot
  fresh — no incremental updates. Live mode requires Samyama daemon (future).
- **Cypher query validation.** `read_cypher` must enforce read-only. Samyama
  `query_readonly` does this at engine level, but we should also validate
  client-side (reject `CREATE`, `SET`, `DELETE`, `MERGE` keywords).
- **Response size limits.** Cypher queries can return large result sets.
  Need `--max-response-tokens` limit (Neo4j MCP uses
  `NEO4J_RESPONSE_TOKEN_LIMIT`).

### Risks and mitigations

| Risk | Mitigation |
|------|------------|
| MCP protocol changes (spec is evolving) | Pin to a specific MCP protocol version; test against multiple clients |
| Samyama Cypher dialect limitations (no IS NOT NULL, CONTAINS, NOT pattern) | `get_schema` resource documents Samyama Cypher capabilities; prompts guide LLM to compatible queries |
| Snapshot staleness (loaded once at startup) | Document clearly: snapshot mode is for testing; live mode requires daemon |
| Rate limit abuse on HTTP transport | Default 10 req/s; configurable; per-client tracking |
| LLM generates destructive Cypher | `read_cypher` uses `query_readonly` (engine-enforced); client-side keyword check; no `write_cypher` tool |
| Large ontology YAML in resource response | Stream or paginate resources; `get_schema` returns summary, full YAML is a separate resource |

## LLM Reading Notes

- **Read-only by design.** No `write_cypher` tool. D127 import gate is not
  affected. The MCP server never mutates the graph.
- **Hexagonal.** `da-mcp` depends on `kg-storage` (port), `kg-ontology`,
  `da-domain`, `da-ports`. Does NOT depend on `da-application` or
  `da-adapters`. No ingest/enrich/extract in the MCP server.
- **Snapshot mode for testing.** `--snapshot <file.sgsnap>` loads the graph
  into an in-process Samyama engine. Dies on exit. Same pattern as
  `graph_task_suite` runner (ADR-051 §8.1).
- **Samyama Cypher limitations documented.** Resources and prompts guide the
  LLM to Samyama-compatible Cypher (no IS NOT NULL, CONTAINS, NOT pattern).
  MEM654 / T05 experience informs this.
- **Consensus + Neo4j patterns borrowed.** `search_papers` + `get_paper`
  from Consensus; `get_schema` + `read_cypher` from Neo4j MCP. Proven
  patterns, familiar to MCP clients.
- **Wave mapping.** Wave 1 (MCP-1..3) covers core read tools; Wave 2
  (MCP-4..6) adds resources + prompts + vector; Wave 3 (MCP-7..9) adds HTTP
  transport + reading paths + integration tests. Each wave is independently
  useful.
- **MCP protocol implementation TBD.** Evaluate `rmcp` Rust crate vs
  hand-rolled JSON-RPC. Hand-rolled is simpler for stdio; `rmcp` may help
  with HTTP transport. Decide in MCP-1 slice.

## Open questions (deferred to implementation)

1. Which Rust MCP crate to use? (`rmcp` if it exists and is maintained;
   otherwise hand-rolled JSON-RPC over stdio + axum for HTTP.)
2. Should `vector_search` accept raw embedding vectors or accept text and
   embed server-side? (Leaning: accept text, embed server-side via
   `FdApiEmbedder` — but this adds a dependency on kg-embeddings. Alternative:
   accept pre-computed vectors only, client does embedding.)
3. Should the MCP server support multiple snapshots simultaneously (multi-tenant)?
   (Leaning: no — one snapshot per process. Multi-tenant is a daemon concern.)
4. OAuth for HTTP transport? (Consensus uses OAuth. For local development,
   no auth. For remote, token-based auth. Defer to Wave 3.)

## Supersession

None. This ADR **complements** ADR-040 (Samyama sole KG — MCP server is a
read-only adapter over it), ADR-041 (embedded Cypher — MCP `read_cypher` uses
it), ADR-050 (ontology as DATA — MCP resources expose YAML directly), and
ADR-053 (Article Preparation 2.0 — MCP tools expose Layers A-J to external
clients). It does not supersede any prior decision.
