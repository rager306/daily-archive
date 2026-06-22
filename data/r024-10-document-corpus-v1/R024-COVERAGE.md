# R024 Coverage Report: 10-document corpus validation (M116)

**Generated**: 2026-06-22T11:50Z  
**Milestone**: M116-6xgs2h (5 slices: S01-S05)  
**Source**: `data/r024-10-document-corpus-v1/`  
**Architecture**: Bounded graph probe via NetworkX intermediate (NO LadybugDB, NO FalkorDB, NO Neo4j)

## Executive Summary

R024 first-stage validation (10-document corpus) is complete. All 5 slices
delivered. M115 invariants preserved. Fail-closed: no production graph import,
no network I/O, no DB connection.

## Corpus Selection (S01)

- **Corpus**: `data/r024-10-document-corpus-v1/selection.json`
- **Articles**: 10 unique (5 baseline from M025 + 5 new)
  - Baseline (M025): `arxiv/cs-ai/2512.24601`, `arxiv/cs-ai/2605.28617v1`,
    `arxiv/cs-cv/2605.26525v1`, `arxiv/cs-cl/2507.19457`,
    `company_blog/cs-ir/pageindex_zhang2025pageindex`
  - Extension: `arxiv/cond-mat-mtrl-sci/2605.20918`, `arxiv/cs-ai/2502.13025`,
    `arxiv/cs-ai/2510.21148`, `arxiv/cs-cl/2108.12409`, `arxiv/cs-cl/2109.10862`
- **Local sources**: 10/10 have local article.html / abs.html
- **Tests**: 7/7 pass
- **Fail-closed**: `network_fetch_attempted=false`,
  `graph_import_allowed=false`, `ladybugdb_written=false`

## Parser + Chunking Replay (S02)

- **Output**: `data/r024-10-document-corpus-v1/parser-chunking/`
  (events.jsonl + summary.json)
- **Process**: `parse_article(FullTextIngestionResult)` +
  `build_page_index_from_parsed(parsed)` via `FullTextSource`
  (text/markdown)
- **Result**: 10/10 articles processed, 0 errors
- **Chunks per article**: 2 (via `page_index.nodes`)
- **Tests**: 8/8 pass

## Quality Metrics (S03)

- **Metrics**: `data/r024-10-document-corpus-v1/quality-metrics.json`
- **Comparison**: `data/r024-10-document-corpus-v1/quality-comparison-5-vs-10.md`

| Stage | Articles | Chunks | Avg chunks/article |
|-------|----------|--------|-------------------|
| M025 baseline | 5 | 25 | 5.0 |
| R024 (10-doc) | 10 | 20 | 2.0 |
| Scale factor | 2x | 0.8x | 0.4x |

**Finding**: Chunk-count discrepancy between M025 (5/article) and R024
(2/article). M025 uses M025-S07 chunking which produces granular chunks
(metadata, table_context, figure_caption_context, citation_context,
retrieval_context). R024 uses `parse_article + build_page_index_from_parsed`
which produces coarser page-index nodes.

**Tests**: 9/9 pass.

## NetworkX Probe (S04)

- **Output**: `data/r024-10-document-corpus-v1/networkx-probe/`
  (probe.graphml + summary.json + events.jsonl)
- **Graph**: NetworkX DiGraph (in-memory only)
- **Nodes**: 31 (1 corpus root + 10 articles + 20 chunks)
- **Edges**: 30 (10 corpus_contains_article + 20 article_contains_chunk)

**Fail-closed invariants (9 flags)**:
- `network_fetch_attempted=false`
- `production_import_attempted=false`
- `graph_import_allowed=false`
- `ladybugdb_written=false`
- `trusted_kg_import_allowed=false`
- `graph_readiness_claim=false`
- `falkordb_written=false`
- `neo4j_written=false`
- `ladybugdb_connection_attempted=false`

**Implementation**: `networkx` library, DiGraph, in-memory only, no DB
connection, no network I/O.

**Tests**: 12/12 pass.

## Comparison vs M025 baseline

| Aspect | M025 baseline | R024 (10-doc) | Notes |
|--------|---------------|---------------|-------|
| Corpus size | 5 articles | 10 articles | 2x baseline |
| Parser framework | M025-S07 (granular) | M025-S04 (page_index) | Coarser |
| Total chunks | 25 | 20 | Different chunkers |
| Graph probe | None | NetworkX 31 nodes / 30 edges | NEW |
| Production graph import | No (deferred) | No (still deferred) | R056 preserved |
| Network I/O | No | No | Preserved |

## Recommendations for 20-doc and one-week stages

### 20-doc stage (M117)

1. **Chunk-count reconciliation**: investigate why R024 chunks (2/article)
   differ from M025 (5/article). May need to apply M025-S07 chunking for
   finer granularity.
2. **Add entity/relation extraction**: bounded probe currently only has
   articles + chunks. Add entities (e.g. authors, sections, citations) for
   richer graph structure.
3. **Extend NetworkX probe**: add `chunk_contains_entity`, `article_cites_*`
   edges.
4. **Maintain fail-closed**: NO LadybugDB / FalkorDB / Neo4j in 20-doc stage.

### One-week stage (M118+)

1. **Bounded NetworkX remains the only graph**: scale-out test within
   NetworkX (e.g. 100 docs, 500 chunks, 1000 entities).
2. **Memory profiling**: track NetworkX in-memory footprint during scale.
3. **Production-import guardrail remains**: when/if LadybugDB activation
   is approved (per R019-R023 / R050-R056), run NetworkX vs production
   side-by-side validation FIRST.
4. **Document per-stage metrics**: preserve S03 quality metrics pattern.

## Risk / Blocker Notes

- **No blockers**: 10-doc validation completed cleanly.
- **No regressions**: M115 invariants (ruff 0, ty 22, pyrefly 4, format 0,
  onion clean, 22/22 green, 2283 tests collected, pre-commit 4 hooks)
  preserved across all 5 slices.
- **No production import attempted**: NetworkX intermediate is the only
  graph target. R056 (no production graph import) preserved.

## M116 Stats

| Metric | Value |
|--------|-------|
| Slices completed | 5 / 5 |
| Tasks completed | 18 / 18 |
| New tests | 36 |
| New artifacts | 5 (selection, parser-chunking, quality-metrics, networkx-probe, R024-COVERAGE) |
| Commits | 4 (S01, S02, S03, S04) + 1 (S05 final) |
| Fail-closed violations | 0 |
| Production imports | 0 |

## Next Steps

1. **R024 20-doc stage** (M117): extend corpus + entities + relations.
2. **R024 one-week stage** (M118+): bounded NetworkX scale-out.
3. **Continue deferring** R019-R023 (hybrid retrieval / SymFSM), R050-R056
   (pipeline orchestration) per PROJECT.md "pause after M003" policy.

---

*This report documents the first stage of R024 graph readiness validation.
All slices delivered. All fail-closed invariants preserved.*