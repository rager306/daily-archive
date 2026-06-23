# R024 Coverage Report: r024-218-document-corpus-v1

**Generated**: 2026-06-23T13:48:43+00:00
**Milestone**: M122 Pipeline Script Architecture Migration
**Source**: `r024-218-document-corpus-v1/`
**Architecture**: Offline catalog coverage, parser/chunking replay, and bounded graph probe. NO network, NO LadybugDB, NO FalkorDB, NO Neo4j, NO production graph import.

## Executive Summary

Coverage for `r024-218-document-corpus-v1` records **221 article records**, including **166 M056** records where applicable.

The verified downstream stages are:
1. **Catalog coverage**: denominator `221/221 included`.
2. **Parser/chunking replay**: **219 source-backed** records completed; **2 metadata-only** records skipped; **0 errors**.
3. **NetworkX graph probe**: **3891 nodes**, **10102 edges**, **6212 citation edges**, **13.43 MB peak memory**.

This report does **not** claim production graph readiness. It preserves fail-closed semantics.

## Stage Summary

| Stage | Evidence | Result |
|-------|----------|--------|
| Catalog ingest | `data/r024-218-document-corpus-v1/ingest-summary.json` | 221 article records |
| Parser + Chunking Replay | `data/r024-218-document-corpus-v1/parser-chunking/summary.json` | 219 completed, 2 skipped, 0 errors |
| NetworkX Probe | `data/r024-218-document-corpus-v1/networkx-probe/summary.json` | 3891 nodes, 10102 edges |

## Catalog Expansion (S01-S03)

### Results

- **M056 cumulative records**: 166
- **Catalog article records after ingest**: 221
- **M056 records ingested**: 166
- **Fail-closed metadata**: source variants remain offline and do not authorize production import

## Parser + Chunking Replay (S04)

### Results

- **Total catalog records**: 221
- **Source-backed records completed**: 219
- **Metadata-only records skipped**: 2
- **Errors**: 0
- **HTML-NATIVE sources**: 21
- **PDF-CONVERTED sources**: 198
- **Chunk count total**: 2576

### Metadata-only exclusions

- `arxiv/mixed-source/2605.29548`
- `stanford/cs224n/gradient-notes`
- `metadata_only_no_local_source_artifact`: 2

## NetworkX Probe (S05)

| Metric | Value |
|--------|-------|
| Source-backed records | 219 |
| Metadata-only exclusions | 2 |
| Chunks | 2576 |
| Total nodes | 3891 |
| Total edges | 10102 |
| Citation relations | 6212 |
| Peak memory | 13.43 MB |

## Verification Baseline

Fail-closed invariants:

- `network_fetch_attempted=false`
- `production_import_attempted=false`
- `graph_import_allowed=false`
- `ladybugdb_written=false`
- `falkordb_written=false`
- `neo4j_written=false`
- NO network, NO LadybugDB, NO FalkorDB, NO Neo4j, NO production graph import

Diagnostics:

- `metadata_only_no_local_source_artifact`: 2 (parser replay skipped records)

## R024 Interpretation

R024 coverage currently validates 219 source-backed records out of 221 parser replay records, with 2 metadata-only exclusions recorded explicitly.

The result advances corpus evidence while preserving fail-closed boundaries and does **not** claim production graph readiness.

## Recommendations

1. Continue using package use cases for coverage regeneration instead of milestone script logic.
2. Keep metadata-only exclusions explicit and fail-closed.
3. Treat NetworkX graph output as bounded evidence, not production graph readiness.

## Files of Record

- `data/r024-218-document-corpus-v1/ingest-summary.json` (json-summary)
- `data/r024-218-document-corpus-v1/parser-chunking/summary.json` (json-summary)
- `data/r024-218-document-corpus-v1/networkx-probe/summary.json` (json-summary)
- `/root/daily-archive/data/r024-218-document-corpus-v1/R024-COVERAGE.md` (coverage markdown)
- `/root/daily-archive/data/r024-218-document-corpus-v1/coverage-summary.json` (coverage json summary)
