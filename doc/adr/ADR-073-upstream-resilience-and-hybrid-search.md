# ADR-073: Upstream Resilience and Hybrid Search Patterns (vendor-source research wave 2)

- Status: Accepted
- Date: 2026-08-25
- Deciders: repo owner + agent (M451)
- Tags: resilience, caching, openalex, circuit-breaker, batch-api, search, temporal, ranking
- Sources: citracer (darkbluevn1765), iwe (crates.io), graphiti (getzep), edgequake,
  semantica, pdffigures2, simd-html-to-md — all indexed in codebase-memory
  (opendataloader/dedoc/pdf-craft wave covered by ADR-072 + cv_structure)

## 0. One-line Decision

**Adopt the citracer resilience trio for OpenAlex (batched DOI fetching 50/call, negative-cache purge on logic fixes, per-upstream circuit breaker with cooldown); adopt graphiti's search-side temporal filters and RRF-reranker composition as the design frame for ADR-071 ranking; record iwe's incremental graph index, edgequake's content_ref indirection and semantica's ontology snapshot-diff as normative follow-ups.**

## 1. Context

Deep-dive of 13 vendor-source repositories via codebase-memory indexes
(+42K new nodes; totals: edgequake 63K, semantica 16K, iwe 6.7K, citracer 0.7K).
The three closest-in-domain projects carry patterns solving our live
operational problems (OpenAlex cache debt: 83×404 + 2×504) and our roadmap
gaps (temporal search, ranking fusion).

## 2. Decision

### 2.1 citracer resilience trio (shipped now)

1. **Negative-cache purge** (`purge_negatives`): terminal-negative entries
   (404/422 "not found") are not forever. They go stale when (a) the
   upstream was flaky at fetch time, (b) our resolution logic was fixed
   (citation-salvage wave), or (c) arXiv papers get indexed by OpenAlex
   days/weeks after publication. Purge on every run unless `--no-refetch`;
   positives untouched.
2. **Concurrent bounded fetching** (transport corrected by live probe):
   the `doi:` filter rejects pipe-OR (`filter=doi:A|doi:B` → HTTP 400,
   probed from a Rust network test — path form `/works/doi:X` and single
   filter both 200). Batching therefore rides on **bounded concurrency**
   of proven path-form requests (8 parallel, was sequential ~6s each).
   Latency win without unsupported API features.
3. **Circuit breaker per upstream**: after N consecutive failures
   (default 5) the upstream is skipped for a cooldown (default 120s);
   success resets the counter. Thread-safe (fetcher runs 8 workers).
   Complements the ADR-072-wave action-level retry (retry = transient
   errors within one action; breaker = upstream-down protection across
   the batch).

### 2.2 graphiti search patterns (normative design, staged impl)

- **Temporal filters as first-class search predicates** (`valid_at` /
  `invalid_at` DateFilters in queries): kg-ontology `TemporalEdge` already
  carries the fields; the missing piece is query-side "state as of date X".
  Target surface: graph task Cypher builders + future `da query`.
- **RRF + composable rerankers**: BM25 ∪ vector ∪ fulltext → reciprocal
  rank fusion → pluggable rerankers (`node_distance_reranker`,
  `episode_mentions_reranker`). This is the concrete composition frame for
  ADR-071's three-family ranking: degree / walk / contextual become three
  rerankers over one fused candidate list — one mechanism, not three paths.
- **GLiNER as a non-LLM entity extractor tier** between YAKE (statistical)
  and LLM critic (judgment): extraction candidates layer gains a model
  tier when a local GLiNER binding is vetted.

### 2.3 Normative follow-ups (recorded, not shipped)

| Pattern | Source | Application | Trigger |
|---|---|---|---|
| Incremental graph index (index_node/unindex_node over walk_edges; inclusion vs reference edges) | iwe `liwe` | navigation rebuild goes incremental for live corpus updates | when corpus updates become frequent (agent-driven acquisition) |
| retrieve context levels (one/two levels, JSON) | iwe | claim-level retrieval context expansion (ADR-071) | with retrieval plane |
| `content_ref` indirection (vector row references content by id; body lives in KV) | edgequake | future vector layer: embedding rows carry `(paper_id, section_ordinal)` refs, never inline bodies | vector layer design |
| Gleaning with provider-aware gating (disabled for local LLMs by default) | edgequake | R* depth planner (ADR-069 §2.3): expensive refinement stages gated by provider class | when a second provider class exists |
| Ontology snapshot + entity/relationship diff; provenance on ontology edits | semantica | ADR-068 SCHEMA versioning gains machine diffs between versions + per-change provenance | next schema bump |
| CandidateFilter composition (ColonOnly/NonStandardFont…), give-up thresholds (MaxDuplicateCaptionNames=3) | pdffigures2 | figure caption boundary detection beyond raw GROBID output | figure quality wave |
| `std::simd` 16-lane byte scan | simd-html-to-md | hot byte scans in extractor (dehyphenate, junk filters) — only after profiling shows a hotspot | profiling wave |
| Stratified datalog + magic-sets rewrite | cozo | declarative rule layer over kg-algorithms | distant (query language need) |
| Sharded-by-hash event loop, epoll→io_uring | pogocache | serve-graph scaling path | multi-tenant load |

### 2.4 What we do NOT adopt

- citracer's full dual-stage resolution pipeline (arxiv-search →
  openalex-review): our ID-first resolution (ADR-068 wave) already covers
  the primary path; their OpenReview stage has no analog need yet.
- iwe's markdown-as-database: our article artifacts are already structured
  layered JSON; their value to us is the INDEX discipline, not the storage.
- edgequake's Postgres/AGE storage plane: Samyama stays the sole store
  (ADR-040).
- semantica's full platform (explorer UI, plugins): patterns only.

## 3. Consequences

- **Runtime path is Rust** (user direction): `da source fetch-openalex
  <ids|pending> [--no-purge]` implements the trio in da-adapters/
  da-cli; the toolchain registry routes it as the first rust action of
  the enriched stage (`rust_actions.enriched = [fetch-openalex,
  enrich-layer]`), and the python pre-hook is removed from the runtime
  path (the reference script stays for rehearsal history only).
- Non-arXiv objects (textbook chapters, ACM DOI) get arXiv-DOI-shaped
  lookups that honestly 404 — acceptable terminal negatives, purged and
  retried harmlessly; source-aware DOI construction is a follow-up.
- Cache debt (85 negative entries) is re-examined automatically; 404s for
  young arXiv ids are expected to flip to 200 over time.
- The ADR-071 ranking frame gains its concrete composition mechanism
  (RRF + rerankers) — implementation staged behind retrieval plane.
- Testing: unit tests for purge/breaker/chunking land in
  scripts/tests/test_enrich_openalex.py (no network in tests).

## 4. Evidence

- citracer `reference_resolver.py:121-128` (purge rationale comment),
  `batch_enrich` (50-DOI batching), `_orev_circuit_open/record_failure`
  (breaker with cooldown).
- graphiti `search_filters.py` valid_at/invalid_at; `search_utils.py`
  RRF hybrid + rerankers.
- iwe `liwe/src/graph/index.rs` index_node/unindex_node.
- edgequake `chunk_storage.rs` content_ref; gleaning provider gating.
- Live debt at decision time: 83×404 + 2×504 in
  artifacts/pipeline_cache/openalex_works.
