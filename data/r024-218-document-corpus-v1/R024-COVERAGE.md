# R024 Coverage Report: M121 Catalog Expansion to 221 Article Records

**Generated**: 2026-06-23T07:10Z  
**Milestone**: M121-kd3kzr — Catalog Expansion: Ingest 165 M056 pre-positioned PDFs via catalog_ingest package  
**Source**: `data/r024-218-document-corpus-v1/`  
**Architecture**: Offline catalog expansion, parser/chunking replay, and bounded NetworkX probe. NO network, NO LadybugDB, NO FalkorDB, NO Neo4j, NO production graph import.

## Executive Summary

M121 expanded the canonical article catalog from 55 article records to **221 article records** by ingesting **166 M056 pre-positioned PDFs** through the repository package path `research_graph.infrastructure.corpus.ingestion.catalog_ingest`.

The milestone then verified the expanded catalog through three downstream stages:

1. **Catalog integrity**: all 166 M056 PDFs SHA256-verified; all M056 article records fail-closed and offline.
2. **Parser/chunking replay**: **219 source-backed records** processed; **2 metadata-only records** skipped explicitly; **0 errors**.
3. **NetworkX graph probe**: **3891 nodes**, **10102 edges**, **6212 citation edges**, **13.81 MB peak memory**, in-memory only.

M121 does **not** claim production graph readiness. It advances R024 by expanding and validating the source-backed corpus boundary while preserving fail-closed semantics.

## Stage Summary

| Slice | Stage | Evidence | Result |
|-------|-------|----------|--------|
| S01 | M056 corpus loader | `load_m056_corpus`, `verify_m056_sha256`, `tests/test_catalog_ingest_m056.py` | 166 records loaded, 166 SHA256 matches |
| S02 | Catalog ingest | `scripts/ingest_m056_corpus.py`, `ingest-summary.json` | catalog 55 → 221 article records |
| S03 | Catalog integrity tests | `tests/test_catalog_expansion_m121.py` | 8/8 tests pass; offline metadata drift fixed |
| S04 | Parser/chunking replay | `parser-chunking/summary.json`, `tests/test_r024_218_document_parser_chunking.py` | 219 completed, 2 skipped, 0 errors |
| S05 | NetworkX probe | `networkx-probe/summary.json`, `tests/test_r024_218_document_networkx_probe.py` | 3891 nodes, 10102 edges, 13.81 MB peak |

## Catalog Expansion (S01-S03)

### Inputs

- M056 source corpus: `artifacts/m056-bfs-graph/cumulative-corpus.json`
- M056 PDFs: existing local files under `data/article_catalog/article_catalog/arxiv/.../source/*.pdf`
- No arXiv API calls or external network access were authorized.

### Results

- **M056 cumulative records**: 166
- **SHA256 verified**: 166/166
- **Catalog article records after ingest**: 221
- **M056 records ingested**: 166
- **Index entries**: 221
- **Fail-closed metadata**: all M056 source variants now have `network_fetch_attempted=false`

### S03 drift found and fixed

S03 tests exposed a real S02 metadata drift: `build_article_record` output left the metadata source variant claiming `network_fetch_attempted=true` even though M121 ingest was offline and `safety_override.external_network_authorized=false`.

`script/ingest_m056_corpus.py` was fixed to patch M056 records with explicit offline corpus metadata:

- `source_kind=m056_cumulative_corpus_local_pdf`
- metadata source role `m056_cumulative_corpus_json`
- all source variants `network_fetch_attempted=false`
- existing records are rewritten unless they already satisfy the offline M056 contract

### Catalog integrity tests

`tests/test_catalog_expansion_m121.py`: **8/8 pass**

Covered:

- 221 article records exist
- 166 M056 records exist at PDF parent paths
- SHA256 matches cumulative-corpus.json
- article identity metadata matches M056 records
- fail-closed safety flags remain false
- M056 records do not claim network fetches
- `index.json` matches actual `article.json` files
- index includes all M056 article keys

## Parser + Chunking Replay (S04)

### Runtime artifacts

- Script: `scripts/replay_r024_218_document_parser_chunking.py`
- Summary: `data/r024-218-document-corpus-v1/parser-chunking/summary.json`
- Events: `data/r024-218-document-corpus-v1/parser-chunking/events.jsonl`
- PDF text cache: `data/r024-218-document-corpus-v1/pdf-text-cache/`

### Results

- **Total catalog records**: 221
- **Source-backed records completed**: 219
- **Metadata-only records skipped**: 2
- **Errors**: 0
- **PDF-converted sources**: 198
- **HTML-native sources**: 21
- **Chunk count total**: 2576
- **Chunk count min/max**: 1 / 185

### Metadata-only exclusions

Two catalog entries are metadata-only and have no local PDF/HTML/text source artifact:

1. `arxiv/mixed-source/2605.29548`
2. `stanford/cs224n/gradient-notes`

They are explicitly recorded as `parser_chunking_skipped_metadata_only` with `skip_reason=metadata_only_no_local_source_artifact`.

No network fetch was attempted to fill these gaps. This is intentional and preserves the fail-closed boundary.

### Parser/chunking tests

`tests/test_r024_218_document_parser_chunking.py`: **12/12 pass**

Covered:

- output directory, summary, events, and cache exist
- parser/chunking summary is fail-closed
- 219 completed, 2 skipped, 0 errors
- positive chunk counts for completed records
- source kind counts include 198 PDF + 21 HTML
- events match canonical index refs
- no error events exist

## NetworkX Probe (S05)

### Runtime artifacts

- Script: `scripts/build_r024_218_document_networkx_probe.py`
- GraphML: `data/r024-218-document-corpus-v1/networkx-probe/probe.graphml`
- Summary: `data/r024-218-document-corpus-v1/networkx-probe/summary.json`
- Memory profile: `data/r024-218-document-corpus-v1/networkx-probe/memory-profile.json`
- Events: `data/r024-218-document-corpus-v1/networkx-probe/events.jsonl`

### Graph results

| Metric | Value |
|--------|-------|
| Source-backed records | 219 |
| Metadata-only exclusions | 2 |
| Chunks | 2576 |
| Entity types | 5 |
| Entity nodes | 1095 |
| Citation relations | 6212 |
| Total nodes | 3891 |
| Total edges | 10102 |
| Peak memory | 13.81 MB |

### Node types

| Node type | Count |
|-----------|-------|
| corpus | 1 |
| article | 219 |
| chunk | 2576 |
| entity | 1095 |

### Edge types

| Edge type | Count |
|-----------|-------|
| corpus_contains_article | 219 |
| article_contains_chunk | 2576 |
| article_has_entity | 1095 |
| article_cites_article | 6212 |

### Fail-closed invariants

All false in S05 summary and events:

- `network_fetch_attempted=false`
- `production_import_attempted=false`
- `graph_import_allowed=false`
- `ladybugdb_written=false`
- `trusted_kg_import_allowed=false`
- `graph_readiness_claim=false`
- `falkordb_written=false`
- `neo4j_written=false`
- `ladybugdb_connection_attempted=false`

### NetworkX tests

`tests/test_r024_218_document_networkx_probe.py`: **10/10 pass**

Covered:

- GraphML loadability
- summary node/edge counts
- fail-closed invariants
- in-memory implementation contract
- excluded metadata-only records
- memory profile bounds
- parser/probe event coverage

## Verification Baseline

Final S05 baseline before S06:

- Targeted S05 tests: **10 passed**
- `ruff check src/ tests/ scripts/`: clean
- `ruff format --check src/ tests/ scripts/`: clean
- `ty check src tests scripts`: **23 diagnostics** (accepted baseline)
- `pyrefly check`: **4 errors** (accepted baseline, 742 suppressed)
- Onion layering: clean
- Package skeleton: **22/22 passed**
- Pytest collection: **2547 tests collected**

## R024 Interpretation

M121 advances R024 from a 53-record catalog-limited validation to a **221-record catalog expansion** with a **219-record source-backed validation boundary**.

Validated:

- local catalog expansion via package code
- SHA256 verification for all M056 PDFs
- offline parser/chunking replay for all source-backed records
- bounded in-memory graph probe at 219-record scale
- explicit metadata-only exclusions
- fail-closed no-network/no-production-write invariants

Not validated:

- production graph import readiness
- LadybugDB/FalkorDB/Neo4j writes
- real semantic entity/relation extraction quality
- source-backed parsing for the two metadata-only records

## Recommendations

1. Treat M121 as successful R024 scale evidence for source-backed local artifacts.
2. Keep R024 active if the project still requires production KG readiness; M121 does not cross that gate.
3. Future source acquisition should capture local artifacts for:
   - `arxiv/mixed-source/2605.29548`
   - `stanford/cs224n/gradient-notes`
4. A future production-readiness milestone should replace synthetic entity nodes and coarse-category citation edges with real extracted entities/relations before any graph DB activation.

## Files of Record

- `data/r024-218-document-corpus-v1/ingest-summary.json`
- `data/r024-218-document-corpus-v1/parser-chunking/summary.json`
- `data/r024-218-document-corpus-v1/networkx-probe/summary.json`
- `data/r024-218-document-corpus-v1/networkx-probe/memory-profile.json`
- `tests/test_catalog_expansion_m121.py`
- `tests/test_r024_218_document_parser_chunking.py`
- `tests/test_r024_218_document_networkx_probe.py`
