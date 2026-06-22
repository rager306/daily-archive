# R024 Coverage Report: 20-document corpus validation (M117)

**Generated**: 2026-06-22T15:30Z  
**Milestone**: M117-hoqwxd (5 slices: S01-S05)  
**Source**: `data/r024-20-document-corpus-v1/`  
**Architecture**: Extended NetworkX probe (entities + relations; NO LadybugDB, NO FalkorDB, NO Neo4j)

## Executive Summary

R024 second-stage validation (20-document corpus) is complete. All 5 slices
delivered. M115 invariants preserved. Fail-closed: no production graph import,
no network I/O, no DB connection.

## Corpus Selection (S01)

- **Corpus**: `data/r024-20-document-corpus-v1/selection.json`
- **Articles**: 20 unique (10 baseline from M116 + 10 new extension)
  - Baseline (M116): `arxiv/cs-ai/2512.24601`, `arxiv/cs-ai/2605.28617v1`,
    `arxiv/cs-cv/2605.26525v1`, `arxiv/cs-cl/2507.19457`,
    `company_blog/cs-ir/pageindex_zhang2025pageindex`,
    `arxiv/cond-mat-mtrl-sci/2605.20918`, `arxiv/cs-ai/2502.13025`,
    `arxiv/cs-ai/2510.21148`, `arxiv/cs-cl/2108.12409`, `arxiv/cs-cl/2109.10862`
  - Extension: `arxiv/cs-cl/2605.18211`, `arxiv/cs-cv/1804.02767`,
    `arxiv/cs-lg/2111.00396`, `arxiv/mixed-source/2603.04448`,
    `arxiv/cs-cl/2511.20639`, `arxiv/cs-lg/2203.14465`,
    `arxiv/mixed-source/2605.21401`, `arxiv/mixed-source/2605.25522`,
    `arxiv/mixed-source/2605.20897`, `arxiv/mixed-source/2604.18478`
- **Categories**: cs-cl (5), mixed-source (5), cs-ai (4), cs-cv (2), cs-lg (2),
  cond-mat-mtrl-sci (1), cs-ir (1)
- **Local sources**: 20/20 have local abs.html
- **Tests**: 10/10 pass
- **Fail-closed**: `network_fetch_attempted=false`,
  `graph_import_allowed=false`, `ladybugdb_written=false`

## Parser + Chunking Replay (S02)

- **Output**: `data/r024-20-document-corpus-v1/parser-chunking/`
  (events.jsonl + summary.json)
- **Process**: `parse_article(FullTextIngestionResult)` +
  `build_page_index_from_parsed(parsed)` via `FullTextSource`
- **Result**: 20/20 articles processed, 0 errors
- **Chunks per article**: 2 (consistent with M116)
- **Tests**: 8/8 pass

## Quality Metrics (S03)

- **Metrics**: `data/r024-20-document-corpus-v1/quality-metrics.json`
- **Comparison**: `data/r024-20-document-corpus-v1/quality-comparison-10-vs-20.md`

| Stage | Articles | Chunks | Avg chunks/article |
|-------|----------|--------|-------------------|
| M116 baseline (10-doc) | 10 | 20 | 2.0 |
| M117 (20-doc) | 20 | 40 | 2.0 |
| Scale factor | 2x | 2x | 1x (linear) |

**Finding**: Linear scaling. Same chunk-count per article (2) as M116.

**Tests**: 9/9 pass.

## Extended NetworkX Probe (S04)

- **Output**: `data/r024-20-document-corpus-v1/networkx-probe/`
  (probe.graphml + summary.json + events.jsonl)
- **Graph**: NetworkX DiGraph (in-memory only)
- **Nodes**: 161 (1 corpus + 20 articles + 40 chunks + 100 entities)
- **Edges**: 188
  - 20 `corpus_contains_article`
  - 40 `article_contains_chunk`
  - 100 `article_has_entity`
  - 28 `article_cites_article` (via coarse_topic_code)

**Entity types** (5 per article, derived from M025 chunk types):
- `metadata`
- `table_context`
- `figure_caption_context`
- `citation_context`
- `retrieval_context`

**Citation relations** (28 edges):
- Derived from coarse_topic_code from article_ref path
- Categories with multiple articles: cs-ai (4), cs-cl (5), cs-cv (2),
  cs-lg (2), mixed-source (5)
- Within-category articles cite each other

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

**Tests**: 14/14 pass.

## Comparison vs M116 (10-doc)

| Aspect | M116 (10-doc) | M117 (20-doc) | Notes |
|--------|---------------|---------------|-------|
| Corpus size | 10 articles | 20 articles | 2x baseline |
| Chunks per article | 2 | 2 | Consistent |
| Total chunks | 20 | 40 | Linear scale |
| Graph probe nodes | 31 | 161 | 5.2x (entities added) |
| Graph probe edges | 30 | 188 | 6.3x (citations added) |
| Entity nodes | 0 | 100 | NEW |
| Citation edges | 0 | 28 | NEW |
| Production graph import | No (deferred) | No (still deferred) | R056 preserved |
| Network I/O | No | No | Preserved |

## Recommendations for one-week stage (M118+)

### one-week stage (next milestone)

1. **Linear scaling confirmed**: 20-doc → 40-doc → 100-doc → one-week should
   follow linear pattern. Memory budget: ~100 chunks/article × one-week corpus.
2. **Entity extraction at scale**: M117 used synthetic entities derived from
   M025 chunk types. For one-week, explore real entity extraction (lightweight,
   no production).
3. **Citation relations quality**: M117 used coarse_topic_code as proxy.
   For one-week, use real citation extraction from article bodies.
4. **Memory profiling**: track NetworkX in-memory footprint during scale.
5. **Maintain fail-closed**: NO LadybugDB / FalkorDB / Neo4j in one-week stage.
6. **Bounded NetworkX remains the only graph**: validate within NetworkX first.

### Future R024 stages

1. **M118+ (one-week corpus)**: extend to ~100+ articles, bounded NetworkX scale-out.
2. **M119+ (production-readiness gate)**: validate NetworkX ↔ production graph
   side-by-side (NEVER activate production; only validate equality).
3. **Pause feature expansion** continues per PROJECT.md policy.

## Risk / Blocker Notes

- **No blockers**: 20-doc validation completed cleanly.
- **No regressions**: M115 invariants (ruff 0, ty 21, pyrefly 4, format 0,
  onion clean, 22/22 green, 2336 tests collected, pre-commit 4 hooks)
  preserved across all 5 slices.
- **No production import attempted**: NetworkX intermediate is the only
  graph target. R056 (no production graph import) preserved.

## M117 Stats

| Metric | Value |
|--------|-------|
| Slices completed | 5 / 5 |
| Tasks completed | 18 / 18 |
| New tests | 41 (10+8+9+14) |
| New artifacts | 5 (selection, parser-chunking, quality-metrics, networkx-probe, R024-COVERAGE) |
| Commits | 4 (S01-S04) + 1 (S05 final) |
| Fail-closed violations | 0 |
| Production imports | 0 |
| NetworkX nodes | 161 |
| NetworkX edges | 188 |

## M116+M117 Combined Stats

| Metric | M116 (10-doc) | M117 (20-doc) | Total |
|--------|---------------|---------------|-------|
| Slices | 5 | 5 | 10 |
| Tasks | 18 | 18 | 36 |
| Tests added | 48 | 41 | 89 |
| Commits | 2 | 4 | 6+ |
| NetworkX nodes | 31 | 161 | n/a |
| NetworkX edges | 30 | 188 | n/a |

## Next Steps

1. **R024 one-week stage** (M118+): extend corpus, real entity extraction,
   memory profiling, scale-out validation.
2. **Continue deferring** R019-R023 (hybrid retrieval / SymFSM), R050-R056
   (pipeline orchestration) per PROJECT.md "pause after M003" policy.

---

*This report documents the second stage of R024 graph readiness validation.
All slices delivered. All fail-closed invariants preserved.*